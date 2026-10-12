//! P4 actions delegate to the existing GUI domain transaction bodies.
use super::mcp_write::*;
use super::*;
use crate::mcp::{contract::McpError, security::Authorization, write_types::*};
use serde_json::Value as Json;

pub(super) fn resolve(
    conn: &Connection,
    t: &Target,
    auth: &Authorization,
    trash: bool,
) -> Result<LegalTask, McpError> {
    if t.batch_ref.is_some() {
        return Err(err("invalid_arguments"));
    }
    if !trash {
        return target(conn, t, auth);
    }
    if usize::from(t.task_id.is_some())
        + usize::from(t.permanent_number.is_some())
        + usize::from(t.title.is_some())
        != 1
    {
        return Err(err("invalid_arguments"));
    }
    let mut found = load_all_tasks(conn)
        .map_err(domain)?
        .into_iter()
        .filter(|v| {
            allowed(v, auth)
                && v.deleted_at.is_some()
                && (t.task_id == Some(v.id)
                    || t.permanent_number.as_ref() == Some(&v.permanent_number)
                    || t.title.as_ref() == Some(&v.title))
        });
    let first = found.next().ok_or_else(|| err("not_found"))?;
    if found.next().is_some() {
        return Err(err("needs_disambiguation"));
    }
    Ok(first)
}
fn check(conn: &Connection, id: i64, expected: i64) -> Result<(), McpError> {
    if version(conn, id)? != expected {
        Err(err("conflict"))
    } else {
        Ok(())
    }
}
pub(super) fn execute(
    conn: &Connection,
    auth: &Authorization,
    args: &MutateArgs,
) -> Result<(Option<i64>, Json, Json, String, bool), McpError> {
    if let TaskAction::CreateSubtask {
        parent,
        parent_version,
        task,
        enqueue_today,
    } = &args.operation
    {
        let p = resolve(conn, parent, auth, false)?;
        check(conn, p.id, *parent_version)?;
        if p.is_scheduled && task.planned_date.is_none() {
            return Err(err("explicit_intent_required"));
        }
        if !auth.scope.allows(
            &serde_json::to_string(&task.departments).unwrap(),
            &task.task_type,
        ) {
            return Err(err("forbidden"));
        }
        if !args.intent.allow_possible_duplicate
            && load_all_tasks(conn).map_err(domain)?.iter().any(|t| {
                allowed(t, auth) && t.deleted_at.is_none() && t.title.trim() == task.title.trim()
            })
        {
            return Err(err("possible_duplicate"));
        }
        let child = create_subtask_on_domain(
            conn,
            CreateSubtaskInput {
                parent_task_id: p.id,
                planned_date: task.planned_date.clone(),
                title: task.title.clone(),
                details: task.details.clone(),
                task_type: Some(task.task_type.clone()),
                departments: Some(task.departments.clone()),
                contacts: Some(task.contacts.clone()),
                priority: task.priority.clone(),
                workload: task.workload.clone(),
                is_urgent: false,
                urgent_requester: String::new(),
                urgent_reason: String::new(),
                requested_deadline: task.requested_deadline.clone(),
                requested_deadline_label: None,
                internal_notes: task.internal_notes.clone(),
                enqueue_today: *enqueue_today,
            },
        )
        .map_err(domain)?;
        return Ok((
            Some(child.id),
            Json::Null,
            proof(conn, child.id)?,
            "createSubtask".into(),
            false,
        ));
    }
    if let TaskAction::ReorderSubtasks {
        parent,
        parent_version,
        task_ids,
        task_versions,
    } = &args.operation
    {
        let p = resolve(conn, parent, auth, false)?;
        check(conn, p.id, *parent_version)?;
        if task_ids.len() > 100 || task_ids.len() != task_versions.len() {
            return Err(err("invalid_arguments"));
        }
        for id in task_ids {
            let t = get_task_on(conn, *id).map_err(domain)?;
            if !allowed(&t, auth) {
                return Err(err("forbidden"));
            }
            check(
                conn,
                *id,
                *task_versions
                    .get(&id.to_string())
                    .ok_or_else(|| err("conflict"))?,
            )?;
        }
        let before = proof(conn, p.id)?;
        reorder_subtasks_on_domain(
            conn,
            ReorderSubtasksInput {
                parent_task_id: p.id,
                task_ids: task_ids.clone(),
            },
        )
        .map_err(domain)?;
        return Ok((
            Some(p.id),
            before,
            proof(conn, p.id)?,
            "reorderSubtasks".into(),
            false,
        ));
    }
    let (t, v, restore) = match &args.operation {
        TaskAction::SetParent {
            target,
            task_version,
            ..
        }
        | TaskAction::Queue {
            target,
            task_version,
            ..
        }
        | TaskAction::Reschedule {
            target,
            task_version,
            ..
        }
        | TaskAction::Archive {
            target,
            task_version,
            ..
        }
        | TaskAction::Trash {
            target,
            task_version,
            ..
        }
        | TaskAction::CorrectWorkEvent {
            target,
            task_version,
            ..
        }
        | TaskAction::VoidWorkEvent {
            target,
            task_version,
            ..
        } => (target, *task_version, false),
        TaskAction::Restore {
            target,
            task_version,
        } => (target, *task_version, true),
        _ => return Err(err("unsupported")),
    };
    let task = resolve(conn, t, auth, restore)?;
    check(conn, task.id, v)?;
    let before = proof(conn, task.id)?;
    let id = task.id;
    let action = match &args.operation {
        TaskAction::SetParent {
            parent,
            parent_version,
            ..
        } => {
            // Old and new parents are read for relationship logs and must also be in range.
            if let Some(old) = task.parent_task_id {
                if !allowed(&get_task_on(conn, old).map_err(domain)?, auth) {
                    return Err(err("forbidden"));
                }
            }
            let p = parent
                .as_ref()
                .map(|t| resolve(conn, t, auth, false))
                .transpose()?;
            if let Some(p) = &p {
                check(conn, p.id, parent_version.ok_or_else(|| err("conflict"))?)?;
            }
            if task.parent_task_id == p.as_ref().map(|p| p.id) {
                return Ok((Some(id), before.clone(), before, "setParent".into(), true));
            }
            set_parent_task_on_domain(conn, id, p.map(|p| p.id)).map_err(domain)?;
            "setParent"
        }
        TaskAction::Queue {
            mode,
            inherit_deadline,
            ..
        } => {
            let q = QueueInput {
                id,
                inherit_deadline: *inherit_deadline,
                reason: args.reason.clone(),
            };
            match mode.as_str() {
                "enqueue" => {
                    if task.has_active_queue {
                        return Ok((Some(id), before.clone(), before, "queue".into(), true));
                    }
                    enqueue_task_on_domain(conn, q).map_err(domain)?;
                }
                "reopen" => reopen_task_on_domain(conn, q).map_err(domain)?,
                "moveUp" => move_task_on_domain(conn, id, MoveDirection::Up).map_err(domain)?,
                "moveDown" => move_task_on_domain(conn, id, MoveDirection::Down).map_err(domain)?,
                "processRound" | "completeRound" => {
                    if !args.intent.confirmed_real_work {
                        return Err(err("explicit_intent_required"));
                    }
                    if mode == "processRound" {
                        process_round_on_domain(conn, id).map_err(domain)?;
                    } else {
                        complete_round_on_domain(conn, id).map_err(domain)?;
                    }
                }
                _ => return Err(err("invalid_arguments")),
            };
            "queue"
        }
        TaskAction::Reschedule {
            planned_date,
            requested_deadline,
            ..
        } => {
            if task.planned_date == *planned_date && task.requested_deadline == *requested_deadline
            {
                return Ok((Some(id), before.clone(), before, "reschedule".into(), true));
            }
            let mut input = task_input(&task)?;
            input.planned_date = Some(planned_date.clone());
            input.confirm_schedule_change = true;
            input.requested_deadline = requested_deadline.clone();
            input.requested_deadline_label = None;
            save_task_on(conn, input).map_err(domain)?;
            "reschedule"
        }
        TaskAction::Archive {
            include_completed_subtasks,
            ..
        } => {
            if task.status == "archived" && !*include_completed_subtasks {
                return Ok((Some(id), before.clone(), before, "archive".into(), true));
            }
            archive_task_group_on_domain(
                conn,
                ArchiveTaskInput {
                    task_id: id,
                    include_completed_subtasks: *include_completed_subtasks,
                },
            )
            .map_err(domain)?;
            "archive"
        }
        TaskAction::Trash {
            include_subtasks, ..
        } => {
            delete_task_group_on_domain(
                conn,
                DeleteTaskInput {
                    task_id: id,
                    include_subtasks: *include_subtasks,
                },
            )
            .map_err(domain)?;
            "trash"
        }
        TaskAction::Restore { .. } => {
            restore_on_domain(conn, id).map_err(domain)?;
            "restore"
        }
        TaskAction::CorrectWorkEvent {
            event_id,
            result_status,
            handled_at,
            note,
            confirm_historical_impact,
            ..
        } => {
            if !args.intent.confirmed_real_work || !*confirm_historical_impact {
                return Err(err("explicit_intent_required"));
            }
            event_owner(conn, *event_id, id)?;
            void_work_event_on(conn, *event_id, true).map_err(domain)?;
            record_work_event_on_input(
                conn,
                WorkEventInput {
                    task_id: id,
                    result_status: result_status.clone(),
                    handled_at: handled_at.clone(),
                    note: note.clone(),
                    sync_status: false,
                },
            )
            .map_err(domain)?;
            let new_id: i64 = conn
                .query_row(
                    "SELECT MAX(id) FROM task_work_events WHERE task_id=?",
                    [id],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            conn.execute(
                "UPDATE task_work_events SET correction_of=? WHERE id=?",
                params![event_id, new_id],
            )
            .map_err(sql)?;
            add_log(conn,id,"audit",&format!("明确确认统计影响后更正办理活动；原事件 {event_id} 作废，新事件 {new_id} 关联原记录")).map_err(domain)?;
            "correctWorkEvent"
        }
        TaskAction::VoidWorkEvent {
            event_id,
            confirm_historical_impact,
            ..
        } => {
            if !*confirm_historical_impact {
                return Err(err("explicit_intent_required"));
            }
            event_owner(conn, *event_id, id)?;
            void_work_event_on(conn, *event_id, true).map_err(domain)?;
            "voidWorkEvent"
        }
        _ => return Err(err("unsupported")),
    };
    Ok((Some(id), before, proof(conn, id)?, action.into(), false))
}
fn event_owner(conn: &Connection, event: i64, id: i64) -> Result<(), McpError> {
    let exists:bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM task_work_events WHERE id=? AND task_id=? AND voided_at IS NULL)",params![event,id],|r|r.get(0)).map_err(sql)?;
    if exists {
        Ok(())
    } else {
        Err(err("not_found"))
    }
}

