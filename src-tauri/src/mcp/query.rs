//! Authorized P2 reads: immutable SQLite images, opaque client-bound cursors,
//! field projection, and independently scoped named query definitions.
#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;
use super::{basis::DataVersion, contract::*, platform, query_types::*, security::Authorization};
use crate::{
    database::Database,
    models::{LegalTask, ALL_STATUSES, PRIORITIES, WORKLOADS},
};
use chrono::{DateTime, FixedOffset, Local, NaiveDate, TimeZone, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::{Duration, Instant},
};

const TTL: Duration = Duration::from_secs(600);
const REGULAR: &[&str] = &[
    "taskVersion",
    "fieldVersions",
    "id",
    "permanentNumber",
    "dailySequence",
    "ticketDate",
    "departments",
    "taskType",
    "title",
    "status",
    "priority",
    "workload",
    "isUrgent",
    "requestedDeadline",
    "requestedDeadlineLabel",
    "createdAt",
    "updatedAt",
    "startedAt",
    "completedAt",
    "archivedAt",
    "deletedAt",
    "customSortOrder",
    "processingRounds",
    "hasActiveQueue",
    "parentTaskId",
    "subtaskSortOrder",
    "plannedDate",
    "isScheduled",
    "scheduleAction",
    "scheduleActionAt",
    "ticketColor",
];
const FULL: &[&str] = &[
    "contacts",
    "contact",
    "details",
    "internalNotes",
    "urgentRequester",
    "urgentReason",
];

