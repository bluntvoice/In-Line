use crate::models::*;
use chrono::{Datelike, FixedOffset, Local, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

const SELECT_TASK: &str = "SELECT tasks.id, permanent_number, daily_sequence, ticket_date, department, contact, task_type, title, details, status, priority, workload, is_urgent, urgent_requester, urgent_reason, requested_deadline, internal_notes, created_at, updated_at, started_at, completed_at, archived_at, deleted_at, custom_sort_order, requested_deadline_label,
    (SELECT count(*) FROM task_work_events work WHERE work.task_id=tasks.id AND work.voided_at IS NULL),
    EXISTS(SELECT 1 FROM task_queue_entries queue_entry WHERE queue_entry.task_id=tasks.id AND queue_entry.closed_at IS NULL)
    FROM tasks";
const OVERDUE_RANK_SQL: &str = "CASE WHEN requested_deadline IS NOT NULL AND strftime('%s',requested_deadline) < strftime('%s','now') THEN 0 ELSE 1 END";

pub struct Database {
    path: PathBuf,
    backup_dir: PathBuf,
    connection: Mutex<Option<Connection>>,
}

impl Database {
    pub fn open() -> Result<Self, String> {
        let root = dirs::config_dir()
            .ok_or("无法定位应用数据目录")?
            .join("in-line");
        fs::create_dir_all(&root).map_err(display_error)?;
        let path = root.join("inline.db");
        let backup_dir = root.join("backups");
        fs::create_dir_all(&backup_dir).map_err(display_error)?;
        Self::normalize_backup_names(&backup_dir)?;
        let existed = path.exists();
        let mut connection = Self::connect(&path)?;
        if existed && Self::schema_version(&connection)? < 6 {
            let backup = backup_dir.join(Self::backup_name("before-migration"));
            Self::backup_connection(&connection, &backup)?;
        }
        Self::migrate(&mut connection)?;
        let date_marker = Local::now().format("%Y%m%d").to_string();
        let daily_exists = fs::read_dir(&backup_dir)
            .map_err(display_error)?
            .filter_map(Result::ok)
            .any(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                name.starts_with(&format!("InLine-backup-{date_marker}"))
                    && name.ends_with("-auto.db")
            });
        if !daily_exists {
            let daily = backup_dir.join(Self::backup_name("auto"));
            Self::backup_connection(&connection, &daily)?;
        }
        Self::prune_backups(&backup_dir, 30)?;
        Ok(Self {
            path,
            backup_dir,
            connection: Mutex::new(Some(connection)),
        })
    }

    #[cfg(test)]
    pub fn open_at(path: PathBuf) -> Result<Self, String> {
        let backup_dir = path.parent().unwrap().join("backups");
        fs::create_dir_all(&backup_dir).map_err(display_error)?;
        let mut connection = Self::connect(&path)?;
        Self::migrate(&mut connection)?;
        Ok(Self {
            path,
            backup_dir,
            connection: Mutex::new(Some(connection)),
        })
    }

    fn connect(path: &Path) -> Result<Connection, String> {
        let connection = Connection::open(path).map_err(display_error)?;
        connection
            .execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;",
            )
            .map_err(display_error)?;
        Ok(connection)
    }

    fn schema_version(connection: &Connection) -> Result<i64, String> {
        let exists: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='schema_meta'",
                [],
                |row| row.get(0),
            )
            .map_err(display_error)?;
        if exists == 0 {
            return Ok(0);
        }
        connection
            .query_row(
                "SELECT COALESCE(MAX(version),0) FROM schema_meta",
                [],
                |row| row.get(0),
            )
            .map_err(display_error)
    }

    fn migrate(connection: &mut Connection) -> Result<(), String> {
        let transaction = connection.transaction().map_err(display_error)?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_meta(version INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS daily_sequences(ticket_date TEXT PRIMARY KEY,last_sequence INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS tasks(
               id INTEGER PRIMARY KEY AUTOINCREMENT, permanent_number TEXT NOT NULL UNIQUE,
               daily_sequence INTEGER NOT NULL, ticket_date TEXT NOT NULL, department TEXT NOT NULL,
               contact TEXT NOT NULL, task_type TEXT NOT NULL, title TEXT NOT NULL, details TEXT NOT NULL,
               status TEXT NOT NULL DEFAULT 'pending', priority TEXT NOT NULL DEFAULT 'normal',
               workload TEXT NOT NULL DEFAULT 'standard', is_urgent INTEGER NOT NULL DEFAULT 0,
               urgent_requester TEXT NOT NULL DEFAULT '', urgent_reason TEXT NOT NULL DEFAULT '',
               requested_deadline TEXT, internal_notes TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL,
               updated_at TEXT NOT NULL, started_at TEXT, completed_at TEXT, archived_at TEXT,
               deleted_at TEXT, custom_sort_order INTEGER NOT NULL DEFAULT 0,
               UNIQUE(ticket_date,daily_sequence));
             CREATE TABLE IF NOT EXISTS task_logs(
               id INTEGER PRIMARY KEY AUTOINCREMENT, task_id INTEGER NOT NULL, log_type TEXT NOT NULL,
               content TEXT NOT NULL, created_at TEXT NOT NULL,
               FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE);
             CREATE TABLE IF NOT EXISTS master_values(
               id INTEGER PRIMARY KEY AUTOINCREMENT, kind TEXT NOT NULL, name TEXT NOT NULL,
               sort_order INTEGER NOT NULL DEFAULT 0, is_active INTEGER NOT NULL DEFAULT 1,
               usage_count INTEGER NOT NULL DEFAULT 0, manual_order INTEGER, UNIQUE(kind,name));
             CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS status_history(
               id INTEGER PRIMARY KEY AUTOINCREMENT, task_id INTEGER NOT NULL, old_status TEXT,
               new_status TEXT NOT NULL, reason TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL,
               FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE);
             CREATE TABLE IF NOT EXISTS urgent_records(
               id INTEGER PRIMARY KEY AUTOINCREMENT, task_id INTEGER NOT NULL, requester TEXT NOT NULL,
               reason TEXT NOT NULL, requested_deadline TEXT, requested_at TEXT NOT NULL,
               confirmation_status TEXT NOT NULL DEFAULT 'confirmed', confirmed_at TEXT, cancelled_at TEXT,
               notes TEXT NOT NULL DEFAULT '', FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE);
             CREATE INDEX IF NOT EXISTS idx_tasks_queue ON tasks(deleted_at,archived_at,status,custom_sort_order);
             CREATE INDEX IF NOT EXISTS idx_logs_task ON task_logs(task_id,created_at DESC);"
        ).map_err(display_error)?;
        let version: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(version),0) FROM schema_meta",
                [],
                |row| row.get(0),
            )
            .map_err(display_error)?;
        if version < 2 {
            let mut statement = transaction.prepare(
                "SELECT id FROM tasks WHERE deleted_at IS NULL AND archived_at IS NULL
                 AND status NOT IN ('completed','cancelled','archived')
                 ORDER BY CASE WHEN is_urgent=1 THEN 0 ELSE 1 END,
                 CASE priority WHEN 'critical' THEN 0 WHEN 'urgent' THEN 1 WHEN 'elevated' THEN 2 ELSE 3 END,
                 CASE WHEN requested_deadline IS NOT NULL AND requested_deadline < datetime('now') THEN 0 ELSE 1 END,
                 ticket_date,daily_sequence"
            ).map_err(display_error)?;
            let ids = statement
                .query_map([], |row| row.get::<_, i64>(0))
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;
            drop(statement);
            for (index, id) in ids.into_iter().enumerate() {
                transaction
                    .execute(
                        "UPDATE tasks SET custom_sort_order=? WHERE id=?",
                        params![index as i64 + 1, id],
                    )
                    .map_err(display_error)?;
            }
            transaction
                .execute("DELETE FROM schema_meta", [])
                .map_err(display_error)?;
            transaction
                .execute("INSERT INTO schema_meta(version) VALUES(2)", [])
                .map_err(display_error)?;
        }
        if version < 3 {
            let has_deadline_label: i64 = transaction
                .query_row(
                    "SELECT count(*) FROM pragma_table_info('tasks') WHERE name='requested_deadline_label'",
                    [],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            if has_deadline_label == 0 {
                transaction
                    .execute(
                        "ALTER TABLE tasks ADD COLUMN requested_deadline_label TEXT",
                        [],
                    )
                    .map_err(display_error)?;
            }
            transaction
                .execute("DELETE FROM schema_meta", [])
                .map_err(display_error)?;
            transaction
                .execute("INSERT INTO schema_meta(version) VALUES(3)", [])
                .map_err(display_error)?;
        }
        if version < 4 {
            let has_usage_count: i64 = transaction
                .query_row(
                    "SELECT count(*) FROM pragma_table_info('master_values') WHERE name='usage_count'",
                    [],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            if has_usage_count == 0 {
                transaction
                    .execute(
                        "ALTER TABLE master_values ADD COLUMN usage_count INTEGER NOT NULL DEFAULT 0",
                        [],
                    )
                    .map_err(display_error)?;
            }
            let has_manual_order: i64 = transaction
                .query_row(
                    "SELECT count(*) FROM pragma_table_info('master_values') WHERE name='manual_order'",
                    [],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            if has_manual_order == 0 {
                transaction
                    .execute(
                        "ALTER TABLE master_values ADD COLUMN manual_order INTEGER",
                        [],
                    )
                    .map_err(display_error)?;
            }
            transaction
                .execute_batch(
                    "UPDATE master_values SET usage_count=(
                       SELECT count(*) FROM tasks
                       WHERE (master_values.kind='department' AND trim(tasks.department)=master_values.name)
                          OR (master_values.kind='task_type' AND trim(tasks.task_type)=master_values.name)
                          OR (master_values.kind='contact' AND trim(tasks.contact)=master_values.name)
                     );
                     DELETE FROM schema_meta;
                     INSERT INTO schema_meta(version) VALUES(4);",
                )
                .map_err(display_error)?;
        }
        if version < 5 {
            transaction
                .execute_batch(
                    "CREATE TABLE IF NOT EXISTS task_queue_entries(
                       id INTEGER PRIMARY KEY AUTOINCREMENT,
                       task_id INTEGER NOT NULL,
                       queue_date TEXT NOT NULL,
                       daily_sequence INTEGER NOT NULL,
                       requested_deadline TEXT,
                       requested_deadline_label TEXT,
                       enqueued_at TEXT NOT NULL,
                       closed_at TEXT,
                       close_reason TEXT NOT NULL DEFAULT '',
                       created_at TEXT NOT NULL,
                       updated_at TEXT NOT NULL,
                       FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE,
                       UNIQUE(queue_date,daily_sequence)
                     );
                     CREATE TABLE IF NOT EXISTS task_work_events(
                       id INTEGER PRIMARY KEY AUTOINCREMENT,
                       task_id INTEGER NOT NULL,
                       result_status TEXT NOT NULL,
                       handled_at TEXT NOT NULL,
                       task_type_snapshot TEXT NOT NULL,
                       source TEXT NOT NULL,
                       note TEXT NOT NULL DEFAULT '',
                       created_at TEXT NOT NULL,
                       updated_at TEXT NOT NULL,
                       voided_at TEXT,
                       FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
                     );
                     CREATE UNIQUE INDEX IF NOT EXISTS idx_queue_one_active_task
                       ON task_queue_entries(task_id) WHERE closed_at IS NULL;
                     CREATE INDEX IF NOT EXISTS idx_queue_active
                       ON task_queue_entries(closed_at,queue_date,daily_sequence);
                     CREATE INDEX IF NOT EXISTS idx_work_events_range
                       ON task_work_events(handled_at,task_id) WHERE voided_at IS NULL;
                     CREATE INDEX IF NOT EXISTS idx_work_events_task
                       ON task_work_events(task_id,handled_at DESC) WHERE voided_at IS NULL;
                     INSERT OR IGNORE INTO task_queue_entries(
                       task_id,queue_date,daily_sequence,requested_deadline,requested_deadline_label,
                       enqueued_at,closed_at,close_reason,created_at,updated_at
                     )
                     SELECT id,ticket_date,daily_sequence,requested_deadline,requested_deadline_label,
                       created_at,
                       CASE WHEN deleted_at IS NOT NULL OR archived_at IS NOT NULL
                                  OR status IN ('waiting_materials','waiting_confirmation','waiting_counterparty_confirmation','paused','processed','completed','cancelled','archived')
                            THEN COALESCE(completed_at,archived_at,deleted_at,updated_at) ELSE NULL END,
                       CASE WHEN deleted_at IS NOT NULL THEN 'deleted'
                            WHEN archived_at IS NOT NULL OR status='archived' THEN 'archived'
                            WHEN status='completed' THEN 'completed'
                            WHEN status IN ('waiting_materials','waiting_confirmation','waiting_counterparty_confirmation','paused','processed') THEN 'deferred'
                            WHEN status='cancelled' THEN 'cancelled' ELSE '' END,
                       created_at,updated_at
                     FROM tasks;
                     INSERT INTO task_work_events(
                       task_id,result_status,handled_at,task_type_snapshot,source,note,created_at,updated_at
                     )
                     SELECT history.task_id,history.new_status,history.created_at,tasks.task_type,
                       'status_change','',history.created_at,history.created_at
                     FROM status_history history
                     JOIN tasks ON tasks.id=history.task_id
                     WHERE history.new_status IN ('completed','waiting_materials','waiting_confirmation','waiting_counterparty_confirmation');
                     INSERT INTO task_work_events(
                       task_id,result_status,handled_at,task_type_snapshot,source,note,created_at,updated_at
                     )
                     SELECT tasks.id,'completed',tasks.completed_at,tasks.task_type,'status_change','',tasks.completed_at,tasks.completed_at
                     FROM tasks
                     WHERE tasks.completed_at IS NOT NULL
                       AND NOT EXISTS(
                         SELECT 1 FROM task_work_events event
                         WHERE event.task_id=tasks.id AND event.result_status='completed'
                       );
                     DELETE FROM schema_meta;
                     INSERT INTO schema_meta(version) VALUES(5);",
                )
                .map_err(display_error)?;
        }
        if version < 6 {
            let stamp = now();
            let handled_condition = "is_urgent=1 AND (deleted_at IS NOT NULL OR status IN ('waiting_materials','waiting_confirmation','waiting_counterparty_confirmation','paused','processed','completed'))";
            transaction
                .execute(
                    &format!(
                        "UPDATE urgent_records SET cancelled_at=? WHERE cancelled_at IS NULL
                         AND task_id IN (SELECT id FROM tasks WHERE {handled_condition})"
                    ),
                    [stamp.clone()],
                )
                .map_err(display_error)?;
            transaction
                .execute(
                    &format!(
                        "INSERT INTO task_logs(task_id,log_type,content,created_at)
                         SELECT id,'urgent','取消加急：事项已完成、进入暂缓队列或回收站',?
                         FROM tasks WHERE {handled_condition}"
                    ),
                    [stamp.clone()],
                )
                .map_err(display_error)?;
            transaction
                .execute(
                    &format!("UPDATE tasks SET is_urgent=0 WHERE {handled_condition}"),
                    [],
                )
                .map_err(display_error)?;
            transaction
                .execute_batch(
                    "DELETE FROM schema_meta; INSERT INTO schema_meta(version) VALUES(6);",
                )
                .map_err(display_error)?;
        }
        let count: i64 = transaction
            .query_row(
                "SELECT count(*) FROM master_values WHERE kind='task_type'",
                [],
                |row| row.get(0),
            )
            .map_err(display_error)?;
        if count == 0 {
            for (index, name) in [
                "任务处理",
                "资料审核",
                "咨询答复",
                "文本起草",
                "问题排查",
                "沟通协调",
                "其他",
            ]
            .iter()
            .enumerate()
            {
                transaction.execute("INSERT OR IGNORE INTO master_values(kind,name,sort_order) VALUES('task_type',?,?)", params![name, index]).map_err(display_error)?;
            }
        }
        let stored_contacts = {
            let mut statement = transaction
                .prepare("SELECT contact FROM tasks WHERE trim(contact)<>''")
                .map_err(display_error)?;
            let values = statement
                .query_map([], |row| row.get::<_, String>(0))
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;
            values
        };
        for stored in stored_contacts {
            for contact in parse_contacts(&stored) {
                ensure_master(&transaction, "contact", &contact)?;
            }
        }
        transaction.commit().map_err(display_error)
    }

    fn with_conn<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let guard = self
            .connection
            .lock()
            .map_err(|_| "数据库正忙，请稍后重试".to_string())?;
        operation(guard.as_ref().ok_or("数据库尚未打开")?)
    }

    fn row_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<LegalTask> {
        let stored_department: String = row.get(4)?;
        let departments = parse_contacts(&stored_department);
        let stored_contact: String = row.get(5)?;
        let contacts = parse_contacts(&stored_contact);
        Ok(LegalTask {
            id: row.get(0)?,
            permanent_number: row.get(1)?,
            daily_sequence: row.get(2)?,
            ticket_date: row.get(3)?,
            department: departments.join("、"),
            departments,
            contact: contacts.join("、"),
            contacts,
            task_type: row.get(6)?,
            title: row.get(7)?,
            details: row.get(8)?,
            status: row.get(9)?,
            priority: row.get(10)?,
            workload: row.get(11)?,
            is_urgent: row.get::<_, i64>(12)? != 0,
            urgent_requester: row.get(13)?,
            urgent_reason: row.get(14)?,
            requested_deadline: row.get(15)?,
            internal_notes: row.get(16)?,
            created_at: row.get(17)?,
            updated_at: row.get(18)?,
            started_at: row.get(19)?,
            completed_at: row.get(20)?,
            archived_at: row.get(21)?,
            deleted_at: row.get(22)?,
            custom_sort_order: row.get(23)?,
            requested_deadline_label: row.get(24)?,
            processing_rounds: row.get(25)?,
            has_active_queue: row.get::<_, i64>(26)? != 0,
        })
    }

    pub fn list_tasks(&self, view: TaskView) -> Result<Vec<LegalTask>, String> {
        self.with_conn(|connection| {
            let condition = match view {
                TaskView::Queue => "deleted_at IS NULL AND archived_at IS NULL AND status NOT IN ('completed','cancelled','archived')",
                TaskView::Archive => "deleted_at IS NULL AND (archived_at IS NOT NULL OR status IN ('completed','cancelled','archived'))",
                TaskView::Trash => "deleted_at IS NOT NULL",
            };
            let order = if matches!(view, TaskView::Queue) { format!("{OVERDUE_RANK_SQL} ASC,custom_sort_order ASC,id ASC") } else { "updated_at DESC".into() };
            let mut statement = connection.prepare(&format!("{SELECT_TASK} WHERE {condition} ORDER BY {order}")).map_err(display_error)?;
            let rows=statement.query_map([], Self::row_task).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error);
            rows
        })
    }

    pub fn get_task(&self, id: i64) -> Result<LegalTask, String> {
        self.with_conn(|connection| get_task_on(connection, id))
    }
}

