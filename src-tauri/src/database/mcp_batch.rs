//! Atomic P4 batches; simulations always roll back, including number and version allocation.
use super::mcp_write::*;
use super::*;
use crate::mcp::{
    basis::DataVersion,
    contract::McpError,
    security::{Authorization, Store},
    write_types::*,
};
use serde_json::{json, Value as Json};

fn basis(conn: &Connection) -> Result<DataVersion, McpError> {
    conn.query_row("SELECT database_uuid,data_generation,commit_sequence,(SELECT epoch FROM mcp_change_index WHERE singleton=1) FROM mcp_data_basis WHERE singleton=1",[],|r|Ok(DataVersion{database_uuid:r.get(0)?,data_generation:r.get(1)?,commit_sequence:r.get(2)?,index_epoch:r.get(3)?,schema_version:DATABASE_SCHEMA_VERSION as u32,query_schema_version:1,statistics_definition_version:1})).map_err(sql)
}
fn limit(conn: &Connection) -> Result<usize, McpError> {
    let s: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key='mcp_batch_limit'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?;
    Ok(s.and_then(|s| s.parse().ok())
        .filter(|n| (1..=100).contains(n))
        .unwrap_or(20))
}
fn budget(conn: &Connection, client: &str, count: usize) -> Result<(), McpError> {
    let stamp = Utc::now().timestamp();
    let (start, used): (i64, i64) = conn
        .query_row(
            "SELECT window_start,item_count FROM mcp_batch_usage WHERE client_id=?",
            [client],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(sql)?
        .unwrap_or((stamp, 0));
    let used = if stamp - start >= 60 { 0 } else { used };
    if used + count as i64 > 200 {
        return Err(err("rate_limited"));
    }
    conn.execute("INSERT INTO mcp_batch_usage VALUES(?,?,?) ON CONFLICT(client_id) DO UPDATE SET window_start=excluded.window_start,item_count=excluded.item_count",params![client,if used==0{stamp}else{start},used+count as i64]).map_err(sql)?;
    Ok(())
}
fn replace_refs(value: &mut Json, refs: &HashMap<String, (i64, i64)>) -> Result<(), McpError> {
    match value {
        Json::Object(m) => {
            if let Some(name) = m
                .get("batchRef")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
            {
                if m.iter().any(|(k, v)| k != "batchRef" && !v.is_null()) {
                    return Err(err("invalid_arguments"));
                }
                let (id, _) = refs.get(&name).ok_or_else(|| err("needs_disambiguation"))?;
                *value = json!({"taskId":id});
                return Ok(());
            }
            for v in m.values_mut() {
                replace_refs(v, refs)?;
            }
        }
        Json::Array(a) => {
            for v in a {
                replace_refs(v, refs)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn differences(
    conn: &Connection,
    audit_id: Option<i64>,
    auth: &Authorization,
) -> Result<Json, McpError> {
    let Some(id) = audit_id else {
        return Ok(json!([]));
    };
    let (b, a): (String, String) = conn
        .query_row(
            "SELECT before_json,after_json FROM mcp_ai_audit WHERE id=?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(sql)?;
    let b: Json = serde_json::from_str(&b).map_err(|_| err("internal_error"))?;
    let a: Json = serde_json::from_str(&a).map_err(|_| err("internal_error"))?;
    let mut rows = vec![];
    if let Some(fields) = a["task"].as_object() {
        for (key, v) in fields {
            if matches!(key.as_str(), "updatedAt" | "createdAt" | "scheduleActionAt") {
                continue;
            }
            if b["task"][key] != *v {
                let sensitive = matches!(
                    key.as_str(),
                    "details"
                        | "internalNotes"
                        | "contacts"
                        | "contact"
                        | "urgentRequester"
                        | "urgentReason"
                );
                rows.push(json!({"field":key,"before":if sensitive&&!auth.permissions.full_read{json!("[需完整读取权限]")}else{b["task"][key].clone()},"after":if sensitive&&!auth.permissions.full_read{json!("[需完整读取权限]")}else{v.clone()}}));
            }
        }
    }
    if let Some(tasks) = a["relatedTasks"].as_array() {
        for t in tasks {
            rows.push(json!({"field":"relatedTask","taskId":t["task"]["id"],"taskVersion":t["taskVersion"]}));
            let old = b["relatedTasks"]
                .as_array()
                .and_then(|values| {
                    values
                        .iter()
                        .find(|v| v["id"] == t["task"]["id"] || v["task"]["id"] == t["task"]["id"])
                })
                .cloned()
                .unwrap_or(Json::Null);
            let old = if old["task"].is_object() {
                old["task"].clone()
            } else {
                old
            };
            if let Some(fields) = t["task"].as_object() {
                for (field, new) in fields {
                    if old[field] != *new
                        && !matches!(
                            field.as_str(),
                            "updatedAt" | "createdAt" | "scheduleActionAt"
                        )
                    {
                        let sensitive = matches!(
                            field.as_str(),
                            "details"
                                | "internalNotes"
                                | "contacts"
                                | "contact"
                                | "urgentRequester"
                                | "urgentReason"
                        );
                        rows.push(json!({"field":field,"taskId":t["task"]["id"],"before":if sensitive&&!auth.permissions.full_read{json!("[需完整读取权限]")}else{old[field].clone()},"after":if sensitive&&!auth.permissions.full_read{json!("[需完整读取权限]")}else{new.clone()}}));
                    }
                }
            }
        }
    }
    let mut old_events = vec![];
    let mut new_events = vec![];
    if let Some(events) = a["workEvents"].as_array() {
        for event in events {
            let old = b["workEvents"]
                .as_array()
                .and_then(|values| values.iter().find(|v| v["id"] == event["id"]))
                .cloned()
                .unwrap_or(Json::Null);
            if old != *event {
                let mut old = old;
                let mut new = event.clone();
                if !auth.permissions.full_read {
                    if let Some(m) = old.as_object_mut() {
                        m.remove("note");
                    }
                    new.as_object_mut().unwrap().remove("note");
                }
                old_events.push(old);
                new_events.push(new);
            }
        }
    }
    if !new_events.is_empty() {
        rows.push(json!({"field":"workEvents","before":old_events,"after":new_events}));
    }
    Ok(json!(rows))
}
fn run(
    conn: &Connection,
    client: &str,
    auth: &Authorization,
    args: &BatchArgs,
) -> Result<Json, McpError> {
    let mut refs = HashMap::new();
    let mut items = vec![];
    let mut affected = HashSet::new();
    for (i, item) in args.operations.iter().enumerate() {
        let mut value = json!(item.operation);
        replace_refs(&mut value, &refs)?;
        // An explicit 0 version is accepted only for a freshly-created batch reference.
        for field in ["target", "parent"] {
            if let Some(name) = json!(item.operation)[field]["batchRef"].as_str() {
                let (_, v) = refs.get(name).ok_or_else(|| err("needs_disambiguation"))?;
                let key = if field == "parent" {
                    "parentVersion"
                } else {
                    "taskVersion"
                };
                if value[key].as_i64() != Some(0) {
                    return Err(err("invalid_arguments"));
                }
                value[key] = json!(v);
            }
        }
        let subkey = format!("batch:{}", fingerprint(&json!([args.idempotency_key, i])));
        let mutation = MutateArgs {
            idempotency_key: subkey,
            intent: args.intent.clone(),
            reason: args.reason.clone(),
            operation: serde_json::from_value(value).map_err(|_| err("invalid_arguments"))?,
        };
        let request = json!({"tool":"mutate_task","args":mutation});
        let mut result = mutate_on(conn, client, auth, &mutation, &request)?;
        result["differences"] = differences(conn, result["auditId"].as_i64(), auth)?;
        if let Some(id) = result["taskId"].as_i64() {
            affected.insert(id);
        }
        for row in result["differences"].as_array().unwrap() {
            if let Some(id) = row["taskId"].as_i64() {
                affected.insert(id);
            }
        }
        if affected.len() > limit(conn)? {
            return Err(err("resource_limit"));
        }
        if let Some(name) = &item.ref_key {
            if name.is_empty()
                || name.len() > 80
                || refs.contains_key(name)
                || !matches!(
                    item.operation,
                    TaskAction::Create { .. } | TaskAction::CreateSubtask { .. }
                )
            {
                return Err(err("invalid_arguments"));
            }
            let id = result["taskId"]
                .as_i64()
                .ok_or_else(|| err("invalid_arguments"))?;
            refs.insert(name.clone(), (id, version(conn, id)?));
        }
        items.push(result);
    }
    Ok(json!({"items":items,"refs":refs,"affectedCount":affected.len()}))
}
fn simulate(
    conn: &Connection,
    client: &str,
    auth: &Authorization,
    args: &BatchArgs,
) -> Result<Json, McpError> {
    conn.execute_batch("SAVEPOINT mcp_dry_run;").map_err(sql)?;
    let result = run(conn, client, auth, args);
    // Rollback even on a business error. If rollback fails, the enclosing transaction fails too.
    conn.execute_batch("ROLLBACK TO mcp_dry_run;RELEASE mcp_dry_run;")
        .map_err(sql)?;
    result
}
fn execute(
    conn: &Connection,
    client: &str,
    auth: &Authorization,
    args: &BatchArgs,
    request: &Json,
) -> Result<Json, McpError> {
    let mut result = run(conn, client, auth, args)?;
    budget(
        conn,
        client,
        args.operations
            .len()
            .max(result["affectedCount"].as_u64().unwrap() as usize),
    )?;
    let mut states = vec![];
    let mut versions = serde_json::Map::new();
    for item in result["items"].as_array().unwrap() {
        if let Some(id) = item["taskId"].as_i64() {
            versions.insert(id.to_string(), json!(version(conn, id)?));
            states.push(proof(conn, id)?);
        }
    }
    let operation_audits = result["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["auditId"].as_i64())
        .collect::<Vec<_>>();
    let audit_id = audit(
        conn,
        client,
        "batch",
        None,
        &args.intent,
        &args.reason,
        &json!({"operationAudits":operation_audits}),
        &json!({"relatedTasks":states}),
        request,
        None,
    )?;
    result["commitStatus"] = json!("committed");
    result["auditId"] = json!(audit_id);
    result["taskVersions"] = json!(versions);
    result["replayed"] = json!(false);
    // All indirect changes must remain in the root receipt's scope proof too.
    let audit_items = result["items"].as_array().unwrap().clone();
    for item in audit_items {
        if let Some(id) = item["auditId"].as_i64() {
            let raw: String = conn
                .query_row(
                    "SELECT after_json FROM mcp_ai_audit WHERE id=?",
                    [id],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            let a: Json = serde_json::from_str(&raw).map_err(|_| err("internal_error"))?;
            if let Some(related) = a["relatedTasks"].as_array() {
                for t in related {
                    let id = t["task"]["id"]
                        .as_i64()
                        .ok_or_else(|| err("internal_error"))?;
                    versions.insert(id.to_string(), json!(version(conn, id)?));
                    states.push(proof(conn, id)?);
                }
            }
        }
    }
    conn.execute(
        "UPDATE mcp_ai_audit SET after_json=? WHERE id=?",
        params![json!({"relatedTasks":states}).to_string(), audit_id],
    )
    .map_err(sql)?;
    result["taskVersions"] = json!(versions);
    Ok(result)
}

pub(super) fn undo_batch(conn: &Connection, before: &Json) -> Result<(), McpError> {
    for id in before["operationAudits"]
        .as_array()
        .ok_or_else(|| err("internal_error"))?
        .iter()
        .rev()
    {
        let (action, task, b, a): (String, i64, String, String) = conn
            .query_row(
                "SELECT action,task_id,before_json,after_json FROM mcp_ai_audit WHERE id=?",
                [id.as_i64().ok_or_else(|| err("internal_error"))?],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(sql)?;
        super::mcp_complex::undo(
            conn,
            &action,
            task,
            &serde_json::from_str::<Json>(&b).map_err(|_| err("internal_error"))?,
            &serde_json::from_str::<Json>(&a).map_err(|_| err("internal_error"))?,
        )?;
    }
    Ok(())
}
impl Database {
    pub fn mcp_batch_limit(&self) -> Result<usize, McpError> {
        self.with_conn(|conn| limit(conn).map_err(|e| e.to_string()))
            .map_err(|_| err("internal_error"))
    }
    pub fn mcp_batch(
        &self,
        client: &str,
        auth: &Authorization,
        args: BatchArgs,
    ) -> Result<Json, McpError> {
        if !auth.permissions.write || !auth.permissions.regular_read {
            return Err(err("forbidden"));
        }
        validate_intent(&args.idempotency_key, &args.intent, &args.reason)?;
        if !matches!(args.action.as_str(), "dryRun" | "apply") {
            return Err(err("invalid_arguments"));
        }
        let request = json!({"tool":"batch_mutate","args":args});
        self.mcp_transaction(|conn|{
            if args.action=="apply"{if let Some(r)=replay(conn,client,&args.idempotency_key,&request,auth)?{return Ok(r);}}
            if args.operations.is_empty()||args.operations.len()>limit(conn)?||serde_json::to_vec(&args).unwrap().len()>256*1024{return Err(err("resource_limit"));}
            let current=basis(conn)?;if args.expected_basis.as_ref().is_some_and(|v|*v!=current){return Err(err("conflict"));}
            // Validate the entire mixed request before publishing a proposal or performing any writes.
            let preview=simulate(conn,client,auth,&args)?;
            if args.action=="dryRun"{return Ok(json!({"commitStatus":"dry_run","basis":current,"preview":preview,"batchLimit":limit(conn)?}));}
            let date_targets: std::collections::HashSet<String> = args.operations.iter().filter_map(|v| match &v.operation {
                TaskAction::Reschedule { target, .. } => Some(serde_json::to_string(target).unwrap()),
                TaskAction::Patch { target, patch, .. } if patch.contains_key("requestedDeadline") => Some(serde_json::to_string(target).unwrap()),
                _ => None,
            }).collect();
            if date_targets.len()>1{
                conn.execute("INSERT INTO mcp_batch_approvals(client_id,request_key,request_json,basis_json,preview_json,created_at) VALUES(?,?,?,?,?,?)",params![client,args.idempotency_key,serde_json::to_string(&args).unwrap(),serde_json::to_string(&current).unwrap(),preview.to_string(),now()]).map_err(sql)?;
                let result=json!({"commitStatus":"needs_user_approval","approvalId":conn.last_insert_rowid(),"replayed":false});receipt(conn,client,&args.idempotency_key,&request,None,None,&result)?;return Ok(result);
            }
            let result=execute(conn,client,auth,&args,&request)?;receipt(conn,client,&args.idempotency_key,&request,None,result["auditId"].as_i64(),&result)?;Ok(result)
        })
    }
    pub fn mcp_batch_state(&self) -> Result<Json, McpError> {
        self.with_conn(|conn|{let mut stmt=conn.prepare("SELECT id,client_id,request_json,preview_json,created_at FROM mcp_batch_approvals WHERE status='pending' ORDER BY id").map_err(display_error)?;let rows=stmt.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?))).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;Ok(json!({"batchLimit":limit(conn).map_err(|e|e.to_string())?,"pending":rows.into_iter().map(|(id,client,req,preview,time)|json!({"id":id,"clientId":client,"request":serde_json::from_str::<Json>(&req).unwrap_or(Json::Null),"preview":serde_json::from_str::<Json>(&preview).unwrap_or(Json::Null),"createdAt":time})).collect::<Vec<_>>()}))}).map_err(|_|err("internal_error"))
    }
    pub fn mcp_set_batch_limit(&self, value: usize) -> Result<(), McpError> {
        if !(1..=100).contains(&value) {
            return Err(err("invalid_arguments"));
        }
        self.with_transaction(|tx|{tx.execute("INSERT INTO settings VALUES('mcp_batch_limit',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[value.to_string()]).map_err(display_error)?;Ok(())}).map_err(domain)
    }
    pub fn mcp_resolve_batch(
        &self,
        id: i64,
        approve: bool,
        store: &Store,
    ) -> Result<Json, McpError> {
        self.mcp_transaction(|conn|{
            let (client,raw,base,status):(String,String,String,String)=conn.query_row("SELECT client_id,request_json,basis_json,status FROM mcp_batch_approvals WHERE id=?",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|_|err("not_found"))?;
            if status!="pending"{return Ok(json!({"status":status}));}
            let args:BatchArgs=serde_json::from_str(&raw).map_err(|_|err("internal_error"))?;
            let result=if !approve{json!({"commitStatus":"rejected","status":"rejected"})}else{
                if store.paused{return Err(err("paused"));}
                let c=store.clients.iter().find(|c|c.id==client&&!c.revoked).ok_or_else(||err("revoked"))?;
                let auth=Authorization{permissions:store.groups.intersection(&c.permissions),scope:c.scope.clone(),revision:store.revision};
                if !auth.permissions.write||!auth.permissions.regular_read{return Err(err("forbidden"));}
                if basis(conn)?!=serde_json::from_str::<DataVersion>(&base).map_err(|_|err("internal_error"))?{return Err(err("conflict"));}
                if args.operations.len()>limit(conn)?{return Err(err("resource_limit"));}
                execute(conn,&client,&auth,&args,&json!({"tool":"batch_mutate","args":args}))?
            };
            conn.execute("UPDATE mcp_batch_approvals SET status=?,resolved_at=? WHERE id=?",params![if approve{"approved"}else{"rejected"},now(),id]).map_err(sql)?;
            conn.execute("UPDATE mcp_receipts SET audit_id=?,result_json=? WHERE client_id=? AND request_key=?",params![result["auditId"].as_i64(),result.to_string(),client,args.idempotency_key]).map_err(sql)?;Ok(result)
        })
    }
}