struct Snapshot {
    db: Database,
    basis: DataVersion,
    client: String,
    revision: u64,
    hash: String,
    ids: Vec<i64>,
    created: Instant,
    expires: i64,
    cursors: HashMap<String, (usize, i64)>,
    rows: Option<Vec<Value>>,
    summary: Option<Value>,
    bytes: usize,
}
pub struct QueryState {
    root: PathBuf,
    snapshots: HashMap<String, Snapshot>,
}
impl QueryState {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            snapshots: HashMap::new(),
        }
    }
    fn begin(
        &mut self,
        db: &Database,
        client: &str,
        auth: &Authorization,
        hash: String,
        filters: &QueryFilters,
    ) -> Result<String, McpError> {
        self.snapshots
            .retain(|_, s| s.created.elapsed() < TTL && s.revision == auth.revision);
        let current = db.mcp_basis()?;
        if let Some((id, _)) = self
            .snapshots
            .iter()
            .find(|(_, s)| s.client == client && s.hash == hash && s.basis == current)
        {
            return Ok(id.clone());
        }
        if self.snapshots.len() >= 16
            || self
                .snapshots
                .values()
                .filter(|s| s.client == client)
                .count()
                >= 4
        {
            return Err(McpError::new("resource_limit"));
        }
        let frozen = db.mcp_snapshot()?;
        let basis = frozen.mcp_basis()?;
        let bytes = frozen.mcp_size()?;
        if self.snapshots.values().map(|s| s.bytes).sum::<usize>() + bytes > 64 * 1024 * 1024 {
            return Err(McpError::new("resource_limit"));
        }
        let tasks = frozen.mcp_tasks()?;
        let allowed: HashSet<i64> = tasks
            .iter()
            .filter(|t| {
                auth.scope.allows(
                    &serde_json::to_string(&t.departments).unwrap(),
                    &t.task_type,
                )
            })
            .map(|t| t.id)
            .collect();
        let mut ids = vec![];
        if filters
            .parent_task_id
            .is_some_and(|id| !allowed.contains(&id))
        {
            return Err(McpError::new("not_found"));
        }
        let mut dependencies = HashSet::new();
        for task in &tasks {
            if !allowed.contains(&task.id) || !matches_task(&frozen, task, filters, auth)? {
                continue;
            }
            ids.push(task.id);
            dependencies.insert(task.id);
            if let Some(parent) = task.parent_task_id {
                if allowed.contains(&parent) {
                    dependencies.insert(parent);
                }
            }
        }
        frozen.mcp_keep_tasks(&ids)?;
        let id = platform::random_secret()?;
        self.snapshots.insert(
            id.clone(),
            Snapshot {
                db: frozen,
                basis,
                client: client.into(),
                revision: auth.revision,
                hash,
                ids: dependencies.into_iter().collect(),
                created: Instant::now(),
                expires: Utc::now().timestamp() + 600,
                cursors: HashMap::new(),
                rows: None,
                summary: None,
                bytes,
            },
        );
        Ok(id)
    }
    fn resolve(
        &mut self,
        db: &Database,
        client: &str,
        auth: &Authorization,
        hash: &str,
        snapshot: Option<&str>,
        cursor: Option<&str>,
        limit: i64,
    ) -> Result<(String, usize), McpError> {
        let id = if let Some(id) = snapshot {
            id.to_string()
        } else if let Some(token) = cursor {
            self.snapshots
                .iter()
                .find(|(_, s)| s.cursors.contains_key(token))
                .map(|(id, _)| id.clone())
                .ok_or_else(|| McpError::new("cursor_invalid"))?
        } else {
            return Err(McpError::new("cursor_invalid"));
        };
        let s = self
            .snapshots
            .get(&id)
            .ok_or_else(|| McpError::new("snapshot_expired"))?;
        if s.client != client || s.hash != hash {
            return Err(McpError::new("cursor_invalid"));
        }
        if s.revision != auth.revision {
            return Err(McpError::new("authorization_changed"));
        }
        if s.created.elapsed() >= TTL {
            self.snapshots.remove(&id);
            return Err(McpError::new("snapshot_expired"));
        }
        let live = db.mcp_basis()?;
        if live.database_uuid != s.basis.database_uuid
            || live.data_generation != s.basis.data_generation
            || !db.mcp_in_scope(&s.ids, &auth.scope)?
        {
            return Err(McpError::new("snapshot_invalid"));
        }
        let offset = if let Some(token) = cursor {
            let (offset, page_size) = s
                .cursors
                .get(token)
                .ok_or_else(|| McpError::new("cursor_invalid"))?;
            if *page_size != limit {
                return Err(McpError::new("cursor_invalid"));
            }
            *offset
        } else {
            0
        };
        Ok((id, offset))
    }
    fn page(&mut self, id: &str, offset: usize, limit: i64) -> Result<Value, McpError> {
        let s = self
            .snapshots
            .get_mut(id)
            .ok_or_else(|| McpError::new("snapshot_expired"))?;
        let rows = s
            .rows
            .as_ref()
            .ok_or_else(|| McpError::new("internal_error"))?;
        if offset > rows.len() {
            return Err(McpError::new("invalid_arguments"));
        }
        let end = offset.saturating_add(limit as usize).min(rows.len());
        let items = rows[offset..end].to_vec();
        let next = if end < rows.len() {
            if s.cursors.len() >= 4096 {
                return Err(McpError::new("resource_limit"));
            }
            let token = platform::random_secret()?;
            s.cursors.insert(token.clone(), (end, limit));
            Some(token)
        } else {
            None
        };
        let result = json!({"items":items,"total":rows.len(),"offset":offset,"limit":limit,"hasMore":next.is_some(),"nextCursor":next,"meta":meta(id,s)});
        if serde_json::to_vec(&result)
            .map_err(|_| McpError::new("internal_error"))?
            .len()
            > 7 * 1024 * 1024
        {
            return Err(McpError::new("resource_limit"));
        }
        Ok(result)
    }
}