fn get_task_on(connection: &Connection, id: i64) -> Result<LegalTask, String> {
    connection
        .query_row(
            &format!("{SELECT_TASK} WHERE id=?"),
            [id],
            Database::row_task,
        )
        .optional()
        .map_err(display_error)?
        .ok_or("事项不存在或已被移除".into())
}

fn queue_ahead_on(connection: &Connection, id: i64) -> Result<i64, String> {
    connection
        .query_row(
            "WITH target AS (
                 SELECT id AS target_id,custom_sort_order AS target_order,
                        CASE WHEN requested_deadline IS NOT NULL AND strftime('%s',requested_deadline) < strftime('%s','now') THEN 0 ELSE 1 END AS target_rank
                 FROM tasks WHERE id=? AND EXISTS(
                   SELECT 1 FROM task_queue_entries entry WHERE entry.task_id=tasks.id AND entry.closed_at IS NULL
                 )
             )
             SELECT count(*) FROM tasks,target
             WHERE deleted_at IS NULL AND archived_at IS NULL
                AND status NOT IN ('completed','cancelled','archived')
                AND EXISTS(SELECT 1 FROM task_queue_entries entry WHERE entry.task_id=tasks.id AND entry.closed_at IS NULL)
               AND (
                 CASE WHEN requested_deadline IS NOT NULL AND strftime('%s',requested_deadline) < strftime('%s','now') THEN 0 ELSE 1 END < target_rank
                 OR (
                   CASE WHEN requested_deadline IS NOT NULL AND strftime('%s',requested_deadline) < strftime('%s','now') THEN 0 ELSE 1 END = target_rank
                   AND (custom_sort_order < target_order OR (custom_sort_order = target_order AND id < target_id))
                 )
               )",
            [id],
            |row| row.get(0),
        )
        .map_err(display_error)
}

fn now() -> String {
    Utc::now().to_rfc3339()
}
fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}
fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn parse_contacts(stored: &str) -> Vec<String> {
    let parsed =
        serde_json::from_str::<Vec<String>>(stored).unwrap_or_else(|_| vec![stored.into()]);
    let mut contacts = Vec::new();
    for value in parsed {
        let name = value.trim();
        if !name.is_empty() && !contacts.iter().any(|existing| existing == name) {
            contacts.push(name.to_string());
        }
    }
    contacts
}
fn contact_storage(contacts: &[String]) -> Result<String, String> {
    serde_json::to_string(contacts).map_err(display_error)
}

const WORK_EVENT_STATUSES: [&str; 5] = [
    "processed",
    "completed",
    "waiting_materials",
    "waiting_confirmation",
    "waiting_counterparty_confirmation",
];

fn is_work_event_status(status: &str) -> bool {
    WORK_EVENT_STATUSES.contains(&status)
}

fn is_deferred_status(status: &str) -> bool {
    matches!(
        status,
        "processed"
            | "waiting_materials"
            | "waiting_confirmation"
            | "waiting_counterparty_confirmation"
            | "paused"
    )
}

fn clears_urgent_status(status: &str) -> bool {
    status == "completed" || is_deferred_status(status)
}

fn validate_handled_at(value: &str) -> Result<(), String> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|_| ())
        .map_err(|_| "处理时间格式无效".to_string())
}

