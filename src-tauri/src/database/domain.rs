//! Shared GUI/MCP domain actions. The caller owns the transaction.
use super::*;
pub(super) fn save_task_on(
    connection: &Connection,
    mut input: TaskInput,
) -> Result<LegalTask, String> {
    validate_task_input(&input)?;
    let contacts = normalized_contacts(&input);
    let departments = normalized_departments(&input);
    let stored_contacts = contact_storage(&contacts)?;
    let stored_departments = contact_storage(&departments)?;
    let transaction = connection;
    let stamp = now();
    let mut previous_task = input
        .id
        .map(|id| get_task_on(&transaction, id))
        .transpose()?;
    let plan = input.planned_date.clone().unwrap_or_else(|| {
        previous_task
            .as_ref()
            .map(|task| {
                if task.planned_date.is_empty() {
                    task.ticket_date.clone()
                } else {
                    task.planned_date.clone()
                }
            })
            .unwrap_or_else(today)
    });
    let mut plan_changed = input.planned_date.is_some()
        && previous_task
            .as_ref()
            .is_some_and(|task| task.planned_date != plan);
    let terminal = previous_task.as_ref().is_some_and(|task| {
        task.archived_at.is_some() || matches!(task.status.as_str(), "completed" | "archived")
    });
    plan_changed =
        plan_changed || (terminal && input.planned_date.is_some() && input.confirm_schedule_change);
    if terminal && plan_changed {
        input.requested_deadline = None;
        input.requested_deadline_label = None;
    }
    validate_plan(
        &plan,
        input.requested_deadline.as_deref(),
        previous_task.is_none() || plan_changed,
    )?;
    if plan_changed {
        let previous = previous_task.as_ref().unwrap();
        if previous.deleted_at.is_some() {
            return Err("回收站事项请先恢复再制定计划".into());
        }
        if !input.confirm_schedule_change {
            return Err("修改加入日期会作废原队列编号，请先确认".into());
        }
        replan_on(
            &transaction,
            previous.id,
            &plan,
            input.requested_deadline.as_deref(),
            input.requested_deadline_label.as_deref(),
        )?;
        input.status = "pending".into();
        previous_task = Some(get_task_on(&transaction, previous.id)?);
    }
    let scheduled_creation = previous_task.is_none() && plan > today();
    if scheduled_creation && input.status != "pending" {
        return Err("未来事项创建时请使用待处理状态".into());
    }
    if previous_task.as_ref().is_some_and(|task| task.is_scheduled)
        && matches!(
            input.status.as_str(),
            "processing" | "processed" | "completed"
        )
    {
        previous_task = Some(enter_current_workflow_on(&transaction, input.id.unwrap())?);
    }
    let effective_is_urgent = input.is_urgent && !clears_urgent_status(&input.status);
    let id = if let Some(previous) = previous_task.as_ref() {
        let id = previous.id;
        if (previous.archived_at.is_some() || previous.status == "archived")
            && input.status != previous.status
        {
            return Err("已归档事项请先使用“重新开启并加入今日队列”".into());
        }
        if previous.status != input.status && input.status == "completed" {
            ensure_direct_completion_allowed(&transaction, id)?;
        }
        let started = if input.status == "processing" && previous.started_at.is_none() {
            Some(stamp.clone())
        } else {
            previous.started_at.clone()
        };
        let completed = if input.status == "completed" {
            previous
                .completed_at
                .clone()
                .or_else(|| Some(stamp.clone()))
        } else {
            previous.completed_at.clone()
        };
        transaction.execute(
                "UPDATE tasks SET department=?,contact=?,task_type=?,title=?,details=?,status=?,priority=?,workload=?,
                 is_urgent=?,urgent_requester=?,urgent_reason=?,requested_deadline=?,requested_deadline_label=?,internal_notes=?,updated_at=?,
                 started_at=?,completed_at=? WHERE id=?",
                params![&stored_departments,&stored_contacts,input.task_type.trim(),input.title.trim(),
                 input.details.trim(),input.status,input.priority,input.workload,effective_is_urgent as i64,
                input.urgent_requester.trim(),input.urgent_reason.trim(),input.requested_deadline,input.requested_deadline_label,
                input.internal_notes.trim(),stamp,started,completed,id]).map_err(display_error)?;
        if previous.status != input.status {
            add_status(&transaction, id, Some(&previous.status), &input.status, "")?;
            add_log(
                &transaction,
                id,
                "status",
                &format!("状态变更为：{}", input.status),
            )?;
            if is_work_event_status(&input.status) {
                record_work_event_on(
                    &transaction,
                    id,
                    &input.status,
                    &stamp,
                    input.task_type.trim(),
                    "status_change",
                    "",
                )?;
            }
            if is_deferred_status(&input.status)
                || matches!(
                    input.status.as_str(),
                    "completed" | "cancelled" | "archived"
                )
            {
                stop_scheduled_on(
                    &transaction,
                    id,
                    "用户主动修改工作流状态",
                    matches!(
                        input.status.as_str(),
                        "completed" | "cancelled" | "archived"
                    ),
                )?;
                close_active_queue(&transaction, id, &format!("状态变更为 {}", input.status))?;
            } else if matches!(input.status.as_str(), "pending" | "processing")
                && !previous.has_active_queue
            {
                enqueue_on(
                    &transaction,
                    id,
                    &input.status,
                    false,
                    Some((
                        input.requested_deadline.clone(),
                        input.requested_deadline_label.clone(),
                    )),
                    "通过事项编辑重新加入队列",
                    false,
                )?;
            }
        }
        if previous.has_active_queue && matches!(input.status.as_str(), "pending" | "processing") {
            transaction
                    .execute(
                        "UPDATE task_queue_entries SET requested_deadline=?,requested_deadline_label=?,updated_at=?
                         WHERE task_id=? AND closed_at IS NULL",
                        params![
                            input.requested_deadline,
                            input.requested_deadline_label,
                            stamp,
                            id
                        ],
                    )
                    .map_err(display_error)?;
        }
        if previous.is_urgent != effective_is_urgent {
            if effective_is_urgent {
                record_urgent(&transaction, id, &input)?;
                promote_one(&transaction, id)?;
            } else {
                cancel_urgent_records(
                    &transaction,
                    id,
                    if clears_urgent_status(&input.status) {
                        "事项已完成或进入暂缓队列"
                    } else {
                        ""
                    },
                )?;
            }
        }
        add_log(&transaction, id, "updated", "更新事项信息")?;
        id
    } else {
        let identity_date = today();
        let identity_sequence = next_daily_sequence(&transaction, &identity_date)?;
        let date = plan.clone();
        let sequence = if scheduled_creation {
            next_daily_sequence(&transaction, &date)?
        } else {
            identity_sequence
        };
        let permanent = format!(
            "{}-{:02}",
            identity_date.replace('-', ""),
            identity_sequence
        );
        let order: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(custom_sort_order),0)+1 FROM tasks",
                [],
                |row| row.get(0),
            )
            .map_err(display_error)?;
        transaction.execute(
                "INSERT INTO tasks(permanent_number,daily_sequence,ticket_date,department,contact,task_type,title,details,
                 status,priority,workload,is_urgent,urgent_requester,urgent_reason,requested_deadline,requested_deadline_label,internal_notes,
                 created_at,updated_at,started_at,completed_at,custom_sort_order)
                 VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                params![permanent,sequence,date,&stored_departments,&stored_contacts,input.task_type.trim(),
                 input.title.trim(),input.details.trim(),input.status,input.priority,input.workload,effective_is_urgent as i64,
                input.urgent_requester.trim(),input.urgent_reason.trim(),input.requested_deadline,input.requested_deadline_label,input.internal_notes.trim(),
                stamp,stamp,if input.status=="processing"{Some(now())}else{None},if input.status=="completed"{Some(now())}else{None},order]
            ).map_err(display_error)?;
        let id = transaction.last_insert_rowid();
        transaction
            .execute(
                "UPDATE tasks SET planned_date=?,is_scheduled=? WHERE id=?",
                params![plan, scheduled_creation as i64, id],
            )
            .map_err(display_error)?;
        allocate_number_on(&transaction, id, &date, sequence)?;
        if !scheduled_creation {
            transaction
                .execute(
                    "INSERT INTO task_queue_entries(
                       task_id,queue_date,daily_sequence,requested_deadline,requested_deadline_label,
                       enqueued_at,created_at,updated_at
                     ) VALUES(?,?,?,?,?,?,?,?)",
                    params![
                        id,
                        date,
                        sequence,
                        input.requested_deadline,
                        input.requested_deadline_label,
                        stamp,
                        stamp,
                        stamp
                    ],
                )
                .map_err(display_error)?;
            transaction
                .execute(
                    "UPDATE queue_number_allocations SET activated_at=? WHERE task_id=?",
                    params![stamp, id],
                )
                .map_err(display_error)?;
        } else {
            add_log(
                &transaction,
                id,
                "scheduled_created",
                &format!(
                    "未来事项：计划加入日期 {}；预分配队列编号 {:02}（{}-{:02}）",
                    date, sequence, date, sequence
                ),
            )?;
        }
        add_log(
            &transaction,
            id,
            "created",
            &format!("创建事项并取号：{permanent}"),
        )?;
        add_status(&transaction, id, None, &input.status, "创建事项")?;
        if is_work_event_status(&input.status) {
            record_work_event_on(
                &transaction,
                id,
                &input.status,
                &stamp,
                input.task_type.trim(),
                "status_change",
                "",
            )?;
        }
        if is_deferred_status(&input.status)
            || matches!(
                input.status.as_str(),
                "completed" | "cancelled" | "archived"
            )
        {
            close_active_queue(&transaction, id, &format!("初始状态为 {}", input.status))?;
        }
        if effective_is_urgent {
            record_urgent(&transaction, id, &input)?;
            if !scheduled_creation {
                promote_one(&transaction, id)?;
            }
        }
        id
    };
    ensure_master(&transaction, "task_type", &input.task_type)?;
    let department_changed = previous_task
        .as_ref()
        .map(|previous| previous.departments != departments)
        .unwrap_or(true);
    let task_type_changed = previous_task
        .as_ref()
        .map(|previous| previous.task_type != input.task_type.trim())
        .unwrap_or(true);
    if department_changed {
        for department in &departments {
            ensure_master(&transaction, "department", department)?;
            bump_master_use(&transaction, "department", department)?;
        }
    }
    if task_type_changed {
        bump_master_use(&transaction, "task_type", &input.task_type)?;
    }
    for contact in contacts {
        ensure_master(&transaction, "contact", &contact)?;
        let is_new_contact = previous_task
            .as_ref()
            .map(|previous| !previous.contacts.contains(&contact))
            .unwrap_or(true);
        if is_new_contact {
            bump_master_use(&transaction, "contact", &contact)?;
        }
    }
    get_task_on(connection, id)
}
pub(super) fn set_status_on(
    connection: &Connection,
    id: i64,
    status: String,
) -> Result<(), String> {
    if !ALL_STATUSES.contains(&status.as_str()) {
        return Err("事项状态无效".into());
    }
    let transaction = connection;

    let task = if matches!(status.as_str(), "processing" | "processed" | "completed") {
        enter_current_workflow_on(transaction, id)?
    } else {
        get_task_on(transaction, id)?
    };
    if task.status == status {
        if clears_urgent_status(&status) {
            clear_urgent_on(transaction, id, "事项已完成或进入暂缓队列")?;
        }
        return Ok(());
    }
    if status == "completed" {
        ensure_direct_completion_allowed(transaction, id)?;
    }
    if matches!(status.as_str(), "pending" | "processing") && !task.has_active_queue {
        enqueue_on(
            transaction,
            id,
            &status,
            false,
            Some((
                task.requested_deadline.clone(),
                task.requested_deadline_label.clone(),
            )),
            "修改状态并加入今日队列",
            false,
        )?;
        return Ok(());
    }
    let stamp = now();
    let started = if status == "processing" && task.started_at.is_none() {
        Some(stamp.clone())
    } else {
        task.started_at
    };
    let completed = if status == "completed" {
        Some(stamp.clone())
    } else {
        task.completed_at
    };
    transaction
        .execute(
            "UPDATE tasks SET status=?,updated_at=?,started_at=?,completed_at=? WHERE id=?",
            params![status, stamp, started, completed, id],
        )
        .map_err(display_error)?;
    add_status(transaction, id, Some(&task.status), &status, "")?;
    add_log(transaction, id, "status", &format!("状态变更为：{status}"))?;
    if is_work_event_status(&status) {
        record_work_event_on(
            transaction,
            id,
            &status,
            &stamp,
            &task.task_type,
            "status_change",
            "",
        )?;
    }
    if is_deferred_status(&status)
        || matches!(status.as_str(), "completed" | "cancelled" | "archived")
    {
        stop_scheduled_on(
            transaction,
            id,
            "用户主动修改状态",
            matches!(status.as_str(), "completed" | "cancelled" | "archived"),
        )?;
        close_active_queue(transaction, id, &format!("状态变更为 {status}"))?;
    }
    if clears_urgent_status(&status) {
        clear_urgent_on(transaction, id, "事项已完成或进入暂缓队列")?;
    }
    Ok(())
}
pub(super) fn set_urgent_on(
    connection: &Connection,
    id: i64,
    is_urgent: bool,
    requester: String,
    reason: String,
) -> Result<(), String> {
    let transaction = connection;

    let task = get_task_on(transaction, id)?;
    if is_urgent && clears_urgent_status(&task.status) {
        return Err("已完成或暂缓事项不能设置加急".into());
    }
    let requester = requester.trim();
    let reason = reason.trim();
    if is_urgent && (requester.is_empty() || reason.is_empty()) {
        return Err("加急事项需要填写加急申请人和加急原因".into());
    }
    let stamp = now();
    transaction
                .execute(
                    "UPDATE tasks SET is_urgent=?,urgent_requester=?,urgent_reason=?,updated_at=? WHERE id=?",
                    params![is_urgent as i64, if is_urgent { requester } else { "" }, if is_urgent { reason } else { "" }, stamp, id],
                )
                .map_err(display_error)?;
    if is_urgent && !task.is_urgent {
        record_urgent_values(
            transaction,
            id,
            requester,
            reason,
            task.requested_deadline.as_deref(),
        )?;
        promote_one(transaction, id)?;
    } else if is_urgent && (task.urgent_requester != requester || task.urgent_reason != reason) {
        add_log(transaction, id, "urgent", "更新加急信息")?;
    } else if !is_urgent && task.is_urgent {
        cancel_urgent_records(transaction, id, "")?;
    }
    Ok(())
}
pub(super) fn record_work_event_on_input(
    connection: &Connection,
    input: WorkEventInput,
) -> Result<(), String> {
    if !is_work_event_status(&input.result_status) {
        return Err("处理结果无效".into());
    }
    validate_handled_at(&input.handled_at)?;
    let tx = connection;

    let mut task = get_task_on(tx, input.task_id)?;
    if task.deleted_at.is_some() {
        return Err("回收站事项不能新增处理活动".into());
    }
    if input.sync_status && (task.archived_at.is_some() || task.status == "archived") {
        return Err("已归档事项请先重新开启；也可以取消勾选同步状态，仅补录处理活动".into());
    }
    if input.sync_status && matches!(input.result_status.as_str(), "processed" | "completed") {
        task = enter_current_workflow_on(tx, input.task_id)?;
    }
    if input.sync_status && task.status != input.result_status {
        if input.result_status == "completed" {
            ensure_direct_completion_allowed(tx, task.id)?;
        }
        let completed_at = if input.result_status == "completed" {
            Some(input.handled_at.clone())
        } else {
            task.completed_at.clone()
        };
        tx.execute(
            "UPDATE tasks SET status=?,completed_at=?,updated_at=? WHERE id=?",
            params![input.result_status, completed_at, now(), input.task_id],
        )
        .map_err(display_error)?;
        add_status(
            tx,
            input.task_id,
            Some(&task.status),
            &input.result_status,
            "记录本次处理",
        )?;
    }
    if input.sync_status {
        if is_deferred_status(&input.result_status)
            || matches!(input.result_status.as_str(), "completed" | "cancelled")
        {
            stop_scheduled_on(
                tx,
                input.task_id,
                "处理活动同步状态",
                matches!(input.result_status.as_str(), "completed" | "cancelled"),
            )?;
        }
        if clears_urgent_status(&input.result_status) {
            clear_urgent_on(tx, input.task_id, "处理记录已同步事项状态")?;
        }
        close_active_queue(tx, input.task_id, "记录本次处理并同步状态")?;
    } else {
        tx.execute(
            "UPDATE tasks SET updated_at=? WHERE id=?",
            params![now(), input.task_id],
        )
        .map_err(display_error)?;
    }
    record_work_event_on(
        tx,
        input.task_id,
        &input.result_status,
        &input.handled_at,
        &task.task_type,
        "manual",
        &input.note,
    )?;
    add_log(
        tx,
        input.task_id,
        "work",
        &format!("记录本次处理：{}", input.result_status),
    )
}
pub(super) fn void_work_event_on(
    connection: &Connection,
    id: i64,
    confirm_historical_impact: bool,
) -> Result<(), String> {
    let tx = connection;

    let task_id: i64 = tx
        .query_row(
            "SELECT task_id FROM task_work_events WHERE id=? AND voided_at IS NULL",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(display_error)?
        .ok_or("处理活动不存在")?;
    let first_id: i64 = tx
        .query_row(
            "SELECT id FROM task_work_events WHERE task_id=? AND voided_at IS NULL
                     ORDER BY strftime('%s',handled_at),id LIMIT 1",
            [task_id],
            |row| row.get(0),
        )
        .map_err(display_error)?;
    if first_id == id && !confirm_historical_impact {
        return Err(
            "此操作将改变该事项的统计归属期间，并可能影响历史周报、月报或季度统计。是否继续？"
                .into(),
        );
    }
    tx.execute(
        "UPDATE task_work_events SET voided_at=?,updated_at=? WHERE id=?",
        params![now(), now(), id],
    )
    .map_err(display_error)?;
    tx.execute(
        "UPDATE tasks SET updated_at=? WHERE id=?",
        params![now(), task_id],
    )
    .map_err(display_error)?;
    add_log(tx, task_id, "audit", "作废一条结构化处理活动")
}
