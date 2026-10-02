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
    connection.execute("UPDATE tasks SET planned_date=?,ticket_date=?,daily_sequence=?,is_scheduled=?,status='pending',requested_deadline=?,requested_deadline_label=?,schedule_action=?,schedule_action_at=?,custom_sort_order=?,updated_at=? WHERE id=?",
        params![date,date,sequence,scheduled as i64,deadline,deadline_label,if old.is_scheduled&&!scheduled{"early"}else{""},if old.is_scheduled&&!scheduled{Some(now())}else{None},order,now(),id]).map_err(display_error)?;
    allocate_number_on(connection, id, date, sequence)?;
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
            assert_eq!(Database::schema_version(tx)?,9);
            Ok(())
        }).unwrap();
    }
}
