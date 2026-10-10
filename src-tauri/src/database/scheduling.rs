//! Planned queue numbers are separate from real queue history and permanent identity.
use super::*;

pub(super) fn migrate_scheduling(connection: &Connection, version: i64) -> Result<(), String> {
    if version >= 9 {
        return Ok(());
    }
    for (name, definition) in [
        ("planned_date", "TEXT NOT NULL DEFAULT ''"),
        (
            "is_scheduled",
            "INTEGER NOT NULL DEFAULT 0 CHECK(is_scheduled IN (0,1))",
        ),
        ("schedule_action", "TEXT NOT NULL DEFAULT ''"),
        ("schedule_action_at", "TEXT"),
    ] {
        let exists: i64 = connection
            .query_row(
                "SELECT count(*) FROM pragma_table_info('tasks') WHERE name=?",
                [name],
                |row| row.get(0),
            )
            .map_err(display_error)?;
        if exists == 0 {
            connection
                .execute_batch(&format!("ALTER TABLE tasks ADD COLUMN {name} {definition}"))
                .map_err(display_error)?;
        }
    }
    connection.execute_batch(
        "UPDATE tasks SET planned_date=ticket_date WHERE planned_date='';
         CREATE TABLE IF NOT EXISTS queue_number_allocations(
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           task_id INTEGER REFERENCES tasks(id) ON DELETE SET NULL,
           permanent_number TEXT NOT NULL,
           queue_date TEXT NOT NULL,
           daily_sequence INTEGER NOT NULL,
           allocated_at TEXT NOT NULL,
           activated_at TEXT,
           voided_at TEXT,
           void_reason TEXT NOT NULL DEFAULT '',
           UNIQUE(queue_date,daily_sequence));
         CREATE INDEX IF NOT EXISTS idx_allocations_task ON queue_number_allocations(task_id,id);
         CREATE INDEX IF NOT EXISTS idx_scheduled_due ON tasks(is_scheduled,planned_date);
         INSERT OR IGNORE INTO queue_number_allocations(task_id,permanent_number,queue_date,daily_sequence,allocated_at,activated_at,voided_at,void_reason)
           SELECT t.id,t.permanent_number,q.queue_date,q.daily_sequence,q.created_at,q.enqueued_at,
             CASE WHEN t.ticket_date<>q.queue_date OR t.daily_sequence<>q.daily_sequence THEN q.updated_at ELSE NULL END,
             CASE WHEN t.ticket_date<>q.queue_date OR t.daily_sequence<>q.daily_sequence THEN '历史重新取号' ELSE '' END
           FROM task_queue_entries q JOIN tasks t ON t.id=q.task_id;
         INSERT OR IGNORE INTO queue_number_allocations(task_id,permanent_number,queue_date,daily_sequence,allocated_at)
           SELECT id,permanent_number,ticket_date,daily_sequence,created_at FROM tasks;
         INSERT INTO daily_sequences(ticket_date,last_sequence)
           SELECT queue_date,MAX(daily_sequence) FROM queue_number_allocations GROUP BY queue_date
           ON CONFLICT(ticket_date) DO UPDATE SET last_sequence=MAX(last_sequence,excluded.last_sequence);
         DELETE FROM schema_meta; INSERT INTO schema_meta(version) VALUES(9);"
    ).map_err(display_error)
}

pub(super) fn allocate_number_on(
    connection: &Connection,
    id: i64,
    date: &str,
    sequence: i64,
) -> Result<(), String> {
    let task = get_task_on(connection, id)?;
    connection.execute(
        "INSERT INTO queue_number_allocations(task_id,permanent_number,queue_date,daily_sequence,allocated_at) VALUES(?,?,?,?,?)",
        params![id,task.permanent_number,date,sequence,now()],
    ).map_err(display_error)?;
    Ok(())
}

pub(super) fn void_number_on(connection: &Connection, id: i64, reason: &str) -> Result<(), String> {
    let task = get_task_on(connection, id)?;
    let changed = connection
        .execute(
            "UPDATE queue_number_allocations SET voided_at=?,void_reason=?
         WHERE task_id=? AND queue_date=? AND daily_sequence=? AND voided_at IS NULL",
            params![now(), reason, id, task.ticket_date, task.daily_sequence],
        )
        .map_err(display_error)?;
    if changed > 0 {
        add_log(
            connection,
            id,
            "queue_number_voided",
            &format!(
                "{}-{:02} 已作废，永久不回收（{}）",
                task.ticket_date, task.daily_sequence, reason
            ),
        )?;
    }
    Ok(())
}