fn meta(id: &str, s: &Snapshot) -> Value {
    json!({"snapshotId":id,"dataVersion":s.basis,"authorizationRevision":s.revision,"expiresAt":s.expires,"basis":"immutable_in_memory_sqlite","complete":false,"knownGaps":["Historical records predating event tracking may be absent; original free-form logs are not structured business events"],"cache":{"reused":false},"querySchemaVersion":1,"statisticsDefinitionVersion":1})
}
fn hash(value: Value) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(&value).unwrap()))
}
fn limit(value: Option<i64>) -> Result<i64, McpError> {
    let n = value.unwrap_or(100);
    if !(1..=100).contains(&n) {
        Err(McpError::new("invalid_arguments"))
    } else {
        Ok(n)
    }
}
fn invalid() -> McpError {
    McpError::new("invalid_arguments")
}
fn projection(fields: Option<&Vec<String>>, auth: &Authorization) -> Result<Vec<String>, McpError> {
    let list = fields.cloned().unwrap_or_else(|| {
        REGULAR
            .iter()
            .chain(if auth.permissions.full_read {
                FULL.iter()
            } else {
                [].iter()
            })
            .map(|s| s.to_string())
            .collect()
    });
    if list.len() > 40 || list.iter().collect::<HashSet<_>>().len() != list.len() {
        return Err(invalid());
    }
    for field in &list {
        if FULL.contains(&field.as_str()) && !auth.permissions.full_read {
            return Err(McpError::new("forbidden"));
        }
        if !REGULAR.contains(&field.as_str()) && !FULL.contains(&field.as_str()) {
            return Err(invalid());
        }
    }
    Ok(list)
}
fn validate(f: &QueryFilters, auth: &Authorization) -> Result<(), McpError> {
    if f.ids
        .as_ref()
        .is_some_and(|ids| ids.len() > 100 || ids.iter().any(|id| *id <= 0))
    {
        return Err(invalid());
    }
    for list in [
        &f.permanent_numbers,
        &f.statuses,
        &f.departments,
        &f.task_types,
        &f.priorities,
        &f.workloads,
    ]
    .into_iter()
    .flatten()
    {
        if list.len() > 100
            || list
                .iter()
                .any(|s| s.is_empty() || s.len() > 400 || s.chars().any(char::is_control))
        {
            return Err(invalid());
        }
    }
    for (values, allowed) in [
        (&f.statuses, ALL_STATUSES.as_slice()),
        (&f.priorities, PRIORITIES.as_slice()),
        (&f.workloads, WORKLOADS.as_slice()),
    ] {
        if values
            .as_ref()
            .is_some_and(|v| v.iter().any(|s| !allowed.contains(&s.as_str())))
        {
            return Err(invalid());
        }
    }
    if f.text
        .as_ref()
        .is_some_and(|s| s.len() > 400 || s.chars().any(char::is_control))
    {
        return Err(invalid());
    }
    if f.structure
        .as_ref()
        .is_some_and(|s| !["all", "topLevel", "subtask"].contains(&s.as_str()))
        || f.archive
            .as_ref()
            .is_some_and(|s| !["all", "active", "archived"].contains(&s.as_str()))
        || f.parent_task_id.is_some_and(|id| id <= 0)
    {
        return Err(invalid());
    }
    if f.include_trash && !auth.permissions.full_read {
        return Err(McpError::new("forbidden"));
    }
    for value in [&f.planned_from, &f.planned_to].into_iter().flatten() {
        let day = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| invalid())?;
        if day.format("%Y-%m-%d").to_string() != *value {
            return Err(invalid());
        }
    }
    if let (Some(a), Some(b)) = (&f.planned_from, &f.planned_to) {
        if b < a {
            return Err(invalid());
        }
    }
    for (from, to) in [
        (&f.created_from, &f.created_to),
        (&f.deadline_from, &f.deadline_to),
        (&f.handled_from, &f.handled_to),
    ] {
        let a = from
            .as_ref()
            .map(|s| DateTime::parse_from_rfc3339(s).map_err(|_| invalid()))
            .transpose()?;
        let b = to
            .as_ref()
            .map(|s| DateTime::parse_from_rfc3339(s).map_err(|_| invalid()))
            .transpose()?;
        if matches!((a,b),(Some(a),Some(b)) if b<=a) {
            return Err(invalid());
        }
    }
    Ok(())
}
fn time_matches(value: Option<&str>, from: &Option<String>, to: &Option<String>) -> bool {
    if from.is_none() && to.is_none() {
        return true;
    }
    let Some(at) = value.and_then(|s| DateTime::parse_from_rfc3339(s).ok()) else {
        return false;
    };
    from.as_ref()
        .is_none_or(|s| DateTime::parse_from_rfc3339(s).is_ok_and(|f| at >= f))
        && to
            .as_ref()
            .is_none_or(|s| DateTime::parse_from_rfc3339(s).is_ok_and(|t| at < t))
}
fn contains(list: &Option<Vec<String>>, value: &str) -> bool {
    list.as_ref()
        .is_none_or(|values| values.iter().any(|v| v == value))
}
fn matches_task(
    db: &Database,
    t: &LegalTask,
    f: &QueryFilters,
    auth: &Authorization,
) -> Result<bool, McpError> {
    let archived = t.archived_at.is_some()
        || ["completed", "cancelled", "archived"].contains(&t.status.as_str());
    if (!f.include_trash && t.deleted_at.is_some())
        || f.ids.as_ref().is_some_and(|ids| !ids.contains(&t.id))
        || !contains(&f.permanent_numbers, &t.permanent_number)
        || !contains(&f.statuses, &t.status)
        || !contains(&f.task_types, &t.task_type)
        || !contains(&f.priorities, &t.priority)
        || !contains(&f.workloads, &t.workload)
        || f.departments
            .as_ref()
            .is_some_and(|dep| !t.departments.iter().any(|d| dep.contains(d)))
        || f.urgent.is_some_and(|v| v != t.is_urgent)
        || f.scheduled.is_some_and(|v| v != t.is_scheduled)
        || f.active_queue.is_some_and(|v| v != t.has_active_queue)
        || f.parent_task_id
            .is_some_and(|id| Some(id) != t.parent_task_id)
    {
        return Ok(false);
    }
    if f.structure.as_deref() == Some("topLevel") && t.parent_task_id.is_some()
        || f.structure.as_deref() == Some("subtask") && t.parent_task_id.is_none()
        || f.archive.as_deref() == Some("active") && archived
        || f.archive.as_deref() == Some("archived") && !archived
    {
        return Ok(false);
    }
    if f.planned_from.as_ref().is_some_and(|d| t.planned_date < *d)
        || f.planned_to.as_ref().is_some_and(|d| t.planned_date > *d)
        || !time_matches(Some(&t.created_at), &f.created_from, &f.created_to)
        || !time_matches(
            t.requested_deadline.as_deref(),
            &f.deadline_from,
            &f.deadline_to,
        )
    {
        return Ok(false);
    }
    if let Some(text) = &f.text {
        let needle = text.to_lowercase();
        let mut text = format!("{} {}", t.title, t.permanent_number);
        if auth.permissions.full_read {
            text.push_str(&format!(
                " {} {} {}",
                t.details,
                t.contacts.join(" "),
                t.internal_notes
            ));
        }
        if !text.to_lowercase().contains(&needle) {
            return Ok(false);
        }
    }
    if f.handled_from.is_some() || f.handled_to.is_some() {
        return Ok(db.mcp_history(t.id, false, false, true)?.iter().any(|e| {
            e["kind"] == "work" && time_matches(e["at"].as_str(), &f.handled_from, &f.handled_to)
        }));
    }
    Ok(true)
}
fn ensure_rows_budget(rows: &[Value]) -> Result<usize, McpError> {
    let bytes = serde_json::to_vec(rows).map_err(|_| invalid())?.len();
    if rows.len() > 100_000 || bytes > 16 * 1024 * 1024 {
        Err(McpError::new("resource_limit"))
    } else {
        Ok(bytes)
    }
}