/// User-approved compensations preserve all original histories and never reuse numbers.
pub(super) fn undo(
    conn: &Connection,
    action: &str,
    id: i64,
    before: &Json,
    after: &Json,
) -> Result<(), McpError> {
    if matches!(action, "createSubtask" | "create") {
        delete_task_group_on_domain(
            conn,
            DeleteTaskInput {
                task_id: id,
                include_subtasks: true,
            },
        )
        .map_err(domain)?;
        return Ok(());
    }
    if !matches!(
        action,
        "setParent"
            | "reorderSubtasks"
            | "queue"
            | "reschedule"
            | "archive"
            | "trash"
            | "restore"
            | "correctWorkEvent"
            | "voidWorkEvent"
            | "patch"
            | "setStatus"
            | "setUrgent"
            | "recordWorkEvent"
    ) {
        return Err(err("unsupported"));
    }
    // Reverse newly-added events by voiding. Restore a voided original only by a new linked event.
    for event in after["workEvents"]
        .as_array()
        .ok_or_else(|| err("internal_error"))?
    {
        if event["voidedAt"].is_null()
            && !before["workEvents"]
                .as_array()
                .unwrap()
                .iter()
                .any(|old| old["id"] == event["id"])
        {
            void_work_event_on(conn, event["id"].as_i64().unwrap(), true).map_err(domain)?;
        }
    }
    for event in before["workEvents"].as_array().unwrap() {
        if event["voidedAt"].is_null()
            && after["workEvents"]
                .as_array()
                .unwrap()
                .iter()
                .any(|new| new["id"] == event["id"] && !new["voidedAt"].is_null())
        {
            record_work_event_on_input(
                conn,
                WorkEventInput {
                    task_id: id,
                    result_status: event["resultStatus"].as_str().unwrap().into(),
                    handled_at: event["handledAt"].as_str().unwrap().into(),
                    note: event["note"].as_str().unwrap().into(),
                    sync_status: false,
                },
            )
            .map_err(domain)?;
            let new_id: i64 = conn
                .query_row(
                    "SELECT MAX(id) FROM task_work_events WHERE task_id=?",
                    [id],
                    |r| r.get(0),
                )
                .map_err(sql)?;
            conn.execute(
                "UPDATE task_work_events SET correction_of=? WHERE id=?",
                params![event["id"].as_i64().unwrap(), new_id],
            )
            .map_err(sql)?;
        }
    }
    let old: LegalTask =
        serde_json::from_value(before["task"].clone()).map_err(|_| err("internal_error"))?;
    let mut rows = vec![old];
    if let Some(related) = before["relatedTasks"].as_array() {
        for t in related {
            rows.push(
                serde_json::from_value::<LegalTask>(t.clone())
                    .map_err(|_| err("internal_error"))?,
            );
        }
    }
    // Children first for restoring completion/archive constraints; relationship changes remain explicit.
    rows.sort_by_key(|t| if t.parent_task_id.is_some() { 0 } else { 1 });
    for old in rows {
        restore_state(conn, &old)?;
    }
    Ok(())
}
fn restore_state(conn: &Connection, old: &LegalTask) -> Result<(), McpError> {
    let mut current = get_task_on(conn, old.id).map_err(domain)?;
    if old.deleted_at.is_some() {
        if current.deleted_at.is_none() {
            soft_delete_task_on(conn, old.id, "用户批准撤销恢复操作").map_err(domain)?;
        }
        return Ok(());
    }
    if current.deleted_at.is_some() {
        restore_on_domain(conn, old.id).map_err(domain)?;
        current = get_task_on(conn, old.id).map_err(domain)?;
    }
    if current.archived_at.is_some() && old.archived_at.is_none() {
        reopen_task_on_domain(
            conn,
            QueueInput {
                id: old.id,
                inherit_deadline: false,
                reason: "用户批准撤销归档".into(),
            },
        )
        .map_err(domain)?;
        current = get_task_on(conn, old.id).map_err(domain)?;
    }
    let last_event: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(id),0) FROM task_work_events WHERE task_id=?",
            [old.id],
            |r| r.get(0),
        )
        .map_err(sql)?;
    if current.status != old.status && old.status != "archived" {
        set_status_on(conn, old.id, old.status.clone()).map_err(domain)?;
    }
    let mut input = task_input(&get_task_on(conn, old.id).map_err(domain)?)?;
    input.title = old.title.clone();
    input.details = old.details.clone();
    input.internal_notes = old.internal_notes.clone();
    input.department = old.department.clone();
    input.departments = old.departments.clone();
    input.contact = old.contact.clone();
    input.contacts = old.contacts.clone();
    input.task_type = old.task_type.clone();
    input.priority = old.priority.clone();
    input.workload = old.workload.clone();
    input.requested_deadline = old.requested_deadline.clone();
    input.requested_deadline_label = old.requested_deadline_label.clone();
    if old.is_scheduled {
        input.planned_date = Some(old.planned_date.clone());
        input.confirm_schedule_change = true;
    }
    save_task_on(conn, input).map_err(domain)?;
    // Status compensation must not fabricate real work statistics.
    let mut statement = conn
        .prepare("SELECT id FROM task_work_events WHERE task_id=? AND id>? AND voided_at IS NULL")
        .map_err(sql)?;
    let newly = statement
        .query_map(params![old.id, last_event], |r| r.get::<_, i64>(0))
        .map_err(sql)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql)?;
    for event in newly {
        void_work_event_on(conn, event, true).map_err(domain)?;
    }
    if old.parent_task_id != get_task_on(conn, old.id).map_err(domain)?.parent_task_id {
        set_parent_task_on_domain(conn, old.id, old.parent_task_id).map_err(domain)?;
    }
    let current = get_task_on(conn, old.id).map_err(domain)?;
    if old.archived_at.is_some() && current.archived_at.is_none() {
        archive_task_group_on_domain(
            conn,
            ArchiveTaskInput {
                task_id: old.id,
                include_completed_subtasks: false,
            },
        )
        .map_err(domain)?;
    }
    if !old.has_active_queue && !old.is_scheduled {
        close_active_queue(conn, old.id, "用户批准撤销队列操作").map_err(domain)?;
    } else if old.has_active_queue && !current.has_active_queue && !old.is_scheduled {
        enqueue_task_on_domain(
            conn,
            QueueInput {
                id: old.id,
                inherit_deadline: true,
                reason: "用户批准撤销后重新入队，旧号不回收".into(),
            },
        )
        .map_err(domain)?;
    }
    if get_task_on(conn, old.id).map_err(domain)?.is_urgent != old.is_urgent {
        set_urgent_on(
            conn,
            old.id,
            old.is_urgent,
            old.urgent_requester.clone(),
            old.urgent_reason.clone(),
        )
        .map_err(domain)?;
    }
    conn.execute(
        "UPDATE tasks SET custom_sort_order=?,subtask_sort_order=? WHERE id=?",
        params![old.custom_sort_order, old.subtask_sort_order, old.id],
    )
    .map_err(sql)?;
    Ok(())
}
