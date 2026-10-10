//! Writes and receipts share the SAME SQLite transaction and shared GUI domain bodies.
use super::*;
use crate::mcp::{contract::McpError, security::Authorization, write_types::*};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};

use super::mcp_schema::FIELDS;

const PREFS: &[&str] = &[
    "ui_font_family",
    "ui_scale",
    "ticket_colors",
    "show_deferred_in_queue",
    "week_start_day",
    "statistics_rate_mode",
];
fn err(code: &str) -> McpError {
    McpError::new(code)
}
fn sql(_: rusqlite::Error) -> McpError {
    err("internal_error")
}
fn domain(error: String) -> McpError {
    // Existing domain errors are local validation messages, never SQL or data contents.
    let mut e = err("business_rule");
    if !error.contains("SQL") && !error.contains("sqlite") {
        e.message = error;
    }
    e
}

fn allowed(task: &LegalTask, auth: &Authorization) -> bool {
    auth.scope.allows(
        &serde_json::to_string(&task.departments).unwrap(),
        &task.task_type,
    )
}
fn related_scope(conn: &Connection, state: &Json, auth: &Authorization) -> Result<(), McpError> {
    if let Some(rows) = state["relatedTasks"].as_array() {
        for row in rows {
            let id = row["task"]["id"]
                .as_i64()
                .ok_or_else(|| err("internal_error"))?;
            if !allowed(&get_task_on(conn, id).map_err(|_| err("not_found"))?, auth) {
                return Err(err("forbidden"));
            }
        }
    }
    Ok(())
}
fn target(conn: &Connection, target: &Target, auth: &Authorization) -> Result<LegalTask, McpError> {
    if usize::from(target.task_id.is_some())
        + usize::from(target.permanent_number.is_some())
        + usize::from(target.title.is_some())
        != 1
    {
        return Err(err("invalid_arguments"));
    }
    let tasks = load_all_tasks(conn).map_err(domain)?;
    let matches: Vec<_> = tasks
        .into_iter()
        .filter(|t| allowed(t, auth) && t.deleted_at.is_none())
        .filter(|t| {
            target.task_id == Some(t.id)
                || target.permanent_number.as_ref() == Some(&t.permanent_number)
                || target.title.as_ref() == Some(&t.title)
        })
        .collect();
    if matches.len() > 1 {
        return Err(err("needs_disambiguation"));
    }
    matches.into_iter().next().ok_or_else(|| err("not_found"))
}
fn version(conn: &Connection, id: i64) -> Result<i64, McpError> {
    conn.query_row(
        "SELECT version FROM mcp_task_versions WHERE task_id=?",
        [id],
        |r| r.get(0),
    )
    .map_err(sql)
}
fn preference_version(conn: &Connection, key: &str) -> Result<i64, McpError> {
    Ok(conn
        .query_row(
            "SELECT version FROM mcp_preference_versions WHERE key=?",
            [key],
            |r| r.get(0),
        )
        .optional()
        .map_err(sql)?
        .unwrap_or(0))
}
fn preference_state(
    conn: &Connection,
    keys: impl Iterator<Item = String>,
) -> Result<Json, McpError> {
    let mut values = serde_json::Map::new();
    let mut versions = serde_json::Map::new();
    for k in keys {
        let value: Option<String> = conn
            .query_row("SELECT value FROM settings WHERE key=?", [&k], |r| r.get(0))
            .optional()
            .map_err(sql)?;
        values.insert(k.clone(), json!(value));
        versions.insert(k.clone(), json!(preference_version(conn, &k)?));
    }
    values.insert("fieldVersions".into(), json!(versions));
    Ok(json!(values))
}
fn field_versions(conn: &Connection, id: i64, full: bool) -> Result<Json, McpError> {
    let mut map = serde_json::Map::new();
    for (f, _) in FIELDS {
        if !full && matches!(*f, "contacts" | "details" | "internalNotes") {
            continue;
        }
        let n: i64 = conn
            .query_row(
                "SELECT version FROM mcp_field_versions WHERE task_id=? AND field=?",
                params![id, f],
                |r| r.get(0),
            )
            .map_err(sql)?;
        map.insert((*f).into(), json!(n));
    }
    Ok(Json::Object(map))
}
fn proof(conn: &Connection, id: i64) -> Result<Json, McpError> {
    let task = get_task_on(conn, id).map_err(domain)?;
    let mut statement = conn
        .prepare(
            "SELECT id FROM task_work_events WHERE task_id=? AND voided_at IS NULL ORDER BY id",
        )
        .map_err(sql)?;
    let events = statement
        .query_map([id], |r| r.get::<_, i64>(0))
        .map_err(sql)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql)?;
    Ok(
        json!({"task":task,"taskVersion":version(conn,id)?,"fieldVersions":field_versions(conn,id,true)?,"workEventIds":events}),
    )
}
fn validate_intent(key: &str, intent: &Intent, reason: &str) -> Result<(), McpError> {
    if !intent.explicit_user_request {
        return Err(err("explicit_intent_required"));
    }
    if key.is_empty()
        || key.len() > 128
        || key.chars().any(char::is_control)
        || intent.summary.trim().is_empty()
        || intent.summary.len() > 1000
        || reason.trim().is_empty()
        || reason.len() > 1000
    {
        return Err(err("invalid_arguments"));
    }
    Ok(())
}
fn fingerprint(value: &Json) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(value).unwrap()))
}
fn replay(
    conn: &Connection,
    client: &str,
    key: &str,
    request: &Json,
    auth: &Authorization,
) -> Result<Option<Json>, McpError> {
    let row:Option<(String,Option<i64>,String,Option<i64>)>=conn.query_row("SELECT request_hash,task_id,result_json,audit_id FROM mcp_receipts WHERE client_id=? AND request_key=?",params![client,key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(sql)?;
    if let Some((hash, id, receipt, audit_id)) = row {
        if let Some(id) = id {
            if !allowed(&get_task_on(conn, id).map_err(|_| err("not_found"))?, auth) {
                return Err(err("forbidden"));
            }
        }
        if let Some(aid) = audit_id {
            let (_, _, _, _, after) = audit_row(conn, aid)?;
            related_scope(conn, &after, auth)?;
        }
        if hash != fingerprint(request) {
            return Err(err("idempotency_conflict"));
        }
        let mut result: Json = serde_json::from_str(&receipt).map_err(|_| err("internal_error"))?;
        result["replayed"] = json!(true);
        return Ok(Some(result));
    }
    Ok(None)
}
fn receipt(
    conn: &Connection,
    client: &str,
    key: &str,
    request: &Json,
    id: Option<i64>,
    audit: Option<i64>,
    result: &Json,
) -> Result<(), McpError> {
    conn.execute(
        "INSERT INTO mcp_receipts VALUES(?,?,?,?,?,?)",
        params![
            client,
            key,
            fingerprint(request),
            id,
            audit,
            result.to_string()
        ],
    )
    .map_err(sql)?;
    Ok(())
}
fn audit(
    conn: &Connection,
    client: &str,
    action: &str,
    id: Option<i64>,
    intent: &Intent,
    reason: &str,
    before: &Json,
    after: &Json,
    request: &Json,
    undo: Option<i64>,
) -> Result<i64, McpError> {
    conn.execute("INSERT INTO mcp_ai_audit(client_id,action,task_id,reason,intent,before_json,after_json,request_json,created_at,undo_of) VALUES(?,?,?,?,?,?,?,?,?,?)",params![client,action,id,reason,intent.summary,before.to_string(),after.to_string(),request.to_string(),now(),undo]).map_err(sql)?;
    Ok(conn.last_insert_rowid())
}
fn task_input(task: &LegalTask) -> Result<TaskInput, McpError> {
    serde_json::from_value(serde_json::to_value(task).unwrap()).map_err(|_| err("internal_error"))
}
fn changed(before: &Json, after: &Json) -> Vec<String> {
    let a = &before["task"];
    let b = &after["task"];
    let mut result = Vec::new();
    if let Some(map) = b.as_object() {
        for (k, v) in map {
            if a.get(k) != Some(v) {
                result.push(k.clone());
            }
        }
    }
    result
}
fn execute_action(
    conn: &Connection,
    auth: &Authorization,
    args: &MutateArgs,
) -> Result<(Option<i64>, Json, Json, String, bool), McpError> {
    match &args.operation {
        TaskAction::Create { task } => {
            if !auth.scope.allows(
                &serde_json::to_string(&task.departments).unwrap(),
                &task.task_type,
            ) {
                return Err(err("forbidden"));
            }
            // Only exact in-scope candidates can be disclosed. Never merge on similarity.
            if !args.intent.allow_possible_duplicate
                && load_all_tasks(conn).map_err(domain)?.iter().any(|t| {
                    allowed(t, auth)
                        && t.deleted_at.is_none()
                        && t.title.trim() == task.title.trim()
                })
            {
                return Err(err("possible_duplicate"));
            }
            let input = TaskInput {
                id: None,
                planned_date: None,
                confirm_schedule_change: false,
                department: task.departments.first().cloned().unwrap_or_default(),
                departments: task.departments.clone(),
                contact: task.contacts.first().cloned().unwrap_or_default(),
                contacts: task.contacts.clone(),
                task_type: task.task_type.clone(),
                title: task.title.clone(),
                details: task.details.clone(),
                internal_notes: task.internal_notes.clone(),
                status: "pending".into(),
                priority: task.priority.clone().unwrap_or_else(|| "normal".into()),
                workload: task.workload.clone().unwrap_or_else(|| "standard".into()),
                is_urgent: false,
                urgent_requester: String::new(),
                urgent_reason: String::new(),
                requested_deadline: task.requested_deadline.clone(),
                requested_deadline_label: None,
            };
            let t = save_task_on(conn, input).map_err(domain)?;
            Ok((
                Some(t.id),
                Json::Null,
                proof(conn, t.id)?,
                "create".into(),
                false,
            ))
        }
        _ => {
            let target_arg = match &args.operation {
                TaskAction::Patch { target, .. }
                | TaskAction::SetStatus { target, .. }
                | TaskAction::SetUrgent { target, .. }
                | TaskAction::RecordWorkEvent { target, .. } => target,
                _ => unreachable!(),
            };
            let task = target(conn, target_arg, auth)?;
            let before = proof(conn, task.id)?;
            let action;
            match &args.operation {
                TaskAction::Patch {
                    field_base,
                    patch,
                    text_edits,
                    ..
                } => {
                    action = "patch";
                    let mut patch = patch.clone();
                    if text_edits.len() > 10 {
                        return Err(err("invalid_arguments"));
                    }
                    for e in text_edits {
                        if !matches!(e.field.as_str(), "details" | "internalNotes")
                            || patch.contains_key(&e.field)
                            || e.find.is_empty()
                        {
                            return Err(err("invalid_arguments"));
                        }
                        let original = before["task"][&e.field]
                            .as_str()
                            .ok_or_else(|| err("invalid_arguments"))?;
                        if original.matches(&e.find).count() != 1 {
                            return Err(err("conflict"));
                        }
                        patch.insert(
                            e.field.clone(),
                            json!(original.replacen(&e.find, &e.replace, 1)),
                        );
                    }
                    if patch.is_empty()
                        || patch.len() > FIELDS.len()
                        || field_base.len() != patch.len()
                    {
                        return Err(err("invalid_arguments"));
                    }
                    let fv = field_versions(conn, task.id, true)?;
                    let mut value = serde_json::to_value(&task).unwrap();
                    for (field, new) in &patch {
                        if !FIELDS.iter().any(|(f, _)| *f == field) {
                            return Err(err("invalid_arguments"));
                        }
                        if matches!(field.as_str(), "details" | "internalNotes")
                            && !text_edits.iter().any(|e| &e.field == field)
                            && !args.intent.replace_whole_text
                        {
                            return Err(err("explicit_intent_required"));
                        }
                        let base = field_base.get(field).ok_or_else(|| err("conflict"))?;
                        if before["task"][field] != base.value
                            || fv[field].as_i64() != Some(base.version)
                        {
                            return Err(err("conflict"));
                        }
                        value[field] = new.clone();
                    }
                    let input: TaskInput =
                        serde_json::from_value(value).map_err(|_| err("invalid_arguments"))?;
                    if !auth.scope.allows(
                        &serde_json::to_string(&normalized_departments(&input)).unwrap(),
                        &input.task_type,
                    ) {
                        return Err(err("forbidden"));
                    }
                    let unchanged = patch.iter().all(|(f, v)| before["task"][f] == *v);
                    if unchanged {
                        return Ok((Some(task.id), before.clone(), before, action.into(), true));
                    }
                    save_task_on(conn, input).map_err(domain)?;
                }
                TaskAction::SetStatus {
                    task_version,
                    status,
                    ..
                } => {
                    action = "setStatus";
                    if task.status == *status {
                        return Ok((Some(task.id), before.clone(), before, action.into(), true));
                    }
                    if version(conn, task.id)? != *task_version {
                        return Err(err("conflict"));
                    }
                    if task.archived_at.is_some() || status == "archived" {
                        return Err(err("business_rule"));
                    }
                    if is_work_event_status(status) && !args.intent.confirmed_real_work {
                        return Err(err("explicit_intent_required"));
                    }
                    set_status_on(conn, task.id, status.clone()).map_err(domain)?;
                }
                TaskAction::SetUrgent {
                    task_version,
                    is_urgent,
                    requester,
                    reason,
                    ..
                } => {
                    action = "setUrgent";
                    if task.is_urgent == *is_urgent
                        && (!is_urgent
                            || (task.urgent_requester == requester.trim()
                                && task.urgent_reason == reason.trim()))
                    {
                        return Ok((Some(task.id), before.clone(), before, action.into(), true));
                    }
                    if version(conn, task.id)? != *task_version {
                        return Err(err("conflict"));
                    }
                    set_urgent_on(conn, task.id, *is_urgent, requester.clone(), reason.clone())
                        .map_err(domain)?;
                }
                TaskAction::RecordWorkEvent {
                    task_version,
                    result_status,
                    handled_at,
                    note,
                    sync_status,
                    ..
                } => {
                    action = "recordWorkEvent";
                    if version(conn, task.id)? != *task_version {
                        return Err(err("conflict"));
                    }
                    if !args.intent.confirmed_real_work {
                        return Err(err("explicit_intent_required"));
                    }
                    record_work_event_on_input(
                        conn,
                        WorkEventInput {
                            task_id: task.id,
                            result_status: result_status.clone(),
                            handled_at: handled_at.clone(),
                            note: note.clone(),
                            sync_status: *sync_status,
                        },
                    )
                    .map_err(domain)?;
                }
                _ => unreachable!(),
            }
            let after = proof(conn, task.id)?;
            if !allowed(&get_task_on(conn, task.id).map_err(domain)?, auth) {
                return Err(err("forbidden"));
            }
            Ok((Some(task.id), before, after, action.into(), false))
        }
    }
}
impl Database {
    pub fn mcp_task_basis(&self, id: i64, full: bool) -> Result<Json, McpError> {
        self.with_conn(|c|Ok(json!({"taskVersion":version(c,id).map_err(|e|e.to_string())?,"fieldVersions":field_versions(c,id,full).map_err(|e|e.to_string())?}))).map_err(|_|err("internal_error"))
    }
    pub fn mcp_mutate(
        &self,
        client: &str,
        auth: &Authorization,
        args: MutateArgs,
    ) -> Result<Json, McpError> {
        if !auth.permissions.regular_read || !auth.permissions.write {
            return Err(err("forbidden"));
        }
        validate_intent(&args.idempotency_key, &args.intent, &args.reason)?;
        let request = json!({"tool":"mutate_task","args":args});
        self.mcp_transaction(|conn|{
            if let Some(r)=replay(conn,client,&args.idempotency_key,&request,auth)?{return Ok(r);}
            let prior=load_all_tasks(conn).map_err(domain)?.into_iter().map(|t|(t.id,t)).collect::<HashMap<_,_>>();
            let (id,mut before,mut after,action,skipped)=execute_action(conn,auth,&args)?;
            let mut related_before=vec![];let mut related_after=vec![];
            // Domain rules can reorder neighbours. Validate ALL resulting changes, not only input.
            for t in load_all_tasks(conn).map_err(domain)? {
                if prior.get(&t.id).is_none_or(|old|serde_json::to_value(old).unwrap()!=serde_json::to_value(&t).unwrap()) {
                    if !allowed(&t,auth) || prior.get(&t.id).is_some_and(|old|!allowed(old,auth)) {return Err(err("forbidden"));}
                    if Some(t.id)!=id {related_before.push(json!(prior.get(&t.id)));related_after.push(proof(conn,t.id)?);}
                }
            }
            if !related_after.is_empty(){before["relatedTasks"]=json!(related_before);after["relatedTasks"]=json!(related_after);}
            let audit_id=if skipped{None}else{Some(audit(conn,client,&action,id,&args.intent,&args.reason,&before,&after,&request,None)?)};
            // Receipt never contains existing field values, even with full-read permissions.
            let result=json!({"commitStatus":if skipped{"no_change"}else{"committed"},"verificationStatus":"verified_in_transaction","taskId":id,"auditId":audit_id,"changedFields":changed(&before,&after),"taskVersion":after["taskVersion"],"replayed":false});
            receipt(conn,client,&args.idempotency_key,&request,id,audit_id,&result)?;Ok(result)
        })
    }
    fn mcp_transaction(
        &self,
        operation: impl FnOnce(&Connection) -> Result<Json, McpError>,
    ) -> Result<Json, McpError> {
        let mut guard = self.connection.lock().map_err(|_| err("internal_error"))?;
        let conn = guard.as_mut().ok_or_else(|| err("host_unavailable"))?;
        let tx = conn.transaction().map_err(sql)?;
        let mut result = operation(&tx)?;
        tx.commit().map_err(|_| err("result_unknown"))?;
        // Keep the same database mutex across commit and independent readback.
        if let Some(id) = result["taskId"].as_i64() {
            if let Some(expected) = result["taskVersion"].as_i64() {
                result["verificationStatus"] = json!(if version(conn, id).ok() == Some(expected) {
                    "verified_after_commit"
                } else {
                    "verification_failed"
                });
            }
        } else if let Some(preferences) = result["preferences"].as_object() {
            let expected = json!({"fieldVersions":result["fieldVersions"]});
            let state = preference_state(conn, preferences.keys().cloned());
            let valid = state.is_ok_and(|s| {
                preferences.iter().all(|(k, v)| s[k] == *v)
                    && s["fieldVersions"] == expected["fieldVersions"]
            });
            result["verificationStatus"] = json!(if valid {
                "verified_after_commit"
            } else {
                "verification_failed"
            });
        } else if result.get("commitStatus").is_some() {
            result["verificationStatus"] = json!("verified_after_commit");
        }
        #[cfg(test)]
        if self
            .fail_mcp_verification
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            result["verificationStatus"] = json!("verification_failed");
        }
        Ok(result)
    }
    pub fn mcp_preferences(
        &self,
        client: &str,
        auth: &Authorization,
        args: PreferenceArgs,
    ) -> Result<Json, McpError> {
        if args.action == "get" {
            if args.field_versions.is_some()
                || args.patch.is_some()
                || args.field_base.is_some()
                || args.intent.is_some()
                || args.reason.is_some()
                || args.idempotency_key.is_some()
            {
                return Err(err("invalid_arguments"));
            }
            return self
                .with_conn(|conn| {
                    let mut state = preference_state(conn, PREFS.iter().map(|k| (*k).into()))
                        .map_err(|e| e.to_string())?;
                    let versions = state
                        .as_object_mut()
                        .unwrap()
                        .remove("fieldVersions")
                        .unwrap();
                    Ok(json!({"preferences":state,"fieldVersions":versions}))
                })
                .map_err(|_| err("internal_error"));
        }
        if args.action != "patch" {
            return Err(err("invalid_arguments"));
        }
        if !auth.permissions.write {
            return Err(err("forbidden"));
        }
        let key = args
            .idempotency_key
            .as_deref()
            .ok_or_else(|| err("invalid_arguments"))?;
        let intent = args
            .intent
            .as_ref()
            .ok_or_else(|| err("invalid_arguments"))?;
        let reason = args
            .reason
            .as_deref()
            .ok_or_else(|| err("invalid_arguments"))?;
        validate_intent(key, intent, reason)?;
        let patch = args
            .patch
            .as_ref()
            .ok_or_else(|| err("invalid_arguments"))?;
        let base = args
            .field_base
            .as_ref()
            .ok_or_else(|| err("invalid_arguments"))?;
        if patch.is_empty() || patch.len() != base.len() {
            return Err(err("invalid_arguments"));
        }
        let base_versions = args
            .field_versions
            .as_ref()
            .ok_or_else(|| err("invalid_arguments"))?;
        if base_versions.len() != patch.len() {
            return Err(err("invalid_arguments"));
        }
        let request = json!({"tool":"manage_preferences","args":args});
        self.mcp_transaction(|conn|{
            if let Some(r)=replay(conn,client,key,&request,auth)?{return Ok(r);}
            let mut before=serde_json::Map::new();let mut after=serde_json::Map::new();let mut prior_versions=serde_json::Map::new();let mut next_versions=serde_json::Map::new();
            for(k,v)in patch {
                if !PREFS.contains(&k.as_str()) || !valid_setting(k,v){return Err(err("invalid_arguments"));}
                let current:Option<String>=conn.query_row("SELECT value FROM settings WHERE key=?",[k],|r|r.get(0)).optional().map_err(sql)?;
                if base.get(k)!=Some(&current){return Err(err("conflict"));}
                let version=preference_version(conn,k)?;
                if base_versions.get(k)!=Some(&version){return Err(err("conflict"));}
                prior_versions.insert(k.clone(),json!(version));
                before.insert(k.clone(),json!(current));after.insert(k.clone(),json!(v));
            }
            let skipped=before==after;
            if !skipped{for(k,v)in patch {conn.execute("INSERT INTO settings VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![k,v]).map_err(sql)?;}}
            for k in patch.keys(){next_versions.insert(k.clone(),json!(preference_version(conn,k)?));}
            before.insert("fieldVersions".into(),json!(prior_versions));after.insert("fieldVersions".into(),json!(next_versions));
            let a=if skipped{None}else{Some(audit(conn,client,"preferences",None,intent,reason,&Json::Object(before),&json!(after),&request,None)?)};
            let versions=after.remove("fieldVersions").unwrap();
            let result=json!({"commitStatus":if skipped{"no_change"}else{"committed"},"verificationStatus":"verified_in_transaction","preferences":after,"fieldVersions":versions,"auditId":a,"changedFields":if skipped{vec![]}else{patch.keys().collect::<Vec<_>>()},"replayed":false});
            receipt(conn,client,key,&request,None,a,&result)?;Ok(result)
        })
    }
}

fn audit_row(
    conn: &Connection,
    id: i64,
) -> Result<(String, String, Option<i64>, Json, Json), McpError> {
    let row: Option<(String, String, Option<i64>, String, String)> = conn
        .query_row(
            "SELECT client_id,action,task_id,before_json,after_json FROM mcp_ai_audit WHERE id=?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()
        .map_err(sql)?;
    let (client, action, task, before, after) = row.ok_or_else(|| err("not_found"))?;
    Ok((
        client,
        action,
        task,
        serde_json::from_str(&before).map_err(|_| err("internal_error"))?,
        serde_json::from_str(&after).map_err(|_| err("internal_error"))?,
    ))
}
impl Database {
    pub fn mcp_request_undo(
        &self,
        client: &str,
        auth: &Authorization,
        args: UndoArgs,
    ) -> Result<Json, McpError> {
        if !auth.permissions.regular_read || !auth.permissions.write {
            return Err(err("forbidden"));
        }
        validate_intent(&args.idempotency_key, &args.intent, &args.reason)?;
        let request = json!({"tool":"request_undo","args":args});
        self.mcp_transaction(|conn|{
            if let Some(r)=replay(conn,client,&args.idempotency_key,&request,auth)?{return Ok(r);}
            let (owner,_,id,_,after)=audit_row(conn,args.audit_id)?;
            if owner!=client {return Err(err("not_found"));}
            related_scope(conn,&after,auth)?;
            if let Some(id)=id {if !allowed(&get_task_on(conn,id).map_err(domain)?,auth){return Err(err("forbidden"));}}
            let already:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM mcp_ai_audit WHERE undo_of=?)",[args.audit_id],|r|r.get(0)).map_err(sql)?;
            if already {return Err(err("conflict"));}
            conn.execute("INSERT OR IGNORE INTO mcp_undo_requests(audit_id,client_id,reason,created_at) VALUES(?,?,?,?)",params![args.audit_id,client,args.reason,now()]).map_err(sql)?;
            let (uid,status):(i64,String)=conn.query_row("SELECT id,status FROM mcp_undo_requests WHERE audit_id=?",[args.audit_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(sql)?;
            let result=json!({"status":if status=="pending"{"needs_user_approval"}else{&status},"undoRequestId":uid,"auditId":args.audit_id,"taskId":id,"replayed":false});
            receipt(conn,client,&args.idempotency_key,&request,id,Some(args.audit_id),&result)?;Ok(result)
        })
    }
    /// LOCAL GUI ONLY: this is intentionally not an MCP tool or action.
    pub fn mcp_audit_state(&self) -> Result<Json, String> {
        self.with_conn(|conn|{
            let mut query=conn.prepare("SELECT id,client_id,action,task_id,reason,intent,created_at,before_json,after_json,undo_of FROM mcp_ai_audit WHERE id IN (SELECT id FROM mcp_ai_audit ORDER BY id DESC LIMIT 50) OR id IN (SELECT audit_id FROM mcp_undo_requests WHERE status='pending') ORDER BY id DESC").map_err(display_error)?;
        let audits=query.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"clientId":r.get::<_,String>(1)?,"action":r.get::<_,String>(2)?,"taskId":r.get::<_,Option<i64>>(3)?,"reason":r.get::<_,String>(4)?,"intent":r.get::<_,String>(5)?,"createdAt":r.get::<_,String>(6)?,"before":serde_json::from_str::<Json>(&r.get::<_,String>(7)?).unwrap_or(Json::Null),"after":serde_json::from_str::<Json>(&r.get::<_,String>(8)?).unwrap_or(Json::Null),"undoOf":r.get::<_,Option<i64>>(9)?}))).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;
        let mut query=conn.prepare("SELECT id,audit_id,reason,created_at FROM mcp_undo_requests WHERE status='pending' ORDER BY id").map_err(display_error)?;
        let pending=query.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"auditId":r.get::<_,i64>(1)?,"reason":r.get::<_,String>(2)?,"createdAt":r.get::<_,String>(3)?}))).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;
        Ok(json!({"audits":audits,"pending":pending}))
    })
    }
    pub fn mcp_resolve_undo(&self, request_id: i64, approve: bool) -> Result<Json, McpError> {
        self.mcp_transaction(|conn|{
            let (aid,status):(i64,String)=conn.query_row("SELECT audit_id,status FROM mcp_undo_requests WHERE id=?",[request_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|err("not_found"))?;
            if status!="pending"{return Err(err("conflict"));}
            if !approve {conn.execute("UPDATE mcp_undo_requests SET status='rejected',resolved_at=? WHERE id=?",params![now(),request_id]).map_err(sql)?;return Ok(json!({"status":"rejected"}));}
            let(owner,action,id,before,after)=audit_row(conn,aid)?;
            let mut current=if let Some(id)=id{proof(conn,id)?}else{preference_state(conn,after.as_object().unwrap().keys().filter(|k|k.as_str()!="fieldVersions").cloned())?};
            if let Some(related)=after["relatedTasks"].as_array(){let mut current_related=vec![];for t in related{current_related.push(proof(conn,t["task"]["id"].as_i64().ok_or_else(||err("internal_error"))?)?);}current["relatedTasks"]=json!(current_related);}
            if action=="preferences" {
                for(k,v)in after.as_object().ok_or_else(||err("internal_error"))? {if k=="fieldVersions"{continue;}
                    let value:Option<String>=conn.query_row("SELECT value FROM settings WHERE key=?",[k],|r|r.get(0)).optional().map_err(sql)?;
                    if json!(value)!=*v || after["fieldVersions"][k].as_i64()!=Some(preference_version(conn,k)?){return Err(err("conflict"));}
                }
                for(k,v)in before.as_object().unwrap(){if k=="fieldVersions"{continue;}if v.is_null(){conn.execute("DELETE FROM settings WHERE key=?",[k]).map_err(sql)?;}else{conn.execute("INSERT INTO settings VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![k,v.as_str().ok_or_else(||err("internal_error"))?]).map_err(sql)?;}}
            } else {
                let id=id.ok_or_else(||err("internal_error"))?;
                if action=="patch" {
                    let mut value=current["task"].clone();
                    let mut edited=false;
                    for(f,_)in FIELDS {if before["task"][f]!=after["task"][f]{if current["task"][f]!=after["task"][f] || current["fieldVersions"][f]!=after["fieldVersions"][f]{return Err(err("conflict"));}value[f]=before["task"][f].clone();edited=true;}}
                    if !edited{return Err(err("conflict"));}
                    save_task_on(conn,serde_json::from_value(value).map_err(|_|err("internal_error"))?).map_err(domain)?;
                } else {
                    if current!=after {return Err(err("conflict"));}
                    match action.as_str(){
                        "create"=>{if !child_ids_on(conn,id).map_err(domain)?.is_empty(){return Err(err("conflict"));} soft_delete_task_on(conn,id,"用户批准撤销AI新建事项").map_err(domain)?;},
                        "setStatus"|"recordWorkEvent"=>{
                            // Compensating operation: never delete or overwrite historical work rows.
                            for event in after["workEventIds"].as_array().unwrap(){if !before["workEventIds"].as_array().unwrap().contains(event){void_work_event_on(conn,event.as_i64().unwrap(),true).map_err(domain)?;}}
                            let old:LegalTask=serde_json::from_value(before["task"].clone()).map_err(|_|err("internal_error"))?;
                            if get_task_on(conn,id).map_err(domain)?.status!=old.status {
                                let latest:i64=conn.query_row("SELECT COALESCE(MAX(id),0) FROM task_work_events WHERE task_id=?",[id],|r|r.get(0)).map_err(sql)?;
                                set_status_on(conn,id,old.status.clone()).map_err(domain)?;
                                let mut statement=conn.prepare("SELECT id FROM task_work_events WHERE task_id=? AND id>? AND voided_at IS NULL").map_err(sql)?;
                                let new_ids=statement.query_map(params![id,latest],|r|r.get::<_,i64>(0)).map_err(sql)?.collect::<Result<Vec<_>,_>>().map_err(sql)?;
                                for event in new_ids {void_work_event_on(conn,event,true).map_err(domain)?;}
                            }
                            let mut restored=task_input(&get_task_on(conn,id).map_err(domain)?)?;
                            restored.requested_deadline=old.requested_deadline.clone();restored.requested_deadline_label=old.requested_deadline_label.clone();
                            if old.is_scheduled {restored.planned_date=Some(old.planned_date.clone());restored.confirm_schedule_change=true;}
                            save_task_on(conn,restored).map_err(domain)?;
                            if old.is_urgent {set_urgent_on(conn,id,true,old.urgent_requester,old.urgent_reason).map_err(domain)?;}
                        },
                        "setUrgent"=>{let old:LegalTask=serde_json::from_value(before["task"].clone()).map_err(|_|err("internal_error"))?;set_urgent_on(conn,id,old.is_urgent,old.urgent_requester,old.urgent_reason).map_err(domain)?;
                            conn.execute("UPDATE tasks SET custom_sort_order=? WHERE id=?",params![old.custom_sort_order,id]).map_err(sql)?;
                            if let Some(related)=before["relatedTasks"].as_array(){for t in related{conn.execute("UPDATE tasks SET custom_sort_order=? WHERE id=?",params![t["customSortOrder"].as_i64().ok_or_else(||err("internal_error"))?,t["id"].as_i64().ok_or_else(||err("internal_error"))?]).map_err(sql)?;}}
                        },
                        _=>return Err(err("unsupported"))
                    }
                }
                add_log(conn,id,"audit","用户在软件内批准撤销AI操作；历史记录保留").map_err(domain)?;
            }
            let mut final_state=if let Some(id)=id{proof(conn,id)?}else{preference_state(conn,before.as_object().unwrap().keys().filter(|k|k.as_str()!="fieldVersions").cloned())?};
            if let Some(related)=after["relatedTasks"].as_array(){let mut final_related=vec![];for t in related{final_related.push(proof(conn,t["task"]["id"].as_i64().unwrap())?);}final_state["relatedTasks"]=json!(final_related);}
            let intent=Intent{summary:"用户在软件内批准撤销".into(),explicit_user_request:true,replace_whole_text:true,confirmed_real_work:true,allow_possible_duplicate:false};
            let undo_audit=audit(conn,&owner,"approvedUndo",id,&intent,"用户批准的补偿操作；不回收编号、不删除原历史",&current,&final_state,&json!({"requestId":request_id}),Some(aid))?;
            conn.execute("UPDATE mcp_undo_requests SET status='approved',resolved_at=? WHERE id=?",params![now(),request_id]).map_err(sql)?;
            let mut result=json!({"status":"approved","commitStatus":"committed","auditId":undo_audit,"taskId":id,"taskVersion":final_state["taskVersion"]});
            if id.is_none(){let versions=final_state.as_object_mut().unwrap().remove("fieldVersions").unwrap();result["preferences"]=final_state;result["fieldVersions"]=versions;}
            Ok(result)
        })
    }
}