fn record_work_event_on(
    connection: &Connection,
    task_id: i64,
    result_status: &str,
    handled_at: &str,
    task_type_snapshot: &str,
    source: &str,
    note: &str,
) -> Result<i64, String> {
    if !is_work_event_status(result_status) {
        return Err("处理结果无效".into());
    }
    validate_handled_at(handled_at)?;
    if note.chars().count() > 2_000 {
        return Err("处理说明不能超过 2000 个字符".into());
    }
    let stamp = now();
    connection
        .execute(
            "INSERT INTO task_work_events(task_id,result_status,handled_at,task_type_snapshot,source,note,created_at,updated_at)
             VALUES(?,?,?,?,?,?,?,?)",
            params![
                task_id,
                result_status,
                handled_at,
                task_type_snapshot,
                source,
                note.trim(),
                stamp,
                stamp
            ],
        )
        .map_err(display_error)?;
    Ok(connection.last_insert_rowid())
}

fn close_active_queue(
    connection: &Connection,
    task_id: i64,
    reason: &str,
) -> Result<Option<(String, i64)>, String> {
    let active: Option<(i64, String, i64)> = connection
        .query_row(
            "SELECT id,queue_date,daily_sequence FROM task_queue_entries
             WHERE task_id=? AND closed_at IS NULL LIMIT 1",
            [task_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(display_error)?;
    let Some((entry_id, queue_date, daily_sequence)) = active else {
        return Ok(None);
    };
    let stamp = now();
    connection
        .execute(
            "UPDATE task_queue_entries SET closed_at=?,close_reason=?,updated_at=? WHERE id=?",
            params![stamp, reason, stamp, entry_id],
        )
        .map_err(display_error)?;
    add_log(
        connection,
        task_id,
        "queue",
        &format!(
            "退出 {} 队列：{:02}（{}）",
            queue_date, daily_sequence, reason
        ),
    )?;
    Ok(Some((queue_date, daily_sequence)))
}

fn next_daily_sequence(connection: &Connection, date: &str) -> Result<i64, String> {
    let sequence: i64 = connection
        .query_row(
            "SELECT last_sequence FROM daily_sequences WHERE ticket_date=?",
            [date],
            |row| row.get(0),
        )
        .optional()
        .map_err(display_error)?
        .unwrap_or(0)
        + 1;
    connection
        .execute(
            "INSERT INTO daily_sequences(ticket_date,last_sequence) VALUES(?,?)
             ON CONFLICT(ticket_date) DO UPDATE SET last_sequence=excluded.last_sequence",
            params![date, sequence],
        )
        .map_err(display_error)?;
    Ok(sequence)
}

fn enqueue_on(
    connection: &Connection,
    task_id: i64,
    target_status: &str,
    inherit_deadline: bool,
    supplied_deadline: Option<(Option<String>, Option<String>)>,
    reason: &str,
    reopen: bool,
) -> Result<(String, i64), String> {
    if !matches!(target_status, "pending" | "processing") {
        return Err("加入队列后的状态必须为待处理或处理中".into());
    }
    let task = get_task_on(connection, task_id)?;
    if task.has_active_queue {
        return Err("该事项已在有效队列中".into());
    }
    if !reopen
        && (task.deleted_at.is_some()
            || task.archived_at.is_some()
            || matches!(task.status.as_str(), "completed" | "archived"))
    {
        return Err("已完成或已归档事项请使用重新开启操作".into());
    }
    let date = today();
    let sequence = next_daily_sequence(connection, &date)?;
    let inherited: Option<(Option<String>, Option<String>)> = if inherit_deadline {
        connection
            .query_row(
                "SELECT requested_deadline,requested_deadline_label FROM task_queue_entries
                 WHERE task_id=? ORDER BY id DESC LIMIT 1",
                [task_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(display_error)?
    } else {
        None
    };
    let (deadline, deadline_label) = supplied_deadline.or(inherited).unwrap_or((None, None));
    let order: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(custom_sort_order),0)+1 FROM tasks",
            [],
            |row| row.get(0),
        )
        .map_err(display_error)?;
    let stamp = now();
    connection
        .execute(
            "INSERT INTO task_queue_entries(
               task_id,queue_date,daily_sequence,requested_deadline,requested_deadline_label,
               enqueued_at,created_at,updated_at
             ) VALUES(?,?,?,?,?,?,?,?)",
            params![
                task_id,
                date,
                sequence,
                deadline,
                deadline_label,
                stamp,
                stamp,
                stamp
            ],
        )
        .map_err(display_error)?;
    connection
        .execute(
            "UPDATE tasks SET daily_sequence=?,ticket_date=?,requested_deadline=?,requested_deadline_label=?,
             status=?,archived_at=CASE WHEN ? THEN NULL ELSE archived_at END,
             deleted_at=CASE WHEN ? THEN NULL ELSE deleted_at END,
             custom_sort_order=?,updated_at=?,started_at=CASE WHEN ?='processing' AND started_at IS NULL THEN ? ELSE started_at END
             WHERE id=?",
            params![
                sequence,
                date,
                deadline,
                deadline_label,
                target_status,
                reopen,
                reopen,
                order,
                stamp,
                target_status,
                stamp,
                task_id
            ],
        )
        .map_err(display_error)?;
    if task.status != target_status {
        add_status(
            connection,
            task_id,
            Some(&task.status),
            target_status,
            reason,
        )?;
    }
    let reason_text = reason.trim();
    add_log(
        connection,
        task_id,
        "queue",
        &if reason_text.is_empty() {
            format!("重新加入 {} 队列：{:02}", date, sequence)
        } else {
            format!("重新加入 {} 队列：{:02}（{}）", date, sequence, reason_text)
        },
    )?;
    Ok((date, sequence))
}

impl Database {
    pub fn save_task(&self, input: TaskInput) -> Result<LegalTask, String> {
        validate_task_input(&input)?;
        let contacts = normalized_contacts(&input);
        let departments = normalized_departments(&input);
        let stored_contacts = contact_storage(&contacts)?;
        let stored_departments = contact_storage(&departments)?;
        let mut guard = self
            .connection
            .lock()
            .map_err(|_| "数据库正忙，请稍后重试".to_string())?;
        let connection = guard.as_mut().ok_or("数据库尚未打开")?;
        let transaction = connection.transaction().map_err(display_error)?;
        let stamp = now();
        let effective_is_urgent = input.is_urgent && !clears_urgent_status(&input.status);
        let previous_task = input
            .id
            .map(|id| get_task_on(&transaction, id))
            .transpose()?;
        let id = if let Some(previous) = previous_task.as_ref() {
            let id = previous.id;
            if (previous.archived_at.is_some() || previous.status == "archived")
                && input.status != previous.status
            {
                return Err("已归档事项请先使用“重新开启并加入今日队列”".into());
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
            if previous.has_active_queue
                && matches!(input.status.as_str(), "pending" | "processing")
            {
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
            let date = today();
            let sequence = next_daily_sequence(&transaction, &date)?;
            let permanent = format!("{}-{:02}", date.replace('-', ""), sequence);
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
                promote_one(&transaction, id)?;
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
        transaction.commit().map_err(display_error)?;
        get_task_on(connection, id)
    }

    fn with_transaction<T>(
        &self,
        operation: impl FnOnce(&Transaction<'_>) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut guard = self
            .connection
            .lock()
            .map_err(|_| "数据库正忙，请稍后重试".to_string())?;
        let tx = guard
            .as_mut()
            .ok_or("数据库尚未打开")?
            .transaction()
            .map_err(display_error)?;
        let result = operation(&tx)?;
        tx.commit().map_err(display_error)?;
        Ok(result)
    }
}

impl Database {
    pub fn set_status(&self, id: i64, status: String) -> Result<(), String> {
        if !ALL_STATUSES.contains(&status.as_str()) {
            return Err("事项状态无效".into());
        }
        self.with_transaction(|transaction| {
            let task = get_task_on(transaction, id)?;
            if task.status == status {
                if clears_urgent_status(&status) {
                    clear_urgent_on(transaction, id, "事项已完成或进入暂缓队列")?;
                }
                return Ok(());
            }
            if matches!(status.as_str(), "pending" | "processing") && !task.has_active_queue {
                enqueue_on(
                    transaction,
                    id,
                    &status,
                    false,
                    Some((None, None)),
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
                close_active_queue(transaction, id, &format!("状态变更为 {status}"))?;
            }
            if clears_urgent_status(&status) {
                clear_urgent_on(transaction, id, "事项已完成或进入暂缓队列")?;
            }
            Ok(())
        })
    }

    pub fn move_task(&self, id: i64, direction: MoveDirection) -> Result<(), String> {
        self.with_transaction(|transaction| {
            let task = get_task_on(transaction,id)?;
            let comparison = if matches!(direction,MoveDirection::Up){"<"}else{">"};
            let order = if matches!(direction,MoveDirection::Up){"DESC"}else{"ASC"};
            let sql = format!("SELECT id,custom_sort_order FROM tasks WHERE deleted_at IS NULL AND archived_at IS NULL
                AND status NOT IN ('completed','cancelled','archived')
                AND EXISTS(SELECT 1 FROM task_queue_entries entry WHERE entry.task_id=tasks.id AND entry.closed_at IS NULL)
                AND {OVERDUE_RANK_SQL}=(SELECT CASE WHEN target.requested_deadline IS NOT NULL AND strftime('%s',target.requested_deadline) < strftime('%s','now') THEN 0 ELSE 1 END FROM tasks target WHERE target.id=?)
                AND custom_sort_order {comparison} ?
                ORDER BY custom_sort_order {order},id {order} LIMIT 1");
            let adjacent: Option<(i64,i64)> = transaction.query_row(&sql,params![id,task.custom_sort_order],|row|Ok((row.get(0)?,row.get(1)?)))
                .optional().map_err(display_error)?;
            if let Some((other_id,other_order))=adjacent {
                transaction.execute("UPDATE tasks SET custom_sort_order=? WHERE id=?",params![other_order,id]).map_err(display_error)?;
                transaction.execute("UPDATE tasks SET custom_sort_order=? WHERE id=?",params![task.custom_sort_order,other_id]).map_err(display_error)?;
            }
            Ok(())
        })
    }

    pub fn soft_delete(&self, id: i64) -> Result<(), String> {
        self.with_transaction(|tx| {
            get_task_on(tx, id)?;
            let stamp = now();
            tx.execute(
                "UPDATE tasks SET deleted_at=?,updated_at=? WHERE id=?",
                params![stamp, stamp, id],
            )
            .map_err(display_error)?;
            clear_urgent_on(tx, id, "事项移入回收站")?;
            close_active_queue(tx, id, "移入回收站")?;
            add_log(tx, id, "deleted", "事项移入回收站")
        })
    }
    pub fn archive(&self, id: i64) -> Result<(), String> {
        self.with_transaction(|tx| {
            let old = get_task_on(tx, id)?;
            let stamp = now();
            tx.execute(
                "UPDATE tasks SET status='archived',archived_at=?,updated_at=? WHERE id=?",
                params![stamp, stamp, id],
            )
            .map_err(display_error)?;
            close_active_queue(tx, id, "事项归档")?;
            add_status(tx, id, Some(&old.status), "archived", "归档事项")?;
            add_log(tx, id, "archived", "事项已归档")
        })
    }

    pub fn merge_tasks(&self, input: MergeTaskInput) -> Result<(), String> {
        if input.target_task_id == input.source_task_id {
            return Err("不能将事项合并到自身".into());
        }
        self.with_transaction(|tx| {
            let target = get_task_on(tx, input.target_task_id)?;
            let source = get_task_on(tx, input.source_task_id)?;
            if target.deleted_at.is_some() || source.deleted_at.is_some() {
                return Err("回收站中的事项不能参与合并，请先恢复".into());
            }

            close_active_queue(tx, source.id, "合并至其他事项")?;
            if input.trash_source {
                clear_urgent_on(tx, source.id, "合并后移入回收站")?;
            }
            if input.deduplicate_records {
                tx.execute(
                    "DELETE FROM task_logs
                     WHERE task_id=? AND EXISTS(
                       SELECT 1 FROM task_logs target
                       WHERE target.task_id=?
                         AND target.log_type=task_logs.log_type
                         AND target.content=task_logs.content
                         AND target.created_at=task_logs.created_at
                     )",
                    params![source.id, target.id],
                )
                .map_err(display_error)?;
                tx.execute(
                    "DELETE FROM task_work_events
                     WHERE task_id=? AND EXISTS(
                       SELECT 1 FROM task_work_events target
                       WHERE target.task_id=?
                         AND target.result_status=task_work_events.result_status
                         AND target.handled_at=task_work_events.handled_at
                         AND target.task_type_snapshot=task_work_events.task_type_snapshot
                         AND target.source=task_work_events.source
                         AND target.note=task_work_events.note
                         AND (target.voided_at IS NULL)=(task_work_events.voided_at IS NULL)
                     )",
                    params![source.id, target.id],
                )
                .map_err(display_error)?;
            }

            for table in [
                "task_logs",
                "task_work_events",
                "status_history",
                "urgent_records",
                "task_queue_entries",
            ] {
                tx.execute(
                    &format!("UPDATE {table} SET task_id=? WHERE task_id=?"),
                    params![target.id, source.id],
                )
                .map_err(display_error)?;
            }

            let stamp = now();
            tx.execute(
                "UPDATE tasks SET updated_at=? WHERE id=?",
                params![stamp, target.id],
            )
            .map_err(display_error)?;
            add_log(
                tx,
                target.id,
                "merged",
                &format!(
                    "已合并事项 {}《{}》，相关办理记录与历史记录已并入",
                    source.permanent_number, source.title
                ),
            )?;

            tx.execute(
                "UPDATE tasks
                 SET status='archived',archived_at=COALESCE(archived_at,?),
                     deleted_at=CASE WHEN ? THEN ? ELSE NULL END,updated_at=?
                 WHERE id=?",
                params![stamp, input.trash_source, stamp, stamp, source.id],
            )
            .map_err(display_error)?;
            add_log(
                tx,
                source.id,
                "merged",
                &format!(
                    "该重复事项已合并至 {}《{}》",
                    target.permanent_number, target.title
                ),
            )
        })
    }
    pub fn restore(&self, id: i64) -> Result<(), String> {
        self.with_transaction(|tx| {
            let task = get_task_on(tx, id)?;
            if task.deleted_at.is_none() {
                return Err("事项不在回收站中".into());
            }
            enqueue_on(
                tx,
                id,
                "pending",
                false,
                Some((None, None)),
                "从回收站恢复",
                true,
            )?;
            add_log(tx, id, "restored", "事项已恢复并加入今日队列")
        })
    }

    pub fn enqueue_task(&self, input: QueueInput) -> Result<(), String> {
        self.with_transaction(|tx| {
            enqueue_on(
                tx,
                input.id,
                "pending",
                input.inherit_deadline,
                None,
                &input.reason,
                false,
            )?;
            Ok(())
        })
    }

    pub fn reopen_task(&self, input: QueueInput) -> Result<(), String> {
        self.with_transaction(|tx| {
            let task = get_task_on(tx, input.id)?;
            if task.deleted_at.is_some() {
                return Err("回收站事项请先使用恢复操作".into());
            }
            if task.archived_at.is_none()
                && !matches!(task.status.as_str(), "completed" | "archived")
            {
                return Err("只有已完成或已归档事项可以重新开启".into());
            }
            enqueue_on(
                tx,
                input.id,
                "pending",
                input.inherit_deadline,
                None,
                &input.reason,
                true,
            )?;
            Ok(())
        })
    }

    pub fn process_round(&self, id: i64) -> Result<(), String> {
        self.with_transaction(|tx| {
            let task = get_task_on(tx, id)?;
            if task.deleted_at.is_some()
                || task.archived_at.is_some()
                || matches!(task.status.as_str(), "completed" | "cancelled" | "archived")
            {
                return Err("已完成或已归档事项需先重新开启".into());
            }
            let stamp = now();
            let result_status = if matches!(
                task.status.as_str(),
                "waiting_materials" | "waiting_confirmation" | "waiting_counterparty_confirmation"
            ) {
                task.status.as_str()
            } else {
                "processed"
            };
            if matches!(task.status.as_str(), "pending" | "processing") {
                tx.execute(
                    "UPDATE tasks SET status='processed',updated_at=? WHERE id=?",
                    params![stamp, id],
                )
                .map_err(display_error)?;
                add_status(tx, id, Some(&task.status), "processed", "本轮已处理")?;
            }
            clear_urgent_on(tx, id, "事项进入暂缓队列")?;
            close_active_queue(tx, id, "本轮已处理")?;
            record_work_event_on(
                tx,
                id,
                result_status,
                &stamp,
                &task.task_type,
                "quick_action",
                "",
            )?;
            add_log(tx, id, "work", "已记录本轮处理，事项进入暂缓队列")
        })
    }

    pub fn complete_round(&self, id: i64) -> Result<(), String> {
        self.with_transaction(|tx| {
            let task = get_task_on(tx, id)?;
            if task.deleted_at.is_some()
                || task.archived_at.is_some()
                || matches!(task.status.as_str(), "completed" | "archived")
            {
                return Err("该事项已经完成或归档".into());
            }
            let stamp = now();
            tx.execute(
                "UPDATE tasks SET status='completed',completed_at=?,updated_at=? WHERE id=?",
                params![stamp, stamp, id],
            )
            .map_err(display_error)?;
            clear_urgent_on(tx, id, "事项已完成")?;
            add_status(tx, id, Some(&task.status), "completed", "本轮已完成")?;
            close_active_queue(tx, id, "本轮已完成")?;
            record_work_event_on(
                tx,
                id,
                "completed",
                &stamp,
                &task.task_type,
                "quick_action",
                "",
            )?;
            add_log(tx, id, "work", "本轮已完成，事项整体结束")
        })
    }

    pub fn record_work_event(&self, input: WorkEventInput) -> Result<(), String> {
        if !is_work_event_status(&input.result_status) {
            return Err("处理结果无效".into());
        }
        validate_handled_at(&input.handled_at)?;
        self.with_transaction(|tx| {
            let task = get_task_on(tx, input.task_id)?;
            if task.deleted_at.is_some() {
                return Err("回收站事项不能新增处理活动".into());
            }
            if input.sync_status && (task.archived_at.is_some() || task.status == "archived") {
                return Err(
                    "已归档事项请先重新开启；也可以取消勾选同步状态，仅补录处理活动".into(),
                );
            }
            if input.sync_status && task.status != input.result_status {
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
        })
    }

    pub fn list_work_events(&self, task_id: i64) -> Result<Vec<TaskWorkEvent>, String> {
        self.with_conn(|connection| {
            get_task_on(connection, task_id)?;
            let mut statement = connection
                .prepare(
                    "SELECT event.id,event.task_id,event.result_status,event.handled_at,event.task_type_snapshot,
                            event.source,event.note,event.created_at,event.updated_at,
                            event.id=(SELECT first.id FROM task_work_events first
                                      WHERE first.task_id=event.task_id AND first.voided_at IS NULL
                                      ORDER BY strftime('%s',first.handled_at),first.id LIMIT 1)
                     FROM task_work_events event
                     WHERE event.task_id=? AND event.voided_at IS NULL
                     ORDER BY strftime('%s',event.handled_at) DESC,event.id DESC",
                )
                .map_err(display_error)?;
            let events = statement
                .query_map([task_id], |row| {
                    let source: String = row.get(5)?;
                    Ok(TaskWorkEvent {
                        id: row.get(0)?,
                        task_id: row.get(1)?,
                        result_status: row.get(2)?,
                        handled_at: row.get(3)?,
                        task_type_snapshot: row.get(4)?,
                        can_delete: source == "manual",
                        source,
                        note: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                        is_first_valid: row.get::<_, i64>(9)? != 0,
                    })
                })
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;
            Ok(events)
        })
    }

    pub fn update_work_event(&self, input: WorkEventUpdateInput) -> Result<(), String> {
        if !is_work_event_status(&input.result_status) {
            return Err("处理结果无效".into());
        }
        validate_handled_at(&input.handled_at)?;
        self.with_transaction(|tx| {
            let current: (i64, String, String, String) = tx
                .query_row(
                    "SELECT task_id,result_status,handled_at,source FROM task_work_events
                     WHERE id=? AND voided_at IS NULL",
                    [input.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()
                .map_err(display_error)?
                .ok_or("处理活动不存在")?;
            if current.3 != "manual" && current.1 != input.result_status {
                return Err("自动处理活动不能修改处理结果".into());
            }
            let first_id: i64 = tx
                .query_row(
                    "SELECT id FROM task_work_events WHERE task_id=? AND voided_at IS NULL
                     ORDER BY strftime('%s',handled_at),id LIMIT 1",
                    [current.0],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            if first_id == input.id
                && current.2 != input.handled_at
                && !input.confirm_historical_impact
            {
                return Err("此操作将改变该事项的统计归属期间，并可能影响历史周报、月报或季度统计。是否继续？".into());
            }
            tx.execute(
                "UPDATE task_work_events SET result_status=?,handled_at=?,note=?,updated_at=? WHERE id=?",
                params![
                    input.result_status,
                    input.handled_at,
                    input.note.trim(),
                    now(),
                    input.id
                ],
            )
            .map_err(display_error)?;
            tx.execute(
                "UPDATE tasks SET updated_at=? WHERE id=?",
                params![now(), current.0],
            )
            .map_err(display_error)?;
            add_log(tx, current.0, "audit", "调整结构化处理活动")
        })
    }

    pub fn void_work_event(&self, id: i64, confirm_historical_impact: bool) -> Result<(), String> {
        self.with_transaction(|tx| {
            let (task_id, source): (i64, String) = tx
                .query_row(
                    "SELECT task_id,source FROM task_work_events WHERE id=? AND voided_at IS NULL",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(display_error)?
                .ok_or("处理活动不存在")?;
            if source != "manual" {
                return Err("自动生成的处理活动不能删除".into());
            }
            let first_id: i64 = tx
                .query_row(
                    "SELECT id FROM task_work_events WHERE task_id=? AND voided_at IS NULL
                     ORDER BY strftime('%s',handled_at),id LIMIT 1",
                    [task_id],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            if first_id == id && !confirm_historical_impact {
                return Err("此操作将改变该事项的统计归属期间，并可能影响历史周报、月报或季度统计。是否继续？".into());
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
        })
    }
}

impl Database {
    pub fn statistics(
        &self,
        start: String,
        end: String,
        timezone_offset_minutes: i32,
    ) -> Result<StatisticsResult, String> {
        let start_time = chrono::DateTime::parse_from_rfc3339(&start)
            .map_err(|_| "统计开始时间无效".to_string())?;
        let end_time = chrono::DateTime::parse_from_rfc3339(&end)
            .map_err(|_| "统计结束时间无效".to_string())?;
        if end_time <= start_time {
            return Err("统计开始日期不能晚于结束日期".into());
        }
        let weekly = (end_time - start_time).num_days() > 62;
        let offset_seconds = timezone_offset_minutes.clamp(-14 * 60, 14 * 60) * 60;
        let offset = FixedOffset::east_opt(offset_seconds).ok_or("本地时区无效")?;
        self.with_conn(|connection| {
            let cte = "WITH ranged AS (
                SELECT event.id,event.task_id,event.result_status,event.handled_at,event.task_type_snapshot
                FROM task_work_events event
                JOIN tasks ON tasks.id=event.task_id
                WHERE event.voided_at IS NULL AND tasks.deleted_at IS NULL
                  AND strftime('%s',event.handled_at)>=strftime('%s',?1)
                  AND strftime('%s',event.handled_at)<strftime('%s',?2)
              ), ranked AS (
                SELECT *,ROW_NUMBER() OVER(
                  PARTITION BY task_id ORDER BY strftime('%s',handled_at) DESC,id DESC
                ) AS position
                FROM ranged
              )";
            let summary_sql = format!(
                "{cte}
                 SELECT count(*),
                   COALESCE(sum(result_status='processed'),0),
                   COALESCE(sum(result_status='completed'),0),
                   COALESCE(sum(result_status='waiting_materials'),0),
                   COALESCE(sum(result_status='waiting_confirmation'),0),
                   COALESCE(sum(result_status='waiting_counterparty_confirmation'),0)
                 FROM ranked WHERE position=1"
            );
            let values: (i64, i64, i64, i64, i64, i64) = connection
                .query_row(&summary_sql, params![start, end], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                })
                .map_err(display_error)?;
            let rate_mode = connection
                .query_row(
                    "SELECT value FROM settings WHERE key='statistics_rate_mode'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .map_err(display_error)?
                .filter(|value| value == "closure")
                .unwrap_or_else(|| "processing".into());
            let eligible_tasks = if rate_mode == "processing" {
                connection
                    .query_row(
                        "WITH eligible AS (
                           SELECT entry.task_id
                           FROM task_queue_entries entry
                           JOIN tasks ON tasks.id=entry.task_id
                           WHERE tasks.deleted_at IS NULL
                             AND strftime('%s',entry.enqueued_at)<strftime('%s',?2)
                             AND (entry.closed_at IS NULL OR strftime('%s',entry.closed_at)>strftime('%s',?1))
                           UNION
                           SELECT event.task_id
                           FROM task_work_events event
                           JOIN tasks ON tasks.id=event.task_id
                           WHERE event.voided_at IS NULL AND tasks.deleted_at IS NULL
                             AND strftime('%s',event.handled_at)>=strftime('%s',?1)
                             AND strftime('%s',event.handled_at)<strftime('%s',?2)
                         ) SELECT count(*) FROM eligible",
                        params![start, end],
                        |row| row.get::<_, i64>(0),
                    )
                    .map_err(display_error)?
            } else {
                values.0
            };
            let (rate_numerator, rate_denominator) = if rate_mode == "processing" {
                (values.0, eligible_tasks)
            } else {
                (values.2, values.0)
            };
            let summary = StatisticsSummary {
                handled_tasks: values.0,
                processed: values.1,
                completed: values.2,
                waiting_materials: values.3,
                waiting_confirmation: values.4,
                waiting_counterparty_confirmation: values.5,
                rate_mode,
                rate_numerator,
                rate_denominator,
                completion_rate: if rate_denominator == 0 {
                    0.0
                } else {
                    rate_numerator as f64 / rate_denominator as f64
                },
            };
            let type_sql = format!(
                "{cte}
                 SELECT task_type_snapshot,count(*),
                   COALESCE(sum(result_status='completed'),0),
                   COALESCE(sum(result_status<>'completed'),0)
                 FROM ranked WHERE position=1
                 GROUP BY task_type_snapshot ORDER BY count(*) DESC,task_type_snapshot"
            );
            let mut type_statement = connection.prepare(&type_sql).map_err(display_error)?;
            let by_task_type = type_statement
                .query_map(params![start, end], |row| {
                    Ok(TaskTypeStatistics {
                        task_type: row.get(0)?,
                        handled_tasks: row.get(1)?,
                        completed: row.get(2)?,
                        pending_follow_up: row.get(3)?,
                    })
                })
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;

            let mut trend_statement = connection
                .prepare(
                    "SELECT event.task_id,event.handled_at
                     FROM task_work_events event
                     JOIN tasks ON tasks.id=event.task_id
                     WHERE event.voided_at IS NULL AND tasks.deleted_at IS NULL
                       AND strftime('%s',event.handled_at)>=strftime('%s',?)
                       AND strftime('%s',event.handled_at)<strftime('%s',?)
                     ORDER BY strftime('%s',event.handled_at),event.id",
                )
                .map_err(display_error)?;
            let raw_trend = trend_statement
                .query_map(params![start, end], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;
            let mut buckets: BTreeMap<String, HashSet<i64>> = BTreeMap::new();
            for (task_id, handled_at) in raw_trend {
                let parsed = chrono::DateTime::parse_from_rfc3339(&handled_at)
                    .map_err(|_| "处理活动中存在无效时间".to_string())?
                    .with_timezone(&offset);
                let mut date = parsed.date_naive();
                if weekly {
                    date -= chrono::Duration::days(
                        date.weekday().num_days_from_monday() as i64,
                    );
                }
                buckets
                    .entry(date.format("%Y-%m-%d").to_string())
                    .or_default()
                    .insert(task_id);
            }
            let trend = buckets
                .into_iter()
                .map(|(period_start, ids)| TrendPoint {
                    period_start,
                    handled_tasks: ids.len() as i64,
                })
                .collect();
            Ok(StatisticsResult {
                range: StatisticsRange {
                    start: start.clone(),
                    end: end.clone(),
                },
                summary,
                by_task_type,
                trend,
                trend_granularity: if weekly { "week" } else { "day" }.into(),
            })
        })
    }

    pub fn statistics_details(
        &self,
        start: String,
        end: String,
        task_type: String,
    ) -> Result<Vec<StatisticsDetail>, String> {
        let start_time = chrono::DateTime::parse_from_rfc3339(&start)
            .map_err(|_| "统计开始时间无效".to_string())?;
        let end_time = chrono::DateTime::parse_from_rfc3339(&end)
            .map_err(|_| "统计结束时间无效".to_string())?;
        if end_time <= start_time {
            return Err("统计结束时间必须晚于开始时间".into());
        }
        self.with_conn(|connection| {
            let mut statement = connection
                .prepare(
                    "WITH ranged AS (
                       SELECT event.id,event.task_id,event.result_status,event.handled_at,event.task_type_snapshot
                       FROM task_work_events event
                       JOIN tasks ON tasks.id=event.task_id
                       WHERE event.voided_at IS NULL AND tasks.deleted_at IS NULL
                         AND strftime('%s',event.handled_at)>=strftime('%s',?1)
                         AND strftime('%s',event.handled_at)<strftime('%s',?2)
                     ), annotated AS (
                       SELECT *,
                         ROW_NUMBER() OVER(PARTITION BY task_id ORDER BY strftime('%s',handled_at) DESC,id DESC) AS position,
                         FIRST_VALUE(handled_at) OVER(PARTITION BY task_id ORDER BY strftime('%s',handled_at),id) AS first_handled_at,
                         FIRST_VALUE(handled_at) OVER(PARTITION BY task_id ORDER BY strftime('%s',handled_at) DESC,id DESC) AS last_handled_at,
                         count(*) OVER(PARTITION BY task_id) AS handling_count
                       FROM ranged
                     )
                     SELECT tasks.id,tasks.permanent_number,tasks.title,tasks.department,tasks.contact,
                            annotated.result_status,annotated.first_handled_at,annotated.last_handled_at,annotated.handling_count
                     FROM annotated JOIN tasks ON tasks.id=annotated.task_id
                     WHERE annotated.position=1 AND annotated.task_type_snapshot=?3
                     ORDER BY strftime('%s',annotated.last_handled_at) DESC,tasks.id DESC",
                )
                .map_err(display_error)?;
            let details = statement
                .query_map(params![start, end, task_type], |row| {
                    let departments = parse_contacts(&row.get::<_, String>(3)?).join("、");
                    let contacts = parse_contacts(&row.get::<_, String>(4)?).join("、");
                    Ok(StatisticsDetail {
                        task_id: row.get(0)?,
                        permanent_number: row.get(1)?,
                        title: row.get(2)?,
                        department: departments,
                        contact: contacts,
                        result_status: row.get(5)?,
                        first_handled_at: row.get(6)?,
                        last_handled_at: row.get(7)?,
                        handling_count: row.get(8)?,
                    })
                })
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;
            Ok(details)
        })
    }
}
fn add_log(connection: &Connection, id: i64, kind: &str, content: &str) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO task_logs(task_id,log_type,content,created_at) VALUES(?,?,?,?)",
            params![id, kind, content, now()],
        )
        .map_err(display_error)?;
    Ok(())
}
fn promote_one(connection: &Connection, id: i64) -> Result<(), String> {
    let order: Option<i64> = connection
        .query_row(
            "SELECT custom_sort_order FROM tasks WHERE id=? AND deleted_at IS NULL AND archived_at IS NULL
             AND status NOT IN ('completed','cancelled','archived')
             AND EXISTS(SELECT 1 FROM task_queue_entries entry WHERE entry.task_id=tasks.id AND entry.closed_at IS NULL)",
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(display_error)?;
    let Some(order) = order else {
        return Ok(());
    };
    let previous: Option<(i64, i64)> = connection
        .query_row(
            "SELECT id,custom_sort_order FROM tasks WHERE deleted_at IS NULL AND archived_at IS NULL
             AND status NOT IN ('completed','cancelled','archived') AND id<>?
             AND EXISTS(SELECT 1 FROM task_queue_entries entry WHERE entry.task_id=tasks.id AND entry.closed_at IS NULL)
             AND CASE WHEN requested_deadline IS NOT NULL AND strftime('%s',requested_deadline) < strftime('%s','now') THEN 0 ELSE 1 END
                 =(SELECT CASE WHEN target.requested_deadline IS NOT NULL AND strftime('%s',target.requested_deadline) < strftime('%s','now') THEN 0 ELSE 1 END FROM tasks target WHERE target.id=?)
             AND custom_sort_order<?
             ORDER BY custom_sort_order DESC,id DESC LIMIT 1",
            params![id, id, order],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(display_error)?;
    if let Some((other_id, other_order)) = previous {
        connection
            .execute(
                "UPDATE tasks SET custom_sort_order=? WHERE id=?",
                params![other_order, id],
            )
            .map_err(display_error)?;
        connection
            .execute(
                "UPDATE tasks SET custom_sort_order=? WHERE id=?",
                params![order, other_id],
            )
            .map_err(display_error)?;
    }
    Ok(())
}
fn add_status(
    connection: &Connection,
    id: i64,
    old: Option<&str>,
    new: &str,
    reason: &str,
) -> Result<(), String> {
    connection.execute("INSERT INTO status_history(task_id,old_status,new_status,reason,created_at) VALUES(?,?,?,?,?)",
        params![id,old,new,reason,now()]).map_err(display_error)?;
    Ok(())
}
fn record_urgent(connection: &Connection, id: i64, input: &TaskInput) -> Result<(), String> {
    connection.execute("INSERT INTO urgent_records(task_id,requester,reason,requested_deadline,requested_at,confirmation_status,confirmed_at)
            VALUES(?,?,?,?,?,'confirmed',?)",params![id,input.urgent_requester.trim(),input.urgent_reason.trim(),input.requested_deadline,now(),now()]).map_err(display_error)?;
    add_log(
        connection,
        id,
        "urgent",
        &format!("标记加急：{}", input.urgent_requester.trim()),
    )
}

fn cancel_urgent_records(connection: &Connection, id: i64, reason: &str) -> Result<(), String> {
    connection
        .execute(
            "UPDATE urgent_records SET cancelled_at=? WHERE task_id=? AND cancelled_at IS NULL",
            params![now(), id],
        )
        .map_err(display_error)?;
    let content = if reason.is_empty() {
        "取消加急".to_string()
    } else {
        format!("取消加急：{reason}")
    };
    add_log(connection, id, "urgent", &content)
}

fn clear_urgent_on(connection: &Connection, id: i64, reason: &str) -> Result<(), String> {
    let changed = connection
        .execute(
            "UPDATE tasks SET is_urgent=0 WHERE id=? AND is_urgent=1",
            [id],
        )
        .map_err(display_error)?;
    if changed > 0 {
        cancel_urgent_records(connection, id, reason)?;
    }
    Ok(())
}
fn ensure_master(connection: &Connection, kind: &str, name: &str) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO master_values(kind,name,sort_order,is_active) VALUES(?,?,999,1)
        ON CONFLICT(kind,name) DO UPDATE SET is_active=1",
            params![kind, name.trim()],
        )
        .map_err(display_error)?;
    Ok(())
}
fn bump_master_use(connection: &Connection, kind: &str, name: &str) -> Result<(), String> {
    ensure_master(connection, kind, name)?;
    connection
        .execute(
            "UPDATE master_values SET usage_count=usage_count+1 WHERE kind=? AND name=?",
            params![kind, name.trim()],
        )
        .map_err(display_error)?;
    Ok(())
}

impl Database {
    pub fn get_logs(&self, task_id: i64) -> Result<Vec<TaskLog>, String> {
        self.with_conn(|connection|{
            let mut statement=connection.prepare("SELECT id,task_id,log_type,content,created_at FROM task_logs WHERE task_id=? ORDER BY created_at DESC").map_err(display_error)?;
            let rows=statement.query_map([task_id],|row|Ok(TaskLog{id:row.get(0)?,task_id:row.get(1)?,log_type:row.get(2)?,content:row.get(3)?,created_at:row.get(4)?}))
                .map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error);
            rows
        })
    }
    pub fn add_log(&self, task_id: i64, content: String) -> Result<(), String> {
        let content = content.trim();
        if content.is_empty() || content.chars().count() > 2000 {
            return Err("处理记录应为 1 至 2000 个字符".into());
        }
        self.with_conn(|connection| add_log(connection, task_id, "note", content))
    }
    pub fn update_log(&self, log_id: i64, content: String) -> Result<(), String> {
        let content = content.trim();
        if content.is_empty() || content.chars().count() > 2000 {
            return Err("处理记录应为 1 至 2000 个字符".into());
        }
        self.with_conn(|connection| {
            let changed = connection
                .execute(
                    "UPDATE task_logs SET content=? WHERE id=? AND log_type='note'",
                    params![content, log_id],
                )
                .map_err(display_error)?;
            if changed == 0 {
                return Err("系统自动记录不能编辑".into());
            }
            Ok(())
        })
    }
    pub fn delete_log(&self, log_id: i64) -> Result<(), String> {
        self.with_conn(|connection| {
            let changed = connection
                .execute(
                    "DELETE FROM task_logs WHERE id=? AND log_type='note'",
                    [log_id],
                )
                .map_err(display_error)?;
            if changed == 0 {
                return Err("系统自动记录不能删除".into());
            }
            Ok(())
        })
    }
    pub fn masters(&self) -> Result<MasterData, String> {
        self.with_conn(|connection|{
            let mut statement=connection.prepare("SELECT kind,name FROM master_values WHERE is_active=1
                ORDER BY kind,CASE WHEN manual_order IS NULL THEN 1 ELSE 0 END,manual_order,usage_count DESC,sort_order,name COLLATE NOCASE").map_err(display_error)?;
            let rows=statement.query_map([],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?))).map_err(display_error)?
                .collect::<Result<Vec<_>,_>>().map_err(display_error)?;
            Ok(MasterData{departments:rows.iter().filter(|x|x.0=="department").map(|x|x.1.clone()).collect(),
                task_types:rows.iter().filter(|x|x.0=="task_type").map(|x|x.1.clone()).collect(),
                contacts:rows.iter().filter(|x|x.0=="contact").map(|x|x.1.clone()).collect()})
        })
    }
    pub fn add_master(&self, kind: String, name: String) -> Result<MasterData, String> {
        if kind != "department" && kind != "task_type" && kind != "contact" {
            return Err("事项状态无效".into());
        }
        if name.trim().is_empty() || name.chars().count() > 100 {
            return Err("名称应为 1 至 100 个字符".into());
        }
        self.with_conn(|connection| ensure_master(connection, &kind, &name))?;
        self.masters()
    }
    pub fn delete_master(&self, kind: String, name: String) -> Result<MasterData, String> {
        if kind != "department" && kind != "task_type" && kind != "contact" {
            return Err("选项类型无效".into());
        }
        if name.trim().is_empty() || name.chars().count() > 100 {
            return Err("选项名称无效".into());
        }
        self.with_conn(|connection| {
            connection
                .execute(
                    "UPDATE master_values SET is_active=0 WHERE kind=? AND name=?",
                    params![kind, name.trim()],
                )
                .map_err(display_error)?;
            Ok(())
        })?;
        self.masters()
    }
    pub fn move_master(
        &self,
        kind: String,
        name: String,
        direction: MoveDirection,
    ) -> Result<MasterData, String> {
        if kind != "department" && kind != "task_type" {
            return Err("仅部门 / 团队和事项类型支持手动排序".into());
        }
        if name.trim().is_empty() || name.chars().count() > 100 {
            return Err("选项名称无效".into());
        }
        self.with_transaction(|transaction| {
            let mut statement = transaction
                .prepare("SELECT name FROM master_values WHERE kind=? AND is_active=1
                    ORDER BY CASE WHEN manual_order IS NULL THEN 1 ELSE 0 END,manual_order,usage_count DESC,sort_order,name COLLATE NOCASE")
                .map_err(display_error)?;
            let mut names = statement
                .query_map([&kind], |row| row.get::<_, String>(0))
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;
            drop(statement);
            let Some(index) = names.iter().position(|value| value == name.trim()) else {
                return Err("选项不存在或已删除".into());
            };
            let target = match direction {
                MoveDirection::Up if index > 0 => Some(index - 1),
                MoveDirection::Down if index + 1 < names.len() => Some(index + 1),
                _ => None,
            };
            if let Some(target) = target {
                names.swap(index, target);
                for (order, value) in names.iter().enumerate() {
                    transaction
                        .execute(
                            "UPDATE master_values SET manual_order=? WHERE kind=? AND name=?",
                            params![order as i64 + 1, &kind, value],
                        )
                        .map_err(display_error)?;
                }
            }
            Ok(())
        })?;
        self.masters()
    }
    pub fn queue_ahead(&self, id: i64) -> Result<i64, String> {
        self.with_conn(|connection| queue_ahead_on(connection, id))
    }
    pub fn ticket_snapshot(&self, id: i64) -> Result<TicketSnapshot, String> {
        self.with_conn(|connection| {
            let task = get_task_on(connection, id)?;
            let queue_ahead = queue_ahead_on(connection, id)?;
            Ok(TicketSnapshot { task, queue_ahead })
        })
    }
    pub fn settings(&self) -> Result<HashMap<String, String>, String> {
        self.with_conn(|connection| {
            let mut statement = connection
                .prepare("SELECT key,value FROM settings")
                .map_err(display_error)?;
            let rows = statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;
            Ok(rows.into_iter().collect())
        })
    }
    pub fn set_setting(&self, key: String, value: String) -> Result<(), String> {
        let valid = match key.as_str() {
            "show_deferred_in_queue" => value == "true" || value == "false",
            "week_start_day" => value == "monday" || value == "sunday",
            "statistics_rate_mode" => value == "closure" || value == "processing",
            _ => return Err("不支持的设置项".into()),
        };
        if !valid {
            return Err("设置值无效".into());
        }
        self.with_conn(|connection| {
            connection
                .execute(
                    "INSERT INTO settings(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                    params![key, value],
                )
                .map_err(display_error)?;
            Ok(())
        })
    }
    pub fn bootstrap(&self) -> Result<BootstrapData, String> {
        Ok(BootstrapData {
            queue: self.list_tasks(TaskView::Queue)?,
            archive: self.list_tasks(TaskView::Archive)?,
            trash: self.list_tasks(TaskView::Trash)?,
            masters: self.masters()?,
            settings: self.settings()?,
            backups: self.list_backups()?,
        })
    }
}

impl Database {
    fn backup_name(kind: &str) -> String {
        format!(
            "InLine-backup-{}-{kind}.db",
            Local::now().format("%Y%m%d-%H%M%S")
        )
    }

    fn normalize_backup_names(root: &Path) -> Result<(), String> {
        let entries = fs::read_dir(root)
            .map_err(display_error)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|value| value == "db"))
            .collect::<Vec<_>>();
        for source in entries {
            let old_name = source.file_name().unwrap_or_default().to_string_lossy();
            if old_name.starts_with("InLine-backup-") {
                continue;
            }
            let lower = old_name.to_ascii_lowercase();
            let kind = if lower.starts_with("auto-") {
                "auto"
            } else if lower.starts_with("manual-") {
                "manual"
            } else if lower.starts_with("pre-restore-") {
                "before-restore"
            } else if lower.starts_with("pre-tauri-migration-") {
                "before-migration"
            } else {
                "legacy"
            };
            let modified = fs::metadata(&source)
                .and_then(|metadata| metadata.modified())
                .ok()
                .map(chrono::DateTime::<Local>::from)
                .unwrap_or_else(Local::now);
            let base = format!("InLine-backup-{}-{kind}", modified.format("%Y%m%d-%H%M%S"));
            let mut target = root.join(format!("{base}.db"));
            let mut suffix = 1;
            while target.exists() {
                target = root.join(format!("{base}-{suffix}.db"));
                suffix += 1;
            }
            fs::rename(&source, target).map_err(display_error)?;
        }
        Ok(())
    }

    fn prune_backups(root: &Path, keep: usize) -> Result<(), String> {
        let mut files = fs::read_dir(root)
            .map_err(display_error)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().ends_with("-auto.db"))
            })
            .collect::<Vec<_>>();
        files.sort_by_key(|path| fs::metadata(path).and_then(|value| value.modified()).ok());
        let remove = files.len().saturating_sub(keep);
        for path in files.into_iter().take(remove) {
            fs::remove_file(path).map_err(display_error)?;
        }
        Ok(())
    }

    fn backup_connection(connection: &Connection, path: &Path) -> Result<(), String> {
        if path.exists() {
            fs::remove_file(path).map_err(display_error)?;
        }
        let escaped = path.to_string_lossy().replace('\'', "''");
        connection
            .execute_batch(&format!("VACUUM INTO '{}'", escaped))
            .map_err(display_error)
    }
    pub fn create_backup(&self, label: &str) -> Result<BackupInfo, String> {
        let safe = if label == "manual" { "manual" } else { "auto" };
        let path = self.backup_dir.join(Self::backup_name(safe));
        self.with_conn(|connection| Self::backup_connection(connection, &path))?;
        backup_info(&path)
    }
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>, String> {
        let mut values = fs::read_dir(&self.backup_dir)
            .map_err(display_error)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|v| v == "db"))
            .filter_map(|path| backup_info(&path).ok())
            .collect::<Vec<_>>();
        values.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        Ok(values)
    }
    pub fn delete_backup(&self, raw_path: String) -> Result<(), String> {
        let selected = fs::canonicalize(&raw_path).map_err(|_| "找不到所选备份".to_string())?;
        let backup_root = fs::canonicalize(&self.backup_dir).map_err(display_error)?;
        if !selected.starts_with(&backup_root)
            || selected.extension().is_none_or(|value| value != "db")
        {
            return Err("只能删除 In Line 备份目录中的数据库文件".into());
        }
        fs::remove_file(selected).map_err(display_error)
    }

    pub fn restore_backup(&self, raw_path: String) -> Result<(), String> {
        let selected = fs::canonicalize(&raw_path).map_err(|_| "找不到所选备份".to_string())?;
        let backup_root = fs::canonicalize(&self.backup_dir).map_err(display_error)?;
        if !selected.starts_with(&backup_root) || selected.extension().is_none_or(|v| v != "db") {
            return Err("只能恢复 In Line 备份目录中的数据库文件".into());
        }
        let check = Connection::open(&selected).map_err(|_| "备份文件无法打开".to_string())?;
        let integrity: String = check
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(display_error)?;
        if integrity != "ok" {
            return Err("备份文件校验失败，当前数据未改变".into());
        }
        drop(check);
        let emergency = self.backup_dir.join(Self::backup_name("before-restore"));
        {
            let mut guard = self
                .connection
                .lock()
                .map_err(|_| "数据库正忙，请稍后重试".to_string())?;
            if let Some(connection) = guard.as_ref() {
                Self::backup_connection(connection, &emergency)?;
            }
            guard.take();
        }
        let staged = self.path.with_extension("restore.tmp");
        let old = self.path.with_extension("restore.old");
        let restore_result = (|| -> Result<(), String> {
            if staged.exists() {
                fs::remove_file(&staged).map_err(display_error)?;
            }
            fs::copy(&selected, &staged).map_err(display_error)?;
            if old.exists() {
                fs::remove_file(&old).map_err(display_error)?;
            }
            if self.path.exists() {
                fs::rename(&self.path, &old).map_err(display_error)?;
            }
            fs::rename(&staged, &self.path).map_err(display_error)?;
            let mut connection = Self::connect(&self.path)?;
            Self::migrate(&mut connection)?;
            *self
                .connection
                .lock()
                .map_err(|_| "数据库正忙".to_string())? = Some(connection);
            if old.exists() {
                let _ = fs::remove_file(&old);
            }
            Ok(())
        })();
        if restore_result.is_err() {
            let _ = fs::remove_file(&staged);
            if old.exists() {
                let _ = fs::remove_file(&self.path);
                let _ = fs::rename(&old, &self.path);
            }
            if let Ok(connection) = Self::connect(&self.path) {
                *self.connection.lock().unwrap() = Some(connection);
            }
        }
        restore_result
    }
}
fn backup_info(path: &Path) -> Result<BackupInfo, String> {
    let metadata = fs::metadata(path).map_err(display_error)?;
    let modified = metadata.modified().map_err(display_error)?;
    let modified_at = chrono::DateTime::<Local>::from(modified).to_rfc3339();
    Ok(BackupInfo {
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        path: path.to_string_lossy().to_string(),
        size: metadata.len(),
        modified_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(title: &str) -> TaskInput {
        TaskInput {
            id: None,
            department: "产品组".into(),
            departments: vec!["产品组".into()],
            contact: "小林".into(),
            contacts: vec!["小林".into()],
            task_type: "任务处理".into(),
            title: title.into(),
            details: "测试事项".into(),
            status: "pending".into(),
            priority: "normal".into(),
            workload: "standard".into(),
            is_urgent: false,
            urgent_requester: "".into(),
            urgent_reason: "".into(),
            requested_deadline: None,
            requested_deadline_label: None,
            internal_notes: "".into(),
        }
    }
    fn urgent_sample(title: &str) -> TaskInput {
        let mut input = sample(title);
        input.is_urgent = true;
        input.urgent_requester = "测试人".into();
        input.urgent_reason = "需要优先处理".into();
        input
    }
    #[test]
    fn sequence_and_manual_order_are_persistent() {
        let root = std::env::temp_dir().join(format!(
            "inline-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("inline.db");
        let db = Database::open_at(path).unwrap();
        let first = db.save_task(sample("第一项")).unwrap();
        let mut empty_details = sample("可选详情");
        empty_details.details.clear();
        let second = db.save_task(empty_details).unwrap();
        assert_eq!(second.daily_sequence, first.daily_sequence + 1);
        assert_eq!(db.queue_ahead(first.id).unwrap(), 0);
        assert_eq!(db.queue_ahead(second.id).unwrap(), 1);
        assert!(db.masters().unwrap().contacts.contains(&"小林".to_string()));
        assert_eq!(db.settings().unwrap().get("show_deferred_in_queue"), None);
        db.set_setting("show_deferred_in_queue".into(), "true".into())
            .unwrap();
        assert_eq!(
            db.settings()
                .unwrap()
                .get("show_deferred_in_queue")
                .map(String::as_str),
            Some("true")
        );
        db.set_setting("week_start_day".into(), "sunday".into())
            .unwrap();
        assert_eq!(
            db.settings()
                .unwrap()
                .get("week_start_day")
                .map(String::as_str),
            Some("sunday")
        );
        assert!(db
            .set_setting("week_start_day".into(), "friday".into())
            .is_err());
        db.set_setting("statistics_rate_mode".into(), "processing".into())
            .unwrap();
        assert_eq!(
            db.settings()
                .unwrap()
                .get("statistics_rate_mode")
                .map(String::as_str),
            Some("processing")
        );
        assert!(db
            .set_setting("statistics_rate_mode".into(), "unknown".into())
            .is_err());
        assert!(db.set_setting("unknown".into(), "true".into()).is_err());
        db.move_task(second.id, MoveDirection::Up).unwrap();
        assert_eq!(db.list_tasks(TaskView::Queue).unwrap()[0].id, second.id);
        let third = db.save_task(sample("第三项")).unwrap();
        let mut urgent = sample("加急项");
        urgent.id = Some(third.id);
        urgent.is_urgent = true;
        urgent.urgent_requester = "测试人".into();
        urgent.urgent_reason = "需要优先处理".into();
        db.save_task(urgent).unwrap();
        let queue = db.list_tasks(TaskView::Queue).unwrap();
        assert_eq!(queue[1].id, third.id, "首次加急应自动前移一位");
        let snapshot = db.ticket_snapshot(third.id).unwrap();
        assert_eq!(snapshot.queue_ahead, 1);
        db.add_log(third.id, "可编辑记录".into()).unwrap();
        let manual = db
            .get_logs(third.id)
            .unwrap()
            .into_iter()
            .find(|log| log.log_type == "note")
            .unwrap();
        db.update_log(manual.id, "已更新记录".into()).unwrap();
        assert_eq!(
            db.get_logs(third.id)
                .unwrap()
                .into_iter()
                .find(|log| log.id == manual.id)
                .unwrap()
                .content,
            "已更新记录"
        );
        db.delete_log(manual.id).unwrap();
        assert!(!db
            .get_logs(third.id)
            .unwrap()
            .iter()
            .any(|log| log.id == manual.id));
        db.delete_master("contact".into(), "小林".into()).unwrap();
        assert!(!db.masters().unwrap().contacts.contains(&"小林".to_string()));
        let backup = db.create_backup("manual").unwrap();
        assert!(backup.name.starts_with("InLine-backup-"));
        assert!(backup.name.ends_with("-manual.db"));
        db.delete_backup(backup.path).unwrap();
        drop(db);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn merge_tasks_preserves_history_and_deduplicates_events() {
        let root = std::env::temp_dir().join(format!(
            "inline-merge-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        let target = db.save_task(sample("主事项")).unwrap();
        let source = db.save_task(sample("重复事项")).unwrap();
        let handled_at = now();
        for task_id in [target.id, source.id] {
            db.record_work_event(WorkEventInput {
                task_id,
                result_status: "processed".into(),
                handled_at: handled_at.clone(),
                note: "相同办理记录".into(),
                sync_status: false,
            })
            .unwrap();
        }
        db.record_work_event(WorkEventInput {
            task_id: source.id,
            result_status: "completed".into(),
            handled_at: (Utc::now() + chrono::Duration::minutes(1)).to_rfc3339(),
            note: "来源事项独有记录".into(),
            sync_status: false,
        })
        .unwrap();
        db.add_log(source.id, "需要保留的普通备注".into()).unwrap();

        db.merge_tasks(MergeTaskInput {
            target_task_id: target.id,
            source_task_id: source.id,
            deduplicate_records: true,
            trash_source: true,
        })
        .unwrap();

        let merged_events = db.list_work_events(target.id).unwrap();
        assert_eq!(merged_events.len(), 2);
        assert!(merged_events
            .iter()
            .any(|event| event.note == "来源事项独有记录"));
        assert!(db
            .get_logs(target.id)
            .unwrap()
            .iter()
            .any(|log| log.content == "需要保留的普通备注"));
        assert_eq!(db.list_work_events(source.id).unwrap().len(), 0);
        assert!(!db.get_task(source.id).unwrap().has_active_queue);
        assert!(db
            .list_tasks(TaskView::Trash)
            .unwrap()
            .iter()
            .any(|task| task.id == source.id));
        drop(db);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn overdue_tasks_are_prioritized_consistently() {
        let root = std::env::temp_dir().join(format!(
            "inline-overdue-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();

        let regular = db.save_task(sample("普通事项")).unwrap();
        let mut overdue_input = sample("逾期暂缓事项");
        overdue_input.status = "waiting_materials".into();
        overdue_input.requested_deadline =
            Some((Utc::now() - chrono::Duration::hours(1)).to_rfc3339());
        let overdue = db.save_task(overdue_input).unwrap();

        let queue = db.list_tasks(TaskView::Queue).unwrap();
        assert_eq!(queue[0].id, overdue.id);
        assert!(!overdue.has_active_queue);
        assert_eq!(db.queue_ahead(overdue.id).unwrap(), 0);
        assert_eq!(db.ticket_snapshot(regular.id).unwrap().queue_ahead, 0);
        db.move_task(regular.id, MoveDirection::Up).unwrap();
        assert_eq!(db.list_tasks(TaskView::Queue).unwrap()[0].id, overdue.id);

        drop(db);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn counterparty_confirmation_is_a_valid_status() {
        let mut input = sample("等待对方确认");
        input.status = "waiting_counterparty_confirmation".into();
        validate_task_input(&input).unwrap();
    }
    #[test]
    fn contacts_and_master_sorting_are_persistent() {
        let root = std::env::temp_dir().join(format!(
            "inline-master-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();

        let mut low = sample("低频部门事项");
        low.department = "低频组".into();
        low.departments = vec!["低频组".into()];
        low.task_type = "低频类型".into();
        low.contact = "小林、小周".into();
        low.contacts = vec!["小林".into(), "小周".into()];
        let saved = db.save_task(low).unwrap();
        assert_eq!(saved.contacts, vec!["小林", "小周"]);
        assert_eq!(saved.contact, "小林、小周");

        for title in ["高频一", "高频二"] {
            let mut high = sample(title);
            high.department = "高频组".into();
            high.departments = vec!["高频组".into()];
            high.task_type = "高频类型".into();
            db.save_task(high).unwrap();
        }
        let masters = db.masters().unwrap();
        assert_eq!(&masters.departments[..2], &["高频组", "低频组"]);
        assert_eq!(&masters.task_types[..2], &["高频类型", "低频类型"]);
        assert_eq!(masters.contacts[0], "小林");
        assert!(masters.contacts.contains(&"小周".to_string()));

        let moved = db
            .move_master("department".into(), "低频组".into(), MoveDirection::Up)
            .unwrap();
        assert_eq!(&moved.departments[..2], &["低频组", "高频组"]);
        drop(db);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn processed_tasks_can_requeue_without_losing_identity_or_rounds() {
        let root = std::env::temp_dir().join(format!(
            "inline-processed-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();

        let mut input = sample("多部门处理事项");
        input.departments = vec!["产品组".into(), "法务组".into()];
        input.department = "产品组、法务组".into();
        input.requested_deadline = Some((Utc::now() + chrono::Duration::days(1)).to_rfc3339());
        let created = db.save_task(input).unwrap();
        let original_number = created.permanent_number.clone();
        let original_sequence = created.daily_sequence;
        assert_eq!(created.departments, vec!["产品组", "法务组"]);
        assert!(created.has_active_queue);

        db.process_round(created.id).unwrap();
        let processed = db.get_task(created.id).unwrap();
        assert_eq!(processed.status, "processed");
        assert_eq!(processed.processing_rounds, 1);
        assert!(!processed.has_active_queue);

        let start = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let end = (Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        let statistics = db.statistics(start.clone(), end.clone(), 480).unwrap();
        assert_eq!(statistics.summary.handled_tasks, 1);
        assert_eq!(statistics.summary.processed, 1);

        db.enqueue_task(QueueInput {
            id: created.id,
            inherit_deadline: false,
            reason: "继续跟进".into(),
        })
        .unwrap();
        let requeued = db.get_task(created.id).unwrap();
        assert_eq!(requeued.status, "pending");
        assert!(requeued.has_active_queue);
        assert_eq!(requeued.permanent_number, original_number);
        assert!(requeued.daily_sequence > original_sequence);
        assert_eq!(requeued.processing_rounds, 1);
        assert_eq!(requeued.requested_deadline, None);

        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn handled_deferred_and_deleted_tasks_cancel_urgent_state() {
        let root = std::env::temp_dir().join(format!(
            "inline-urgent-lifecycle-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();

        let processed = db.save_task(urgent_sample("本轮已处理")).unwrap();
        db.process_round(processed.id).unwrap();
        assert!(!db.get_task(processed.id).unwrap().is_urgent);

        let completed = db.save_task(urgent_sample("本轮已完成")).unwrap();
        db.complete_round(completed.id).unwrap();
        assert!(!db.get_task(completed.id).unwrap().is_urgent);

        let deferred = db.save_task(urgent_sample("状态改为暂缓")).unwrap();
        db.set_status(deferred.id, "waiting_confirmation".into())
            .unwrap();
        assert!(!db.get_task(deferred.id).unwrap().is_urgent);

        let deleted = db.save_task(urgent_sample("移入回收站")).unwrap();
        db.soft_delete(deleted.id).unwrap();
        assert!(!db.get_task(deleted.id).unwrap().is_urgent);

        let edited = db.save_task(urgent_sample("编辑时完成")).unwrap();
        let mut edited_input = urgent_sample("编辑时完成");
        edited_input.id = Some(edited.id);
        edited_input.status = "completed".into();
        assert!(!db.save_task(edited_input).unwrap().is_urgent);

        let synced = db.save_task(urgent_sample("同步处理状态")).unwrap();
        db.record_work_event(WorkEventInput {
            task_id: synced.id,
            result_status: "waiting_materials".into(),
            handled_at: now(),
            note: "等待补充材料".into(),
            sync_status: true,
        })
        .unwrap();
        assert!(!db.get_task(synced.id).unwrap().is_urgent);

        let mut initially_deferred = sample("初始暂缓事项");
        initially_deferred.status = "paused".into();
        initially_deferred.is_urgent = true;
        assert!(!db.save_task(initially_deferred).unwrap().is_urgent);

        let active_urgent_records: i64 = db
            .with_conn(|connection| {
                connection
                    .query_row(
                        "SELECT count(*) FROM urgent_records WHERE cancelled_at IS NULL",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(display_error)
            })
            .unwrap();
        assert_eq!(active_urgent_records, 0);

        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn statistics_use_latest_event_and_guard_historical_attribution() {
        let root = std::env::temp_dir().join(format!(
            "inline-statistics-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();

        let mut input = sample("等待材料事项");
        input.status = "waiting_materials".into();
        let created = db.save_task(input).unwrap();
        db.process_round(created.id).unwrap();
        let waiting = db.get_task(created.id).unwrap();
        assert_eq!(waiting.status, "waiting_materials");
        assert!(!waiting.has_active_queue);

        let completed_at = (Utc::now() + chrono::Duration::seconds(1)).to_rfc3339();
        db.record_work_event(WorkEventInput {
            task_id: created.id,
            result_status: "completed".into(),
            handled_at: completed_at,
            note: "补录完成结果".into(),
            sync_status: false,
        })
        .unwrap();

        let start = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let end = (Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        let statistics = db.statistics(start.clone(), end.clone(), 480).unwrap();
        assert_eq!(statistics.summary.handled_tasks, 1);
        assert_eq!(statistics.summary.completed, 1);
        assert_eq!(statistics.summary.waiting_materials, 0);
        assert_eq!(statistics.summary.rate_mode, "processing");
        assert_eq!(statistics.summary.rate_numerator, 1);
        assert_eq!(statistics.summary.rate_denominator, 1);
        assert_eq!(statistics.summary.completion_rate, 1.0);
        db.save_task(sample("本周期尚未处理事项")).unwrap();
        let processing_statistics = db.statistics(start.clone(), end.clone(), 480).unwrap();
        assert_eq!(processing_statistics.summary.rate_mode, "processing");
        assert_eq!(processing_statistics.summary.rate_numerator, 1);
        assert_eq!(processing_statistics.summary.rate_denominator, 2);
        assert_eq!(processing_statistics.summary.completion_rate, 0.5);
        db.set_setting("statistics_rate_mode".into(), "closure".into())
            .unwrap();
        let closure_statistics = db.statistics(start.clone(), end.clone(), 480).unwrap();
        assert_eq!(closure_statistics.summary.rate_mode, "closure");
        assert_eq!(closure_statistics.summary.rate_numerator, 1);
        assert_eq!(closure_statistics.summary.rate_denominator, 1);
        assert_eq!(closure_statistics.summary.completion_rate, 1.0);
        let details = db
            .statistics_details(start.clone(), end.clone(), "任务处理".into())
            .unwrap();
        assert_eq!(details.len(), 1);
        assert_eq!(details[0].result_status, "completed");
        assert_eq!(details[0].handling_count, 3);

        let first = db
            .list_work_events(created.id)
            .unwrap()
            .into_iter()
            .find(|event| event.is_first_valid)
            .unwrap();
        let changed_time = (Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
        assert!(db
            .update_work_event(WorkEventUpdateInput {
                id: first.id,
                result_status: first.result_status.clone(),
                handled_at: changed_time.clone(),
                note: first.note.clone(),
                confirm_historical_impact: false,
            })
            .is_err());
        db.update_work_event(WorkEventUpdateInput {
            id: first.id,
            result_status: first.result_status,
            handled_at: changed_time,
            note: first.note,
            confirm_historical_impact: true,
        })
        .unwrap();
        assert!(db.void_work_event(first.id, true).is_err());

        db.soft_delete(created.id).unwrap();
        assert_eq!(
            db.statistics(start, end, 480)
                .unwrap()
                .summary
                .handled_tasks,
            0
        );

        drop(db);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn migration_v6_backfills_queue_events_and_clears_handled_urgency() {
        let root = std::env::temp_dir().join(format!(
            "inline-migration-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("inline.db");
        let db = Database::open_at(path.clone()).unwrap();
        let created = db.save_task(sample("旧版迁移事项")).unwrap();
        drop(db);

        let legacy = Connection::open(&path).unwrap();
        legacy
            .execute_batch(
                "DROP TABLE task_work_events;
                 DROP TABLE task_queue_entries;
                 DELETE FROM schema_meta;
                 INSERT INTO schema_meta(version) VALUES(4);",
            )
            .unwrap();
        legacy
            .execute(
                "UPDATE tasks SET status='waiting_materials',department='法务组',is_urgent=1 WHERE id=?",
                [created.id],
            )
            .unwrap();
        legacy
            .execute(
                "INSERT INTO urgent_records(task_id,requester,reason,requested_at,confirmation_status)
                 VALUES(?, '测试人', '旧版加急', ?, 'confirmed')",
                params![created.id, now()],
            )
            .unwrap();
        legacy
            .execute(
                "INSERT INTO status_history(task_id,old_status,new_status,reason,created_at)
                 VALUES(?,'pending','waiting_materials','旧版记录',?)",
                params![created.id, now()],
            )
            .unwrap();
        drop(legacy);

        let migrated = Database::open_at(path).unwrap();
        assert_eq!(migrated.with_conn(Database::schema_version).unwrap(), 6);
        let task = migrated.get_task(created.id).unwrap();
        assert_eq!(task.departments, vec!["法务组"]);
        assert!(!task.has_active_queue);
        assert!(!task.is_urgent);
        let active_urgent_records: i64 = migrated
            .with_conn(|connection| {
                connection
                    .query_row(
                        "SELECT count(*) FROM urgent_records WHERE task_id=? AND cancelled_at IS NULL",
                        [created.id],
                        |row| row.get(0),
                    )
                    .map_err(display_error)
            })
            .unwrap();
        assert_eq!(active_urgent_records, 0);
        let events = migrated.list_work_events(created.id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].result_status, "waiting_materials");

        drop(migrated);
        let _ = fs::remove_dir_all(root);
    }
}