pub fn tasks(
    state: &mut QueryState,
    db: &Database,
    client: &str,
    auth: &Authorization,
    args: QueryArgs,
) -> Result<Value, McpError> {
    let f = args.filters.unwrap_or_default();
    validate(&f, auth)?;
    let fields = projection(args.projection.as_ref(), auth)?;
    let n = limit(args.limit)?;
    let key = hash(json!(["tasks", f, fields]));
    let (id, offset) = if args.snapshot.is_some() || args.cursor.is_some() {
        state.resolve(
            db,
            client,
            auth,
            &key,
            args.snapshot.as_deref(),
            args.cursor.as_deref(),
            n,
        )?
    } else {
        (state.begin(db, client, auth, key, &f)?, 0)
    };
    let remaining = 64 * 1024 * 1024 - state.snapshots.values().map(|s| s.bytes).sum::<usize>();
    let s = state.snapshots.get_mut(&id).unwrap();
    if s.rows.is_none() {
        let tasks = s.db.mcp_tasks()?;
        let available: HashSet<_> = s.ids.iter().copied().collect();
        let mut rows = vec![];
        for mut task in tasks {
            if task
                .parent_task_id
                .is_some_and(|id| !available.contains(&id))
            {
                task.parent_task_id = None;
            }
            let mut raw = serde_json::to_value(&task).map_err(|_| invalid())?;
            let basis = s.db.mcp_task_basis(task.id, auth.permissions.full_read)?;
            raw["taskVersion"] = basis["taskVersion"].clone();
            raw["fieldVersions"] = basis["fieldVersions"].clone();
            let mut row = serde_json::Map::new();
            for field in &fields {
                row.insert(field.clone(), raw[field].clone());
            }
            rows.push(Value::Object(row));
        }
        let row_bytes = ensure_rows_budget(&rows)?;
        if row_bytes > remaining {
            return Err(McpError::new("resource_limit"));
        }
        s.bytes += row_bytes;
        s.rows = Some(rows);
    }
    let mut result = state.page(&id, offset, n)?;
    result["projection"] = json!(fields);
    result["redactedFields"] = json!(if auth.permissions.full_read {
        &[][..]
    } else {
        FULL
    });
    result["analysisPolicy"]=json!("Use returned facts for related-item analysis; label inferences separately; no relationships are persisted");
    Ok(result)
}