pub(super) fn validate_plan(
    date: &str,
    deadline: Option<&str>,
    changed: bool,
) -> Result<(), String> {
    let parsed = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| "加入日期格式无效".to_string())?;
    if parsed.format("%Y-%m-%d").to_string() != date || (changed && date < today().as_str()) {
        return Err("加入日期只能选择今天及未来日期".into());
    }
    if let Some(value) = deadline {
        let value = chrono::DateTime::parse_from_rfc3339(value)
            .map_err(|_| "截止时间格式无效".to_string())?;
        if value.with_timezone(&Local).date_naive() < parsed {
            return Err("截止时间不得早于加入日期，请修改或清空截止时间".into());
        }
    }
    Ok(())
}

pub(super) fn activate_number_on(
    connection: &Connection,
    id: i64,
    effective_at: &str,
) -> Result<(), String> {
    let task = get_task_on(connection, id)?;
    connection.execute("INSERT INTO task_queue_entries(task_id,queue_date,daily_sequence,requested_deadline,requested_deadline_label,enqueued_at,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)",
        params![id,task.ticket_date,task.daily_sequence,task.requested_deadline,task.requested_deadline_label,effective_at,now(),now()]).map_err(display_error)?;
    connection.execute("UPDATE queue_number_allocations SET activated_at=? WHERE task_id=? AND queue_date=? AND daily_sequence=? AND voided_at IS NULL",params![now(),id,task.ticket_date,task.daily_sequence]).map_err(display_error)?;
    Ok(())
}

pub(super) fn replan_on(
    connection: &Connection,
    id: i64,
    date: &str,
    deadline: Option<&str>,
    deadline_label: Option<&str>,
) -> Result<(), String> {
    let old = get_task_on(connection, id)?;
    close_active_queue(connection, id, "修改加入日期")?;
    void_number_on(connection, id, "修改加入日期")?;
    let sequence = next_daily_sequence(connection, date)?;
    let scheduled = date > today().as_str();
    let order: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(custom_sort_order),0)+1 FROM tasks",
            [],
            |row| row.get(0),
        )
        .map_err(display_error)?;
    connection.execute("UPDATE tasks SET planned_date=?,ticket_date=?,daily_sequence=?,is_scheduled=?,status='pending',archived_at=NULL,completed_at=NULL,started_at=NULL,requested_deadline=?,requested_deadline_label=?,schedule_action=?,schedule_action_at=?,custom_sort_order=?,updated_at=? WHERE id=?",
        params![date,date,sequence,scheduled as i64,deadline,deadline_label,if old.is_scheduled&&!scheduled{"early"}else{""},if old.is_scheduled&&!scheduled{Some(now())}else{None},order,now(),id]).map_err(display_error)?;
    allocate_number_on(connection, id, date, sequence)?;
    if old.archived_at.is_some() || matches!(old.status.as_str(), "completed" | "archived") {
        add_log(connection,id,"scheduled_reactivated",&format!("重新激活：原状态 {}；原完成时间 {:?}；原归档时间 {:?}；新计划 {}；新队列 {}-{:02}；清空原截止时间 {:?}；此前完成、归档及统计历史永久保留",old.status,old.completed_at,old.archived_at,date,date,sequence,old.requested_deadline))?;
    }
    add_log(
        connection,
        id,
        "schedule_changed",
        &format!(
            "加入日期：{} → {}；原队列 {}-{:02} 作废，分配 {}-{:02}{}",
            old.planned_date,
            date,
            old.ticket_date,
            old.daily_sequence,
            date,
            sequence,
            if scheduled {
                "；移出当前待办，转为未来事项"
            } else {
                "；加入今日待办"
            }
        ),
    )?;
    if old.status != "pending" {
        add_status(
            connection,
            id,
            Some(&old.status),
            "pending",
            "重新制定加入计划",
        )?;
    }
    if !scheduled {
        activate_number_on(connection, id, &now())?;
    }
    Ok(())
}

