//! Metadata cursors survive restarts; full-sync pages retain the P2 frozen lease.
use super::{
    basis::DataVersion,
    contract::McpError,
    durable,
    job_types::ChangesArgs,
    platform,
    query::{self, QueryState},
    query_types::QueryArgs,
    security::Authorization,
};
use crate::database::Database;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

#[derive(Clone, Serialize, Deserialize)]
struct Cursor {
    predecessor: Option<String>,
    client: String,
    revision: u64,
    fingerprint: String,
    basis: DataVersion,
    epoch: String,
    floor: i64,
    after: i64,
    seen: HashSet<i64>,
    expires: i64,
    full: bool,
    page: Option<String>,
    snapshot: Option<String>,
}
pub struct Changes {
    root: PathBuf,
}
impl Changes {
    pub(crate) fn health(
        &self,
        db: &Database,
        client: &str,
        auth: &Authorization,
        token: &str,
    ) -> Result<Value, McpError> {
        let c: Cursor = durable::read(&self.root, token, 2 * 1024 * 1024)
            .map_err(|_| McpError::new("not_found"))?;
        if c.client != client {
            return Err(McpError::new("not_found"));
        }
        let live = db.mcp_basis()?;
        Ok(
            json!({"valid":db.mcp_quality_health()?["healthy"]==true && db.mcp_change_index()?.get("historyFloor").and_then(Value::as_i64).is_some_and(|floor|c.after>=floor) && c.revision==auth.revision && c.expires>=Utc::now().timestamp() && live.database_uuid==c.basis.database_uuid && live.data_generation==c.basis.data_generation && live.index_epoch.as_deref()==Some(c.epoch.as_str()),"fullSyncInProgress":c.full,"expiresAt":c.expires,"indexEpoch":c.epoch,"historyFloor":c.floor,"watermark":c.after,"businessBodyIncluded":false}),
        )
    }
    pub fn new(parent: PathBuf) -> Self {
        Self {
            root: parent.join("changes"),
        }
    }
    pub fn execute(
        &mut self,
        q: &mut QueryState,
        db: &Database,
        client: &str,
        auth: &Authorization,
        args: ChangesArgs,
    ) -> Result<Value, McpError> {
        let root = durable::root(
            self.root
                .parent()
                .ok_or_else(|| McpError::new("security_unavailable"))?,
            "changes",
        )?;
        let now = Utc::now().timestamp();
        // Expired metadata is safe to prune; no business objects are stored here.
        for id in durable::ids(&root)? {
            if let Ok(c) = durable::read::<Cursor>(&root, &id, 2 * 1024 * 1024) {
                if c.expires < now {
                    durable::remove(&root, &id)?;
                }
            }
        }
        let mode = args.mode.as_deref().unwrap_or("events");
        let since = args
            .since
            .as_ref()
            .map(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| McpError::new("invalid_arguments"))
            })
            .transpose()?;
        if !["events", "current"].contains(&mode) {
            return Err(McpError::new("invalid_arguments"));
        }
        let limit = args.limit.unwrap_or(100);
        if !(1..=100).contains(&limit) {
            return Err(McpError::new("invalid_arguments"));
        }
        let filters = args.filters.unwrap_or_default();
        query::validate(&filters, auth)?;
        let fields = query::projection(args.projection.as_ref(), auth)?;
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&json!([
                    mode,
                    filters,
                    fields,
                    limit,
                    args.since,
                    auth.permissions,
                    auth.scope
                ]))
                .unwrap()
            )
        );
        let live = db.mcp_basis()?;
        let index = db.mcp_change_index()?;
        let mut cursor = if let Some(token) = args.cursor.as_deref() {
            let c: Cursor = durable::read(&root, token, 2 * 1024 * 1024)
                .map_err(|_| McpError::new("cursor_invalid"))?;
            if c.client != client || c.fingerprint != fingerprint {
                return Err(McpError::new("cursor_invalid"));
            }
            if c.revision != auth.revision {
                return Err(McpError::new("authorization_changed"));
            }
            if c.expires < now
                || c.basis.database_uuid != live.database_uuid
                || c.basis.data_generation != live.data_generation
                || c.epoch != index["epoch"].as_str().unwrap_or("")
                || c.after < index["historyFloor"].as_i64().unwrap_or(i64::MAX)
            {
                return Err(McpError::new("resync_required"));
            }
            c
        } else {
            Cursor {
                predecessor: None,
                client: client.into(),
                revision: auth.revision,
                fingerprint,
                basis: live.clone(),
                epoch: index["epoch"].as_str().unwrap_or("").into(),
                floor: index["historyFloor"].as_i64().unwrap_or(0),
                after: live.commit_sequence as i64,
                seen: HashSet::new(),
                expires: now + 600,
                full: true,
                page: None,
                snapshot: None,
            }
        };
        let was_full = cursor.full;
        let mut items = vec![];
        let mut has_more = false;
        if cursor.full {
            // Keep ID internally even if the caller excludes it from projection.
            let mut full_fields = fields.clone();
            if !full_fields.iter().any(|s| s == "id") {
                full_fields.push("id".into());
            }
            let page = query::tasks(
                q,
                db,
                client,
                auth,
                QueryArgs {
                    filters: Some(filters.clone()),
                    projection: Some(full_fields),
                    limit: Some(limit),
                    cursor: cursor.page.clone(),
                    snapshot: if cursor.page.is_none() {
                        cursor.snapshot.clone()
                    } else {
                        None
                    },
                },
            )
            .map_err(|e| {
                if ["snapshot_expired", "snapshot_invalid", "cursor_invalid"]
                    .contains(&e.code.as_str())
                {
                    McpError::new("resync_required")
                } else {
                    e
                }
            })?;
            cursor.basis = serde_json::from_value(page["meta"]["dataVersion"].clone())
                .map_err(|_| McpError::new("internal_error"))?;
            cursor.after = cursor.basis.commit_sequence as i64;
            cursor.snapshot = page["meta"]["snapshotId"].as_str().map(String::from);
            for mut row in page["items"]
                .as_array()
                .ok_or_else(|| McpError::new("internal_error"))?
                .clone()
            {
                let id = row["id"]
                    .as_i64()
                    .ok_or_else(|| McpError::new("internal_error"))?;
                cursor.seen.insert(id);
                if !fields.iter().any(|f| f == "id") {
                    row.as_object_mut().unwrap().remove("id");
                }
                items.push(json!({"kind":"current","taskId":id,"currentTask":row}));
            }
            cursor.page = page["nextCursor"].as_str().map(String::from);
            has_more = cursor.page.is_some();
            if !has_more {
                cursor.full = false;
                cursor.snapshot = None;
                cursor.expires = now + 86400;
                if since.is_some() {
                    cursor.after = cursor.floor;
                }
            }
        } else {
            // One immutable image for current bodies and the event watermark in this page.
            let frozen = db.mcp_snapshot()?;
            let _ = frozen.mcp_change_index()?;
            let basis = frozen.mcp_basis()?;
            let watermark = basis.commit_sequence as i64;
            if basis.database_uuid != cursor.basis.database_uuid
                || basis.data_generation != cursor.basis.data_generation
            {
                return Err(McpError::new("resync_required"));
            }
            let events = frozen.mcp_changes_after(cursor.after, watermark, 1000)?;
            let tasks = frozen.mcp_tasks()?;
            let map: HashMap<_, _> = tasks.iter().map(|t| (t.id, t)).collect();
            let mut positions = HashMap::new();
            for event in &events {
                if since.is_some_and(|since| {
                    event["at"]
                        .as_str()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .is_some_and(|at| at < since)
                }) {
                    cursor.after = event["sequence"].as_i64().unwrap();
                    continue;
                }
                let id = event["taskId"]
                    .as_i64()
                    .ok_or_else(|| McpError::new("internal_error"))?;
                let task = map.get(&id).copied();
                let allowed = if let Some(t) = task {
                    auth.scope.allows(
                        &serde_json::to_string(&t.departments).unwrap(),
                        &t.task_type,
                    ) && query::matches_task(&frozen, t, &filters, auth)?
                } else {
                    false
                };
                let output = if allowed {
                    let t = task.unwrap();
                    let mut value =
                        serde_json::to_value(t).map_err(|_| McpError::new("internal_error"))?;
                    let version = frozen.mcp_task_basis(id, auth.permissions.full_read)?;
                    value["taskVersion"] = version["taskVersion"].clone();
                    value["fieldVersions"] = version["fieldVersions"].clone();
                    value
                        .as_object_mut()
                        .unwrap()
                        .retain(|k, _| fields.contains(k));
                    // Parent IDs outside scope are never released.
                    if t.parent_task_id.is_some_and(|p| {
                        !map.get(&p).is_some_and(|p| {
                            auth.scope.allows(
                                &serde_json::to_string(&p.departments).unwrap(),
                                &p.task_type,
                            )
                        })
                    }) {
                        value
                            .as_object_mut()
                            .unwrap()
                            .insert("parentTaskId".into(), Value::Null);
                    }
                    Some(
                        json!({"kind":if mode=="events"{"event"}else{"current"},"event":event,"taskId":id,"currentTask":value,"stateAt":"page_watermark"}),
                    )
                } else if cursor.seen.contains(&id) {
                    Some(json!({"kind":"removed","taskId":id}))
                } else {
                    None
                };
                let existing = positions.get(&id).copied();
                if output.is_some()
                    && items.len() >= limit as usize
                    && (mode == "events" || existing.is_none())
                {
                    has_more = true;
                    break;
                }
                if let Some(output) = output {
                    if mode == "current" {
                        if let Some(at) = existing {
                            items[at] = output;
                        } else {
                            positions.insert(id, items.len());
                            items.push(output);
                        }
                    } else {
                        items.push(output);
                    }
                    if allowed {
                        cursor.seen.insert(id);
                    } else {
                        cursor.seen.remove(&id);
                    }
                }
                cursor.after = event["sequence"].as_i64().unwrap();
            }
            if !has_more {
                has_more = events.len() == 1000;
                if !has_more {
                    cursor.after = watermark;
                }
            }
            cursor.expires = now + 86400;
        }
        if cursor.seen.len() > 100_000 {
            return Err(McpError::new("resource_limit"));
        }
        // Per-client lease bound as well as a global disk budget.
        let owned = durable::ids(&root)?
            .into_iter()
            .filter(|id| {
                durable::read::<Cursor>(&root, id, 2 * 1024 * 1024)
                    .is_ok_and(|c| c.client == client)
            })
            .count();
        if owned >= 32 {
            return Err(McpError::new("resource_limit"));
        }
        let token = platform::random_secret()?;
        let old_predecessor = cursor.predecessor.clone();
        cursor.predecessor = args.cursor.clone();
        durable::write(
            &root,
            &token,
            &cursor,
            2 * 1024 * 1024,
            16 * 1024 * 1024,
            256,
        )?;
        if let Some(old) = old_predecessor {
            let _ = durable::remove(&root, &old);
        }
        let result = json!({"items":items,"nextCursor":token,"hasMore":has_more,"phase":if was_full{"fullSync"}else{"incremental"},"fullSyncComplete":!cursor.full,"watermark":cursor.after,"meta":{"dataVersion":cursor.basis,"historyFloor":cursor.floor,"knownGaps":["Events before this index's historyFloor are unavailable; historical task bodies are not reconstructed"],"mode":mode,"stateAt":"page_watermark"}});
        if serde_json::to_vec(&result).unwrap().len() > 7 * 1024 * 1024 {
            return Err(McpError::new("resource_limit"));
        }
        Ok(result)
    }
}