pub fn history(
    state: &mut QueryState,
    db: &Database,
    client: &str,
    auth: &Authorization,
    args: HistoryArgs,
) -> Result<Value, McpError> {
    if args.task_id <= 0 {
        return Err(invalid());
    }
    if args.include_voided && !auth.permissions.full_read {
        return Err(McpError::new("forbidden"));
    }
    if args.kinds.as_ref().is_some_and(|k| {
        k.len() > 5
            || k.iter().any(|s| {
                !["status", "queue", "work", "log", "urgent", "audit"].contains(&s.as_str())
            })
    }) {
        return Err(invalid());
    }
    let f = QueryFilters {
        ids: Some(vec![args.task_id]),
        include_trash: args.include_trash,
        ..Default::default()
    };
    validate(&f, auth)?;
    let n = limit(args.limit)?;
    let key = hash(json!([
        "history",
        args.task_id,
        args.kinds,
        args.include_voided,
        args.include_trash
    ]));
    let (id, offset) = if args.snapshot.is_some() || args.cursor.is_some() {
        state.resolve(
            db,
            client,
            auth,
            &key,
            args.snapshot.as_deref(),
            args.cursor.as_deref(),
            n,
        )?
    } else {
        (state.begin(db, client, auth, key, &f)?, 0)
    };
    let remaining = 64 * 1024 * 1024 - state.snapshots.values().map(|s| s.bytes).sum::<usize>();
    let s = state.snapshots.get_mut(&id).unwrap();
    if s.db.mcp_tasks()?.is_empty() {
        return Err(McpError::new("not_found"));
    }
    if s.rows.is_none() {
        let rows =
            s.db.mcp_history(
                args.task_id,
                auth.permissions.full_read,
                args.include_voided,
                auth.scope.departments.is_some() || auth.scope.task_types.is_some(),
            )?
            .into_iter()
            .filter(|e| {
                args.kinds
                    .as_ref()
                    .is_none_or(|k| k.iter().any(|kind| e["kind"] == *kind))
            })
            .collect::<Vec<_>>();
        let row_bytes = ensure_rows_budget(&rows)?;
        if row_bytes > remaining {
            return Err(McpError::new("resource_limit"));
        }
        s.bytes += row_bytes;
        s.rows = Some(rows);
    }
    let mut page = state.page(&id, offset, n)?;
    page["redactedFields"] = json!(if auth.permissions.full_read {
        vec![]
    } else {
        vec!["text", "log"]
    });
    Ok(page)
}