pub(super) fn activate_reserved_on(connection: &Connection, id: i64) -> Result<(), String> {
    let date = today();
    let task = get_task_on(connection, id)?;
    if task.has_active_queue {
        return Err("未来事项存在重复的有效入队记录".into());
    }
    let late = task.planned_date < date;
    let effective_at = if late {
        use chrono::TimeZone;
        let day = chrono::NaiveDate::parse_from_str(&task.planned_date, "%Y-%m-%d")
            .map_err(display_error)?;
        Local
            .from_local_datetime(&day.and_hms_opt(0, 0, 0).ok_or("计划日期无效")?)
            .earliest()
            .ok_or("计划日期的本地时间无效")?
            .to_rfc3339()
    } else {
        now()
    };
    activate_number_on(connection, id, &effective_at)?;
    // Insert reserved slots before later numbers without urgent promotion.
    let before:Option<i64>=connection.query_row("SELECT MIN(custom_sort_order) FROM tasks WHERE ticket_date=? AND daily_sequence>? AND EXISTS(SELECT 1 FROM task_queue_entries q WHERE q.task_id=tasks.id AND q.closed_at IS NULL)",params![task.ticket_date,task.daily_sequence],|row|row.get(0)).map_err(display_error)?;
    let order = if let Some(value) = before {
        connection
            .execute(
                "UPDATE tasks SET custom_sort_order=custom_sort_order+1 WHERE custom_sort_order>=?",
                [value],
            )
            .map_err(display_error)?;
        value
    } else {
        connection
            .query_row(
                "SELECT COALESCE(MAX(custom_sort_order),0)+1 FROM tasks",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(display_error)?
    };
    connection.execute("UPDATE tasks SET is_scheduled=0,schedule_action=?,schedule_action_at=?,custom_sort_order=?,updated_at=? WHERE id=?",params![if late{"late"}else{"planned"},now(),order,now(),id]).map_err(display_error)?;
    add_log(
        connection,
        id,
        if late {
            "scheduled_late"
        } else {
            "scheduled_activated"
        },
        &format!(
            "{}：原计划日期 {}；保留队列编号 {}-{:02}；入队统计归 {}；实际执行 {}",
            if late {
                "延迟自动入队"
            } else {
                "计划事项自动入队"
            },
            task.planned_date,
            task.ticket_date,
            task.daily_sequence,
            task.planned_date,
            now()
        ),
    )?;

    Ok(())
}

impl Database {
    /// Idempotent lifecycle check. Reporting connections never call this write operation.
    pub fn activate_due_scheduled(&self) -> Result<usize, String> {
        if self.with_conn(|connection| {
            connection
                .query_row("PRAGMA query_only", [], |row| row.get::<_, bool>(0))
                .map_err(display_error)
        })? {
            return Ok(0);
        }
        self.with_transaction(|tx| {
            let date=today();
            let ids={
                let mut statement=tx.prepare("SELECT id FROM tasks WHERE is_scheduled=1 AND planned_date<=? AND status='pending' AND deleted_at IS NULL AND archived_at IS NULL ORDER BY planned_date,daily_sequence,id").map_err(display_error)?;
                let ids=statement.query_map([&date],|row|row.get::<_,i64>(0)).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;
                ids
            };
            for id in &ids { activate_reserved_on(tx,*id)?; }
            Ok(ids.len())
        })
    }
}

pub(super) fn enter_current_workflow_on(
    connection: &Connection,
    id: i64,
) -> Result<LegalTask, String> {
    let task = get_task_on(connection, id)?;
    if !task.is_scheduled {
        return Ok(task);
    }
    if task.deleted_at.is_some()
        || task.archived_at.is_some()
        || matches!(task.status.as_str(), "completed" | "archived" | "cancelled")
    {
        return Err("终态事项不能提前入队".into());
    }
    if task.planned_date > today() {
        replan_on(
            connection,
            id,
            &today(),
            task.requested_deadline.as_deref(),
            task.requested_deadline_label.as_deref(),
        )?;
        add_log(
            connection,
            id,
            "scheduled_early",
            &format!(
                "提前进入当天工作流：原计划 {}，原队列 {}-{:02} 已作废；新今日编号 {}-{:02}",
                task.planned_date,
                task.ticket_date,
                task.daily_sequence,
                today(),
                get_task_on(connection, id)?.daily_sequence
            ),
        )?;
    } else {
        activate_reserved_on(connection, id)?;
    }
    get_task_on(connection, id)
}

pub(super) fn stop_scheduled_on(
    connection: &Connection,
    id: i64,
    reason: &str,
    void: bool,
) -> Result<(), String> {
    let task = get_task_on(connection, id)?;
    if task.is_scheduled {
        if void {
            void_number_on(connection, id, reason)?;
        }
        connection.execute("UPDATE tasks SET is_scheduled=0,schedule_action='',schedule_action_at=NULL WHERE id=?",[id]).map_err(display_error)?;
        add_log(
            connection,
            id,
            "scheduled_stopped",
            &format!("停止计划自动入队：原计划 {}；{}", task.planned_date, reason),
        )?;
    }
    Ok(())
}

pub(super) fn merge_sequence_watermarks_on(
    source: &Connection,
    target: &Connection,
) -> Result<(), String> {
    let mut query = source
        .prepare("SELECT ticket_date,last_sequence FROM daily_sequences")
        .map_err(display_error)?;
    for row in query
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(display_error)?
    {
        let (date, sequence) = row.map_err(display_error)?;
        reserve_daily_sequence(target, &date, sequence)?;
    }
    Ok(())
}

/// Preserve source number evidence even when another local task owns the original pair.
pub(super) fn merge_number_history_on(
    source: &Connection,
    target: &Connection,
    ids: &HashMap<i64, i64>,
    inserted: &HashSet<i64>,
) -> Result<(), String> {
    let mut query=source.prepare("SELECT task_id,permanent_number,queue_date,daily_sequence,allocated_at,activated_at,voided_at,void_reason FROM queue_number_allocations ORDER BY id").map_err(display_error)?;
    let rows = query
        .query_map([], |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
            ))
        })
        .map_err(display_error)?;
    for row in rows {
        let (source_id, permanent, date, sequence, allocated, activated, voided, reason) =
            row.map_err(display_error)?;
        let id = source_id.and_then(|value| ids.get(&value).copied());
        if let (Some(source_task_id), Some(target_task_id)) = (source_id, id) {
            if inserted.contains(&target_task_id) {
                let original = get_task_on(source, source_task_id)?;
                if original.ticket_date == date && original.daily_sequence == sequence {
                    let current = get_task_on(target, target_task_id)?;
                    target.execute("UPDATE queue_number_allocations SET activated_at=COALESCE(activated_at,?),voided_at=COALESCE(voided_at,?),void_reason=CASE WHEN void_reason='' THEN ? ELSE void_reason END WHERE task_id=? AND queue_date=? AND daily_sequence=?",params![activated,voided,reason,target_task_id,current.ticket_date,current.daily_sequence]).map_err(display_error)?;
                }
            }
        }
        let existing:Option<(Option<i64>,String,String)>=target.query_row("SELECT task_id,permanent_number,allocated_at FROM queue_number_allocations WHERE queue_date=? AND daily_sequence=?",params![date,sequence],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).optional().map_err(display_error)?;
        if let Some((owner, number, stamp)) = &existing {
            if owner == &id && number == &permanent && (id.is_some() || stamp == &allocated) {
                // Preserve existing local dispositions. Source events were merged separately.
                continue;
            }
        }
        let (saved_sequence, saved_voided, saved_reason) = if existing.is_some() {
            let marker = format!(
                "备份原号 {}-{:02}；固定编号 {}；分配时间 {}；原激活 {:?}；原作废 {:?}；原原因 {}",
                date, sequence, permanent, allocated, activated, voided, reason
            );
            let copied:bool=target.query_row("SELECT EXISTS(SELECT 1 FROM queue_number_allocations WHERE permanent_number=? AND allocated_at=? AND void_reason=?)",params![permanent,allocated,marker],|row|row.get(0)).map_err(display_error)?;
            if copied {
                continue;
            }
            (
                next_import_sequence(target, &date)?,
                Some(voided.clone().unwrap_or_else(now)),
                marker,
            )
        } else {
            (sequence, voided.clone(), reason.clone())
        };
        reserve_daily_sequence(target, &date, saved_sequence)?;
        target.execute("INSERT INTO queue_number_allocations(task_id,permanent_number,queue_date,daily_sequence,allocated_at,activated_at,voided_at,void_reason) VALUES(?,?,?,?,?,?,?,?)",params![id,permanent,date,saved_sequence,allocated,activated,saved_voided,saved_reason]).map_err(display_error)?;
    }
    // Imported active history may have reallocated the current number after task creation.
    target.execute("UPDATE queue_number_allocations SET voided_at=COALESCE(voided_at,?),void_reason=CASE WHEN void_reason='' THEN '导入时调整当前队列号码' ELSE void_reason END WHERE task_id IN (SELECT id FROM tasks) AND voided_at IS NULL AND NOT EXISTS(SELECT 1 FROM tasks t WHERE t.id=queue_number_allocations.task_id AND t.ticket_date=queue_number_allocations.queue_date AND t.daily_sequence=queue_number_allocations.daily_sequence)",[now()]).map_err(display_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_preserves_identity_and_allocations_never_recycle() {
        let path = std::env::temp_dir().join(format!(
            "inline-schedule-foundation-{}.db",
            std::process::id()
        ));
        let db = Database::open_at(path).unwrap();
        db.with_transaction(|tx| {
            let date = "2080-10-05";
            let one = next_daily_sequence(tx,date)?;
            tx.execute("INSERT INTO queue_number_allocations(permanent_number,queue_date,daily_sequence,allocated_at,voided_at) VALUES('deleted-task',?,?,?,?)",params![date,one,now(),now()]).map_err(display_error)?;
            tx.execute("DELETE FROM daily_sequences WHERE ticket_date=?",[date]).map_err(display_error)?;
            assert_eq!(next_daily_sequence(tx,date)?,one+1);
            assert_eq!(Database::schema_version(tx)?,10);
            Ok(())
        }).unwrap();
    }
}
