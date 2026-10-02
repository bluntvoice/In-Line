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