fn range(start: &str, end: &str, offset: Option<i32>) -> Result<(String, String, i32), McpError> {
    let a = NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|_| invalid())?;
    let b = NaiveDate::parse_from_str(end, "%Y-%m-%d").map_err(|_| invalid())?;
    if a.format("%Y-%m-%d").to_string() != start
        || b.format("%Y-%m-%d").to_string() != end
        || b < a
        || (b - a).num_days() > 36_500
    {
        return Err(invalid());
    }
    let minutes = offset.unwrap_or_else(|| Local::now().offset().local_minus_utc() / 60);
    let timezone = FixedOffset::east_opt(minutes.checked_mul(60).ok_or_else(invalid)?)
        .filter(|_| (-840..=840).contains(&minutes))
        .ok_or_else(invalid)?;
    let a = timezone
        .from_local_datetime(&a.and_hms_opt(0, 0, 0).ok_or_else(invalid)?)
        .single()
        .ok_or_else(invalid)?;
    let b = timezone
        .from_local_datetime(
            &b.succ_opt()
                .ok_or_else(invalid)?
                .and_hms_opt(0, 0, 0)
                .ok_or_else(invalid)?,
        )
        .single()
        .ok_or_else(invalid)?;
    Ok((a.to_rfc3339(), b.to_rfc3339(), minutes))
}
pub fn report(
    state: &mut QueryState,
    db: &Database,
    client: &str,
    auth: &Authorization,
    args: ReportItemsArgs,
    summary: bool,
) -> Result<Value, McpError> {
    let f = args.filters.unwrap_or_default();
    validate(&f, auth)?;
    if f.include_trash {
        return Err(invalid());
    }
    let n = limit(args.limit)?;
    let (start, end, tz) = range(
        &args.start_date,
        &args.end_date,
        args.timezone_offset_minutes,
    )?;
    if args.offset.is_some_and(|o| o < 0)
        || args.cursor.is_some() && args.offset.is_some()
        || args.offset.unwrap_or(0) > 0 && args.snapshot.is_none()
        || summary && (args.offset.is_some() || args.cursor.is_some())
    {
        return Err(invalid());
    }
    let key = hash(json!(["report", f, start, end, tz]));
    let (id, offset) = if args.snapshot.is_some() || args.cursor.is_some() {
        state.resolve(
            db,
            client,
            auth,
            &key,
            args.snapshot.as_deref(),
            args.cursor.as_deref(),
            n,
        )?
    } else {
        (state.begin(db, client, auth, key, &f)?, 0)
    };
    let s = state.snapshots.get_mut(&id).unwrap();
    if summary {
        let statistics = s.db.statistics_scoped(start, end, tz, auth.scope.clone())?;
        return Ok(
            json!({"startDate":args.start_date,"endDate":args.end_date,"timezoneOffsetMinutes":tz,"statistics":statistics,"meta":meta(&id,s),"definition":{"aggregation":"last valid event per task in range; trend deduplication per existing UI bucket; archived included; trash and voided excluded","attribution":"current task type and departments at snapshot capture","departmentCountsMayOverlap":true},"coverage":{"startDate":args.start_date,"endDate":args.end_date,"scope":auth.scope,"filters":f}}),
        );
    }
    let offset = args.offset.map(|n| n as usize).unwrap_or(offset);
    // Keep SQL pagination on the frozen database; an item can contain many work events.
    let mut page =
        s.db.report_items_scoped(start, end, n, offset as i64, auth.scope.clone())?;
    let mut value = serde_json::to_value(&page).map_err(|_| invalid())?;
    if !auth.permissions.full_read {
        for item in &mut page.items {
            for event in &mut item.work_events {
                event.note.clear();
            }
        }
        value = serde_json::to_value(page).map_err(|_| invalid())?;
        for item in value["items"].as_array_mut().unwrap() {
            for event in item["workEvents"].as_array_mut().unwrap() {
                event.as_object_mut().unwrap().remove("note");
            }
        }
    }
    let next_offset = offset + value["items"].as_array().unwrap().len();
    let next = if value["hasMore"] == true {
        if s.cursors.len() >= 4096 {
            return Err(McpError::new("resource_limit"));
        }
        let token = platform::random_secret()?;
        s.cursors.insert(token.clone(), (next_offset, n));
        Some(token)
    } else {
        None
    };
    if serde_json::to_vec(&value).map_err(|_| invalid())?.len() > 7 * 1024 * 1024 {
        return Err(McpError::new("resource_limit"));
    }
    Ok(
        json!({"startDate":args.start_date,"endDate":args.end_date,"timezoneOffsetMinutes":tz,"page":value,"nextCursor":next,"meta":meta(&id,s),"redactedFields":if auth.permissions.full_read{vec![]}else{vec!["workEvents.note"]}}),
    )
}
pub fn calendar(
    state: &mut QueryState,
    db: &Database,
    client: &str,
    auth: &Authorization,
    args: CalendarArgs,
) -> Result<Value, McpError> {
    let f = args.filters.unwrap_or_default();
    validate(&f, auth)?;
    if f.include_trash {
        return Err(invalid());
    }
    let n = limit(args.limit)?;
    let (start, end, tz) = range(
        &args.start_date,
        &args.end_date,
        args.timezone_offset_minutes,
    )?;
    let key = hash(json!(["calendar", f, start, end, tz]));
    let (id, offset) = if args.snapshot.is_some() || args.cursor.is_some() {
        state.resolve(
            db,
            client,
            auth,
            &key,
            args.snapshot.as_deref(),
            args.cursor.as_deref(),
            n,
        )?
    } else {
        (state.begin(db, client, auth, key, &f)?, 0)
    };
    let remaining = 64 * 1024 * 1024 - state.snapshots.values().map(|s| s.bytes).sum::<usize>();
    let s = state.snapshots.get_mut(&id).unwrap();
    if s.rows.is_none() {
        let cal = s.db.mcp_work_calendar(start.clone(), end.clone())?;
        let summary = serde_json::to_value(cal.summary).map_err(|_| invalid())?;
        // Include queue intervals even when the task has no handling event.
        let mut rows = cal
            .tasks
            .into_iter()
            .map(|t| {
                let mut value = serde_json::to_value(t).unwrap();
                value["kind"] = json!("queueTask");
                value
            })
            .collect::<Vec<_>>();
        rows.extend(cal.events.into_iter().map(|e| {
            let mut value = serde_json::to_value(e).unwrap();
            value["kind"] = json!("workEvent");
            value
        }));
        let row_bytes = ensure_rows_budget(&rows)?;
        if row_bytes > remaining {
            return Err(McpError::new("resource_limit"));
        }
        s.bytes += row_bytes;
        s.rows = Some(rows);
        s.summary = Some(summary);
    }
    let sum = s.summary.clone().unwrap();
    let mut page = state.page(&id, offset, n)?;
    page["summary"] = sum;
    page["range"] = json!({"start":start,"end":end,"timezoneOffsetMinutes":tz});
    Ok(page)
}

pub fn saved(
    state: &mut QueryState,
    auth: &Authorization,
    client: &str,
    args: SavedQueryArgs,
) -> Result<Value, McpError> {
    let mut all = read_saved(&state.root)?;
    let by_client = all
        .as_object_mut()
        .ok_or_else(|| McpError::new("security_unavailable"))?;
    let mut queries = by_client.get(client).cloned().unwrap_or_else(|| json!({}));
    let definitions = queries
        .as_object_mut()
        .ok_or_else(|| McpError::new("security_unavailable"))?;
    let writable = ["save", "delete"].contains(&args.action.as_str());
    if writable && (!auth.permissions.write || !args.explicit_intent) {
        return Err(McpError::new("explicit_intent_required"));
    }
    if args.filters.is_some() && !matches!(args.action.as_str(), "save")
        || args.projection.is_some() && !matches!(args.action.as_str(), "save")
    {
        return Err(invalid());
    }
    let name = if args.action == "list" {
        String::new()
    } else {
        let name = args.name.clone().ok_or_else(invalid)?;
        if name.is_empty()
            || name.trim() != name
            || name.len() > 160
            || name.chars().any(char::is_control)
        {
            return Err(invalid());
        }
        name
    };
    let result = match args.action.as_str() {
        "list" => {
            json!({"names":definitions.keys().collect::<Vec<_>>(),"policy":"Definitions do not confer access; every execution uses current authorization"})
        }
        "get" => {
            let value = definitions
                .get(&name)
                .ok_or_else(|| McpError::new("not_found"))?
                .clone();
            let f: QueryFilters =
                serde_json::from_value(value["filters"].clone()).map_err(|_| invalid())?;
            validate(&f, auth)?;
            let p: Vec<String> =
                serde_json::from_value(value["projection"].clone()).map_err(|_| invalid())?;
            projection(Some(&p), auth)?;
            json!({"name":name,"definition":value,"scope":auth.scope,"authorizationRevision":auth.revision})
        }
        "save" => {
            let f = args.filters.unwrap_or_default();
            validate(&f, auth)?;
            let p = projection(args.projection.as_ref(), auth)?;
            if definitions.len() >= 100 && !definitions.contains_key(&name) {
                return Err(McpError::new("resource_limit"));
            }
            definitions.insert(name.clone(), json!({"filters":f,"projection":p}));
            json!({"name":name,"saved":true})
        }
        "delete" => {
            definitions
                .remove(&name)
                .ok_or_else(|| McpError::new("not_found"))?;
            json!({"name":name,"deleted":true})
        }
        _ => return Err(invalid()),
    };
    if writable {
        by_client.insert(client.into(), queries);
        let bytes = serde_json::to_vec(&all).map_err(|_| invalid())?;
        if bytes.len() > 1024 * 1024 {
            return Err(McpError::new("resource_limit"));
        }
        let path = state.root.join("saved-queries.json");
        platform::check_path(&path)?;
        let temp = state
            .root
            .join(format!("saved-queries-{}.tmp", platform::random_secret()?));
        std::fs::write(&temp, bytes).map_err(|_| McpError::new("security_unavailable"))?;
        platform::Descriptor::new(false)?.apply(&temp)?;
        let result = platform::atomic_replace(&temp, &path);
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result?;
    }
    Ok(result)
}
fn read_saved(root: &std::path::Path) -> Result<Value, McpError> {
    let path = root.join("saved-queries.json");
    platform::check_path(&path)?;
    if !path.exists() {
        return Ok(json!({}));
    }
    platform::Descriptor::new(false)?.verify(&path)?;
    if std::fs::metadata(&path)
        .map_err(|_| McpError::new("security_unavailable"))?
        .len()
        > 1024 * 1024
    {
        return Err(McpError::new("resource_limit"));
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|_| McpError::new("security_unavailable"))?)
        .map_err(|_| McpError::new("security_unavailable"))
}
