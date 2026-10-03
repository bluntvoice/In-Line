use crate::models::*;
mod scheduling;
use chrono::{Datelike, FixedOffset, Local, Utc};
use rusqlite::{
    params, params_from_iter, types::Value, Connection, OpenFlags, OptionalExtension, Transaction,
};
use scheduling::*;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

const SELECT_TASK: &str = "SELECT tasks.id, permanent_number, daily_sequence, ticket_date, department, contact, task_type, title, details, status, priority, workload, is_urgent, urgent_requester, urgent_reason, requested_deadline, internal_notes, created_at, updated_at, started_at, completed_at, archived_at, deleted_at, custom_sort_order, requested_deadline_label,
    (SELECT count(*) FROM task_work_events work WHERE work.task_id=tasks.id AND work.voided_at IS NULL),
    EXISTS(SELECT 1 FROM task_queue_entries queue_entry WHERE queue_entry.task_id=tasks.id AND queue_entry.closed_at IS NULL),
    (SELECT history.created_at FROM status_history history
     WHERE history.task_id=tasks.id
       AND history.new_status IN ('waiting_materials','waiting_confirmation','waiting_counterparty_confirmation','paused','processed')
       AND (history.old_status IS NULL OR history.old_status NOT IN ('waiting_materials','waiting_confirmation','waiting_counterparty_confirmation','paused','processed'))
     ORDER BY history.id DESC LIMIT 1), is_import_conflict, parent_task_id, subtask_sort_order,
     planned_date,is_scheduled,schedule_action,schedule_action_at
    FROM tasks";
const OVERDUE_RANK_SQL: &str = "CASE WHEN requested_deadline IS NOT NULL AND strftime('%s',requested_deadline) < strftime('%s','now') THEN 0 ELSE 1 END";

pub struct Database {
    backup_dir: PathBuf,
    connection: Mutex<Option<Connection>>,
}

impl Database {
    pub fn open() -> Result<Self, String> {
        let root = dirs::config_dir()
            .ok_or("无法定位应用数据目录")?
            .join("in-line");
        Self::open_root(root)
    }

    fn open_root(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(display_error)?;
        let path = root.join("inline.db");
        let backup_dir = root.join("backups");
        fs::create_dir_all(&backup_dir).map_err(display_error)?;
        Self::normalize_backup_names(&backup_dir)?;
        let existed = path.exists();
        let mut connection = Self::connect(&path)?;
        if existed && Self::schema_version(&connection)? < 9 {
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
            backup_dir,
            connection: Mutex::new(Some(connection)),
        })
    }

    pub fn open_reporting() -> Result<Self, String> {
        let root = dirs::config_dir()
            .ok_or("无法定位应用数据目录")?
            .join("in-line");
        let path = root.join("inline.db");
        #[cfg(debug_assertions)]
        let path = std::env::var_os("IN_LINE_MCP_DATABASE_PATH")
            .map(PathBuf::from)
            .unwrap_or(path);
        Self::open_reporting_path(path)
    }

    fn open_reporting_path(path: PathBuf) -> Result<Self, String> {
        if !path.is_file() {
            return Err("找不到 In Line 数据库，请先启动一次主程序".into());
        }
        let connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(display_error)?;
        connection
            .execute_batch("PRAGMA query_only=ON; PRAGMA busy_timeout=5000;")
            .map_err(display_error)?;
        if Self::schema_version(&connection)? < 9 {
            return Err("数据库版本过旧，请先启动 In Line 完成升级".into());
        }
        let backup_dir = path.parent().ok_or("数据库路径无效")?.join("backups");
        Ok(Self {
            backup_dir,
            connection: Mutex::new(Some(connection)),
        })
    }

    #[cfg(test)]
    pub fn open_reporting_at(path: PathBuf) -> Result<Self, String> {
        Self::open_reporting_path(path)
    }

    #[cfg(any(test, debug_assertions))]
    pub fn open_at(path: PathBuf) -> Result<Self, String> {
        let backup_dir = path.parent().unwrap().join("backups");
        fs::create_dir_all(&backup_dir).map_err(display_error)?;
        let mut connection = Self::connect(&path)?;
        Self::migrate(&mut connection)?;
        Ok(Self {
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
               is_import_conflict INTEGER NOT NULL DEFAULT 0,
               parent_task_id INTEGER,
               subtask_sort_order INTEGER NOT NULL DEFAULT 0,
               FOREIGN KEY(parent_task_id) REFERENCES tasks(id) ON DELETE SET NULL,
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
             CREATE INDEX IF NOT EXISTS idx_logs_task ON task_logs(task_id,created_at DESC);
             CREATE INDEX IF NOT EXISTS idx_status_history_task ON status_history(task_id,id DESC);"
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
        if version < 7 {
            let has_import_conflict: i64 = transaction
                .query_row(
                    "SELECT count(*) FROM pragma_table_info('tasks') WHERE name='is_import_conflict'",
                    [],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            if has_import_conflict == 0 {
                transaction
                    .execute(
                        "ALTER TABLE tasks ADD COLUMN is_import_conflict INTEGER NOT NULL DEFAULT 0",
                        [],
                    )
                    .map_err(display_error)?;
            }
            transaction
                .execute(
                    "UPDATE tasks SET is_import_conflict=1
                     WHERE title LIKE '%（冲突）' OR title GLOB '*（冲突 [0-9]*）'",
                    [],
                )
                .map_err(display_error)?;
            transaction
                .execute_batch(
                    "DELETE FROM schema_meta; INSERT INTO schema_meta(version) VALUES(7);",
                )
                .map_err(display_error)?;
        }
        if version < 8 {
            let has_parent_task_id: i64 = transaction
                .query_row(
                    "SELECT count(*) FROM pragma_table_info('tasks') WHERE name='parent_task_id'",
                    [],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            if has_parent_task_id == 0 {
                transaction
                    .execute(
                        "ALTER TABLE tasks ADD COLUMN parent_task_id INTEGER REFERENCES tasks(id) ON DELETE SET NULL",
                        [],
                    )
                    .map_err(display_error)?;
            }
            let has_subtask_sort_order: i64 = transaction
                .query_row(
                    "SELECT count(*) FROM pragma_table_info('tasks') WHERE name='subtask_sort_order'",
                    [],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            if has_subtask_sort_order == 0 {
                transaction
                    .execute(
                        "ALTER TABLE tasks ADD COLUMN subtask_sort_order INTEGER NOT NULL DEFAULT 0",
                        [],
                    )
                    .map_err(display_error)?;
            }
            transaction
                .execute_batch(
                    "CREATE INDEX IF NOT EXISTS idx_tasks_parent
                       ON tasks(parent_task_id,subtask_sort_order,id);
                     DELETE FROM schema_meta;
                     INSERT INTO schema_meta(version) VALUES(8);",
                )
                .map_err(display_error)?;
        }
        migrate_scheduling(&transaction, version)?;
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
            deferred_entered_at: row.get(27)?,
            is_import_conflict: row.get::<_, i64>(28)? != 0,
            parent_task_id: row.get(29)?,
            subtask_sort_order: row.get(30)?,
            planned_date: row.get(31)?,
            is_scheduled: row.get::<_, i64>(32)? != 0,
            schedule_action: row.get(33)?,
            schedule_action_at: row.get(34)?,
        })
    }

    pub fn list_tasks(&self, view: TaskView) -> Result<Vec<LegalTask>, String> {
        self.activate_due_scheduled()?;
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
    #[cfg(test)]
    if let Some(value) = TEST_TIME.with(|clock| clock.borrow().clone()) {
        return value.to_rfc3339();
    }
    Utc::now().to_rfc3339()
}
fn today() -> String {
    #[cfg(test)]
    if let Some(value) = TEST_TIME.with(|clock| clock.borrow().clone()) {
        return value.format("%Y-%m-%d").to_string();
    }
    Local::now().format("%Y-%m-%d").to_string()
}
#[cfg(test)]
thread_local! {
    static TEST_TIME: std::cell::RefCell<Option<chrono::DateTime<FixedOffset>>> = const { std::cell::RefCell::new(None) };
}
fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn valid_setting(key: &str, value: &str) -> bool {
    match key {
        "show_deferred_in_queue" | "launch_at_login" => matches!(value, "true" | "false"),
        "week_start_day" => matches!(value, "monday" | "sunday"),
        "statistics_rate_mode" => matches!(value, "closure" | "processing"),
        "global_shortcut" => value.is_ascii() && value.len() <= 64 && value.contains('+'),
        "ui_font_family" => value.len() <= 256 && !value.chars().any(char::is_control),
        "ticket_colors" => {
            value.len() <= 256
                && serde_json::from_str::<HashMap<String, String>>(value).is_ok_and(|colors| {
                    colors.len() == 3
                        && ["normal", "future", "urgent"].iter().all(|key| {
                            colors.get(*key).is_some_and(|color| {
                                color.len() == 7
                                    && color.starts_with('#')
                                    && color[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
                            })
                        })
                })
        }
        _ => false,
    }
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
            "SELECT MAX(value) FROM (
                SELECT COALESCE(MAX(last_sequence),0) value FROM daily_sequences WHERE ticket_date=?1
                UNION ALL SELECT COALESCE(MAX(daily_sequence),0) FROM queue_number_allocations WHERE queue_date=?1
                UNION ALL SELECT COALESCE(MAX(daily_sequence),0) FROM task_queue_entries WHERE queue_date=?1
                UNION ALL SELECT COALESCE(MAX(daily_sequence),0) FROM tasks WHERE ticket_date=?1
             )",
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
    if task.is_scheduled {
        let entered = enter_current_workflow_on(connection, task_id)?;
        return Ok((entered.ticket_date, entered.daily_sequence));
    }
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
    void_number_on(connection, task_id, "重新加入今日队列")?;
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
    connection.execute("UPDATE tasks SET planned_date=?,is_scheduled=0,schedule_action='',schedule_action_at=NULL WHERE id=?",params![date,task_id]).map_err(display_error)?;
    allocate_number_on(connection, task_id, &date, sequence)?;
    connection.execute("UPDATE queue_number_allocations SET activated_at=? WHERE task_id=? AND queue_date=? AND daily_sequence=?",params![stamp,task_id,date,sequence]).map_err(display_error)?;
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

fn normalize_subtask_order(connection: &Connection, parent_task_id: i64) -> Result<(), String> {
    let ids = {
        let mut statement = connection
            .prepare(
                "SELECT id FROM tasks WHERE parent_task_id=?
                 ORDER BY subtask_sort_order,id",
            )
            .map_err(display_error)?;
        let rows = statement
            .query_map([parent_task_id], |row| row.get::<_, i64>(0))
            .map_err(display_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(display_error)?;
        rows
    };
    for (index, id) in ids.into_iter().enumerate() {
        connection
            .execute(
                "UPDATE tasks SET subtask_sort_order=? WHERE id=?",
                params![index as i64 + 1, id],
            )
            .map_err(display_error)?;
    }
    Ok(())
}

fn validate_parent_assignment(
    connection: &Connection,
    task_id: i64,
    parent_task_id: i64,
) -> Result<(LegalTask, LegalTask), String> {
    if task_id == parent_task_id {
        return Err("事项不能设为自己的子任务".into());
    }
    let task = get_task_on(connection, task_id)?;
    let parent = get_task_on(connection, parent_task_id)?;
    if task.deleted_at.is_some() || parent.deleted_at.is_some() {
        return Err("回收站中的事项不能新建或更换所属关系".into());
    }
    if parent.parent_task_id.is_some() {
        return Err("子任务不能继续作为所属任务，当前仅支持两级结构".into());
    }
    let child_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM tasks WHERE parent_task_id=?",
            [task_id],
            |row| row.get(0),
        )
        .map_err(display_error)?;
    if child_count > 0 {
        return Err("已有子任务的事项不能再设为其他事项的子任务".into());
    }
    let would_cycle: i64 = connection
        .query_row(
            "WITH RECURSIVE descendants(id) AS (
               SELECT id FROM tasks WHERE parent_task_id=?
               UNION ALL
               SELECT tasks.id FROM tasks JOIN descendants ON tasks.parent_task_id=descendants.id
             )
             SELECT EXISTS(SELECT 1 FROM descendants WHERE id=?)",
            params![task_id, parent_task_id],
            |row| row.get(0),
        )
        .map_err(display_error)?;
    if would_cycle != 0 {
        return Err("所属关系会形成循环".into());
    }
    Ok((task, parent))
}

fn child_ids_on(connection: &Connection, parent_task_id: i64) -> Result<Vec<i64>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id FROM tasks WHERE parent_task_id=?
             ORDER BY subtask_sort_order,id",
        )
        .map_err(display_error)?;
    let rows = statement
        .query_map([parent_task_id], |row| row.get::<_, i64>(0))
        .map_err(display_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(display_error)?;
    Ok(rows)
}

fn incomplete_subtask_count(connection: &Connection, parent_task_id: i64) -> Result<i64, String> {
    connection
        .query_row(
            "SELECT count(*) FROM tasks
             WHERE parent_task_id=? AND deleted_at IS NULL AND archived_at IS NULL
               AND status NOT IN ('completed','cancelled','archived')",
            [parent_task_id],
            |row| row.get(0),
        )
        .map_err(display_error)
}

fn ensure_direct_completion_allowed(connection: &Connection, task_id: i64) -> Result<(), String> {
    if incomplete_subtask_count(connection, task_id)? > 0 {
        return Err("该事项仍有未完成子任务，请先选择仅完成父任务或同时完成全部子任务".into());
    }
    Ok(())
}

fn subtask_completion_state_on(
    connection: &Connection,
    task_id: i64,
) -> Result<Option<SubtaskCompletionState>, String> {
    let task = get_task_on(connection, task_id)?;
    let parent_task_id = if let Some(parent_task_id) = task.parent_task_id {
        parent_task_id
    } else if !child_ids_on(connection, task.id)?.is_empty() {
        task.id
    } else {
        return Ok(None);
    };
    let parent = get_task_on(connection, parent_task_id)?;
    let (total_subtasks, completed_subtasks, eligible_subtasks, completed_eligible_subtasks) =
        connection
            .query_row(
                "SELECT count(*),
                        sum(CASE WHEN status='completed' THEN 1 ELSE 0 END),
                        sum(CASE WHEN deleted_at IS NULL AND archived_at IS NULL
                                      AND status NOT IN ('cancelled','archived') THEN 1 ELSE 0 END),
                        sum(CASE WHEN deleted_at IS NULL AND archived_at IS NULL
                                      AND status='completed' THEN 1 ELSE 0 END)
                 FROM tasks WHERE parent_task_id=?",
                [parent_task_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .map_err(display_error)?;
    Ok(Some(SubtaskCompletionState {
        parent_task_id,
        total_subtasks,
        completed_subtasks,
        eligible_subtasks,
        completed_eligible_subtasks,
        all_eligible_subtasks_completed: eligible_subtasks > 0
            && eligible_subtasks == completed_eligible_subtasks,
        parent_can_be_completed: parent.deleted_at.is_none()
            && parent.archived_at.is_none()
            && !matches!(
                parent.status.as_str(),
                "completed" | "cancelled" | "archived"
            ),
    }))
}

fn complete_task_on(connection: &Connection, id: i64, reason: &str) -> Result<(), String> {
    let task = enter_current_workflow_on(connection, id)?;
    if task.deleted_at.is_some()
        || task.archived_at.is_some()
        || matches!(task.status.as_str(), "completed" | "cancelled" | "archived")
    {
        return Err("该事项已经完成、取消或归档".into());
    }
    let stamp = now();
    connection
        .execute(
            "UPDATE tasks SET status='completed',completed_at=?,updated_at=? WHERE id=?",
            params![stamp, stamp, id],
        )
        .map_err(display_error)?;
    clear_urgent_on(connection, id, "事项已完成")?;
    add_status(connection, id, Some(&task.status), "completed", reason)?;
    close_active_queue(connection, id, reason)?;
    record_work_event_on(
        connection,
        id,
        "completed",
        &stamp,
        &task.task_type,
        "quick_action",
        "",
    )?;
    add_log(connection, id, "work", "本轮已完成，事项整体结束")
}

fn archive_task_on(connection: &Connection, id: i64, reason: &str) -> Result<bool, String> {
    let task = get_task_on(connection, id)?;
    if task.deleted_at.is_some() {
        return Err("回收站事项不能归档".into());
    }
    if task.archived_at.is_some() || task.status == "archived" {
        return Ok(false);
    }
    let stamp = now();
    stop_scheduled_on(connection, id, "已归档", true)?;
    connection
        .execute(
            "UPDATE tasks SET status='archived',archived_at=?,updated_at=? WHERE id=?",
            params![stamp, stamp, id],
        )
        .map_err(display_error)?;
    close_active_queue(connection, id, reason)?;
    add_status(connection, id, Some(&task.status), "archived", reason)?;
    add_log(connection, id, "archived", "事项已归档")?;
    Ok(true)
}

fn soft_delete_task_on(connection: &Connection, id: i64, reason: &str) -> Result<bool, String> {
    let task = get_task_on(connection, id)?;
    if task.deleted_at.is_some() {
        return Ok(false);
    }
    let stamp = now();
    stop_scheduled_on(connection, id, "移入回收站", true)?;
    connection
        .execute(
            "UPDATE tasks SET deleted_at=?,updated_at=? WHERE id=?",
            params![stamp, stamp, id],
        )
        .map_err(display_error)?;
    clear_urgent_on(connection, id, reason)?;
    close_active_queue(connection, id, reason)?;
    add_log(connection, id, "deleted", "事项移入回收站")?;
    Ok(true)
}

impl Database {
    pub fn save_task(&self, mut input: TaskInput) -> Result<LegalTask, String> {
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
        plan_changed = plan_changed
            || (terminal && input.planned_date.is_some() && input.confirm_schedule_change);
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
        transaction.commit().map_err(display_error)?;
        get_task_on(connection, id)
    }

    pub fn create_subtask(&self, input: CreateSubtaskInput) -> Result<LegalTask, String> {
        self.with_transaction(|transaction| {
            let parent = get_task_on(transaction, input.parent_task_id)?;
            if parent.deleted_at.is_some() {
                return Err("回收站中的事项不能新增子任务".into());
            }
            if parent.parent_task_id.is_some() {
                return Err("子任务不能继续新增下级任务，当前仅支持两级结构".into());
            }

            let departments = input
                .departments
                .clone()
                .unwrap_or_else(|| parent.departments.clone());
            let contacts = input
                .contacts
                .clone()
                .unwrap_or_else(|| parent.contacts.clone());
            let task = TaskInput {
                planned_date: input.planned_date.clone(),
                confirm_schedule_change: false,
                id: None,
                department: departments.first().cloned().unwrap_or_default(),
                departments,
                contact: contacts.first().cloned().unwrap_or_default(),
                contacts,
                task_type: input
                    .task_type
                    .clone()
                    .unwrap_or_else(|| parent.task_type.clone()),
                title: input.title.clone(),
                details: input.details.clone(),
                status: "pending".into(),
                priority: input.priority.clone().unwrap_or_else(|| "normal".into()),
                workload: input
                    .workload
                    .clone()
                    .unwrap_or_else(|| "standard".into()),
                is_urgent: input.is_urgent,
                urgent_requester: input.urgent_requester.clone(),
                urgent_reason: input.urgent_reason.clone(),
                requested_deadline: input.requested_deadline.clone(),
                requested_deadline_label: input.requested_deadline_label.clone(),
                internal_notes: input.internal_notes.clone(),
            };
            validate_task_input(&task)?;
            let contacts = normalized_contacts(&task);
            let departments = normalized_departments(&task);
            let stored_contacts = contact_storage(&contacts)?;
            let stored_departments = contact_storage(&departments)?;
            let identity_date = today();
            let date = input.planned_date.clone().unwrap_or_else(|| parent.planned_date.clone().max(identity_date.clone()));
            validate_plan(&date, task.requested_deadline.as_deref(), true)?;
            let scheduled = date > identity_date;
            let identity_sequence = next_daily_sequence(transaction, &identity_date)?;
            let sequence = if scheduled {next_daily_sequence(transaction, &date)?} else {identity_sequence};
            let permanent = format!("{}-{:02}", identity_date.replace('-', ""), identity_sequence);
            let custom_order: i64 = transaction
                .query_row(
                    "SELECT COALESCE(MAX(custom_sort_order),0)+1 FROM tasks",
                    [],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            let subtask_order: i64 = transaction
                .query_row(
                    "SELECT COALESCE(MAX(subtask_sort_order),0)+1 FROM tasks WHERE parent_task_id=?",
                    [parent.id],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            let stamp = now();
            transaction
                .execute(
                    "INSERT INTO tasks(
                       permanent_number,daily_sequence,ticket_date,department,contact,task_type,title,details,
                       status,priority,workload,is_urgent,urgent_requester,urgent_reason,requested_deadline,
                       requested_deadline_label,internal_notes,created_at,updated_at,custom_sort_order,
                       parent_task_id,subtask_sort_order
                     ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                    params![
                        permanent,
                        sequence,
                        date,
                        stored_departments,
                        stored_contacts,
                        task.task_type.trim(),
                        task.title.trim(),
                        task.details.trim(),
                        task.status,
                        task.priority,
                        task.workload,
                        task.is_urgent as i64,
                        task.urgent_requester.trim(),
                        task.urgent_reason.trim(),
                        task.requested_deadline,
                        task.requested_deadline_label,
                        task.internal_notes.trim(),
                        stamp,
                        stamp,
                        custom_order,
                        parent.id,
                        subtask_order
                    ],
                )
                .map_err(display_error)?;
            let id = transaction.last_insert_rowid();
            transaction.execute("UPDATE tasks SET planned_date=?,is_scheduled=? WHERE id=?",params![date,scheduled as i64,id]).map_err(display_error)?;
            allocate_number_on(transaction,id,&date,sequence)?;
            if scheduled {add_log(transaction,id,"scheduled_created",&format!("子任务提前取号：计划 {}，队列 {}-{:02}；未实际入队",date,date,sequence))?;}

            if input.enqueue_today && !scheduled {
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
                            task.requested_deadline,
                            task.requested_deadline_label,
                            stamp,
                            stamp,
                            stamp
                        ],
                    )
                    .map_err(display_error)?;
            }
            if input.enqueue_today && !scheduled {transaction.execute("UPDATE queue_number_allocations SET activated_at=? WHERE task_id=?",params![stamp,id]).map_err(display_error)?;}
            add_log(
                transaction,
                id,
                "created",
                &if input.enqueue_today && !scheduled {
                    format!("创建子任务并取号：{permanent}")
                } else {
                    format!("创建子任务（未加入今日队列）：{permanent}")
                },
            )?;
            add_log(
                transaction,
                id,
                "relation",
                &format!("设置所属任务：{}", parent.title),
            )?;
            add_status(transaction, id, None, "pending", "创建子任务")?;
            if task.is_urgent {
                record_urgent(transaction, id, &task)?;
                if input.enqueue_today && !scheduled {promote_one(transaction, id)?;}
            }
            ensure_master(transaction, "task_type", &task.task_type)?;
            bump_master_use(transaction, "task_type", &task.task_type)?;
            for department in &departments {
                ensure_master(transaction, "department", department)?;
                bump_master_use(transaction, "department", department)?;
            }
            for contact in &contacts {
                ensure_master(transaction, "contact", contact)?;
                bump_master_use(transaction, "contact", contact)?;
            }
            get_task_on(transaction, id)
        })
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

    pub fn list_parent_task_candidates(&self, task_id: i64) -> Result<Vec<LegalTask>, String> {
        self.with_conn(|connection| {
            let task = get_task_on(connection, task_id)?;
            let child_count: i64 = connection
                .query_row(
                    "SELECT count(*) FROM tasks WHERE parent_task_id=?",
                    [task_id],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            if task.deleted_at.is_some() || child_count > 0 {
                return Ok(Vec::new());
            }
            let mut statement = connection
                .prepare(&format!(
                    "{SELECT_TASK} WHERE tasks.id<>? AND parent_task_id IS NULL
                     AND deleted_at IS NULL ORDER BY updated_at DESC,id DESC"
                ))
                .map_err(display_error)?;
            let rows = statement
                .query_map([task_id], Self::row_task)
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;
            Ok(rows)
        })
    }

    pub fn list_subtasks(&self, parent_task_id: i64) -> Result<Vec<LegalTask>, String> {
        self.with_conn(|connection| {
            get_task_on(connection, parent_task_id)?;
            let mut statement = connection
                .prepare(&format!(
                    "{SELECT_TASK} WHERE parent_task_id=? ORDER BY subtask_sort_order,id"
                ))
                .map_err(display_error)?;
            let rows = statement
                .query_map([parent_task_id], Self::row_task)
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;
            Ok(rows)
        })
    }

    pub fn set_parent_task(&self, task_id: i64, parent_task_id: Option<i64>) -> Result<(), String> {
        self.with_transaction(|transaction| {
            let task = get_task_on(transaction, task_id)?;
            if task.parent_task_id == parent_task_id {
                return Ok(());
            }
            let old_parent = task
                .parent_task_id
                .map(|id| get_task_on(transaction, id))
                .transpose()?;
            let stamp = now();
            let log_content = if let Some(parent_task_id) = parent_task_id {
                let (_, parent) =
                    validate_parent_assignment(transaction, task_id, parent_task_id)?;
                let next_order: i64 = transaction
                    .query_row(
                        "SELECT COALESCE(MAX(subtask_sort_order),0)+1
                         FROM tasks WHERE parent_task_id=?",
                        [parent_task_id],
                        |row| row.get(0),
                    )
                    .map_err(display_error)?;
                transaction
                    .execute(
                        "UPDATE tasks SET parent_task_id=?,subtask_sort_order=?,updated_at=? WHERE id=?",
                        params![parent_task_id, next_order, stamp, task_id],
                    )
                    .map_err(display_error)?;
                if let Some(old_parent) = old_parent.as_ref() {
                    format!(
                        "更换所属任务：{} → {}",
                        old_parent.title, parent.title
                    )
                } else {
                    format!("设置所属任务：{}", parent.title)
                }
            } else {
                transaction
                    .execute(
                        "UPDATE tasks SET parent_task_id=NULL,subtask_sort_order=0,updated_at=? WHERE id=?",
                        params![stamp, task_id],
                    )
                    .map_err(display_error)?;
                format!(
                    "解除所属任务：{}",
                    old_parent
                        .as_ref()
                        .map(|parent| parent.title.as_str())
                        .unwrap_or("未知事项")
                )
            };
            if let Some(old_parent_id) = task.parent_task_id {
                normalize_subtask_order(transaction, old_parent_id)?;
            }
            add_log(transaction, task_id, "relation", &log_content)
        })
    }

    pub fn reorder_subtasks(&self, input: ReorderSubtasksInput) -> Result<(), String> {
        self.with_transaction(|transaction| {
            get_task_on(transaction, input.parent_task_id)?;
            let current = {
                let mut statement = transaction
                    .prepare(
                        "SELECT id FROM tasks WHERE parent_task_id=?
                         ORDER BY subtask_sort_order,id",
                    )
                    .map_err(display_error)?;
                let rows = statement
                    .query_map([input.parent_task_id], |row| row.get::<_, i64>(0))
                    .map_err(display_error)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(display_error)?;
                rows
            };
            let requested = input.task_ids.iter().copied().collect::<HashSet<_>>();
            let existing = current.iter().copied().collect::<HashSet<_>>();
            if input.task_ids.len() != requested.len() || requested != existing {
                return Err("子任务排序列表与当前所属关系不一致，请刷新后重试".into());
            }
            for (index, id) in input.task_ids.iter().enumerate() {
                transaction
                    .execute(
                        "UPDATE tasks SET subtask_sort_order=? WHERE id=? AND parent_task_id=?",
                        params![index as i64 + 1, id, input.parent_task_id],
                    )
                    .map_err(display_error)?;
            }
            Ok(())
        })
    }
}

impl Database {
    pub fn subtask_completion_state(
        &self,
        task_id: i64,
    ) -> Result<Option<SubtaskCompletionState>, String> {
        self.with_conn(|connection| subtask_completion_state_on(connection, task_id))
    }

    pub fn complete_task(&self, input: CompleteTaskInput) -> Result<CompleteTaskResult, String> {
        self.with_transaction(|transaction| {
            let task = get_task_on(transaction, input.task_id)?;
            let mut completed_task_ids = Vec::new();
            if input.include_eligible_subtasks && task.parent_task_id.is_none() {
                let child_ids = {
                    let mut statement = transaction
                        .prepare(
                            "SELECT id FROM tasks
                             WHERE parent_task_id=? AND deleted_at IS NULL AND archived_at IS NULL
                               AND status NOT IN ('completed','cancelled','archived')
                             ORDER BY subtask_sort_order,id",
                        )
                        .map_err(display_error)?;
                    let rows = statement
                        .query_map([task.id], |row| row.get::<_, i64>(0))
                        .map_err(display_error)?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(display_error)?;
                    rows
                };
                for child_id in child_ids {
                    complete_task_on(transaction, child_id, "随所属任务一并完成")?;
                    completed_task_ids.push(child_id);
                }
            }
            complete_task_on(transaction, task.id, "本轮已完成")?;
            completed_task_ids.push(task.id);
            let completion_state = subtask_completion_state_on(transaction, task.id)?;
            Ok(CompleteTaskResult {
                completed_task_ids,
                completion_state,
            })
        })
    }

    pub fn archive_task_group(&self, input: ArchiveTaskInput) -> Result<ArchiveTaskResult, String> {
        self.with_transaction(|transaction| {
            let task = get_task_on(transaction, input.task_id)?;
            let mut archived_task_ids = Vec::new();
            if input.include_completed_subtasks && task.parent_task_id.is_none() {
                let child_ids = {
                    let mut statement = transaction
                        .prepare(
                            "SELECT id FROM tasks
                             WHERE parent_task_id=? AND deleted_at IS NULL AND archived_at IS NULL
                               AND status='completed'
                             ORDER BY subtask_sort_order,id",
                        )
                        .map_err(display_error)?;
                    let rows = statement
                        .query_map([task.id], |row| row.get::<_, i64>(0))
                        .map_err(display_error)?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(display_error)?;
                    rows
                };
                for child_id in child_ids {
                    if archive_task_on(transaction, child_id, "随所属任务一并归档")? {
                        archived_task_ids.push(child_id);
                    }
                }
            }
            if archive_task_on(transaction, task.id, "事项归档")? {
                archived_task_ids.push(task.id);
            }
            Ok(ArchiveTaskResult { archived_task_ids })
        })
    }

    pub fn delete_task_group(&self, input: DeleteTaskInput) -> Result<DeleteTaskResult, String> {
        self.with_transaction(|transaction| {
            let task = get_task_on(transaction, input.task_id)?;
            if task.deleted_at.is_some() {
                return Ok(DeleteTaskResult {
                    trashed_task_ids: Vec::new(),
                    detached_subtask_ids: Vec::new(),
                });
            }
            let child_ids = child_ids_on(transaction, task.id)?;
            let mut trashed_task_ids = Vec::new();
            let mut detached_subtask_ids = Vec::new();
            if !child_ids.is_empty() {
                if input.include_subtasks {
                    for child_id in &child_ids {
                        if soft_delete_task_on(transaction, *child_id, "随所属任务移入回收站")? {
                            trashed_task_ids.push(*child_id);
                        }
                    }
                } else {
                    let stamp = now();
                    for child_id in &child_ids {
                        transaction
                            .execute(
                                "UPDATE tasks SET parent_task_id=NULL,subtask_sort_order=0,updated_at=? WHERE id=?",
                                params![stamp, child_id],
                            )
                            .map_err(display_error)?;
                        add_log(
                            transaction,
                            *child_id,
                            "relation",
                            &format!("所属任务《{}》移入回收站，已自动解除所属关系", task.title),
                        )?;
                        detached_subtask_ids.push(*child_id);
                    }
                }
            }
            if soft_delete_task_on(transaction, task.id, "事项移入回收站")? {
                trashed_task_ids.push(task.id);
            }
            Ok(DeleteTaskResult {
                trashed_task_ids,
                detached_subtask_ids,
            })
        })
    }

    pub fn set_status(&self, id: i64, status: String) -> Result<(), String> {
        if !ALL_STATUSES.contains(&status.as_str()) {
            return Err("事项状态无效".into());
        }
        self.with_transaction(|transaction| {
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
        })
    }

    pub fn set_urgent(
        &self,
        id: i64,
        is_urgent: bool,
        requester: String,
        reason: String,
    ) -> Result<(), String> {
        self.with_transaction(|transaction| {
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
        })
    }

    pub fn move_task(&self, id: i64, direction: MoveDirection) -> Result<(), String> {
        self.with_transaction(|transaction| {
            let task = get_task_on(transaction,id)?;
            if task.is_scheduled {return Err("未来事项按加入日期及队列序号排序，不能人工调序".into());}
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
        self.delete_task_group(DeleteTaskInput {
            task_id: id,
            include_subtasks: false,
        })?;
        Ok(())
    }
    pub fn archive(&self, id: i64) -> Result<(), String> {
        self.archive_task_group(ArchiveTaskInput {
            task_id: id,
            include_completed_subtasks: false,
        })?;
        Ok(())
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

            let source_child_ids = child_ids_on(tx, source.id)?;
            if !source_child_ids.is_empty() && target.parent_task_id.is_some() {
                return Err("包含子任务的事项只能合并到顶层事项".into());
            }

            stop_scheduled_on(tx,source.id,"合并至其他事项",true)?;
            void_number_on(tx,source.id,"合并至其他事项")?;
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
                "queue_number_allocations",
            ] {
                tx.execute(
                    &format!("UPDATE {table} SET task_id=? WHERE task_id=?"),
                    params![target.id, source.id],
                )
                .map_err(display_error)?;
            }

            let stamp = now();
            tx.execute(
                "UPDATE tasks SET updated_at=?,is_import_conflict=0 WHERE id=?",
                params![stamp, target.id],
            )
            .map_err(display_error)?;

            if !source_child_ids.is_empty() {
                let mut next_order: i64 = tx
                    .query_row(
                        "SELECT COALESCE(MAX(subtask_sort_order),0) FROM tasks WHERE parent_task_id=?",
                        [target.id],
                        |row| row.get(0),
                    )
                    .map_err(display_error)?;
                for child_id in source_child_ids {
                    next_order += 1;
                    tx.execute(
                        "UPDATE tasks SET parent_task_id=?,subtask_sort_order=?,updated_at=? WHERE id=?",
                        params![target.id, next_order, stamp, child_id],
                    )
                    .map_err(display_error)?;
                    add_log(
                        tx,
                        child_id,
                        "relation",
                        &format!(
                            "所属任务因合并更换：{} → {}",
                            source.title, target.title
                        ),
                    )?;
                }
            }
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
                     deleted_at=CASE WHEN ? THEN ? ELSE NULL END,updated_at=?,is_import_conflict=0
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
    pub fn resolve_import_conflict(&self, id: i64) -> Result<(), String> {
        self.with_transaction(|tx| {
            let task = get_task_on(tx, id)?;
            if !task.is_import_conflict {
                return Err("该事项没有待复核的导入冲突".into());
            }
            let stamp = now();
            tx.execute(
                "UPDATE tasks SET is_import_conflict=0,updated_at=? WHERE id=?",
                params![stamp, id],
            )
            .map_err(display_error)?;
            add_log(tx, id, "import_conflict", "已人工复核并解除导入冲突标识")
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

    pub fn permanently_delete_tasks(&self, mut ids: Vec<i64>) -> Result<usize, String> {
        if ids.is_empty() {
            return Err("未选择需要永久删除的事项".into());
        }
        ids.sort_unstable();
        ids.dedup();
        self.with_transaction(|tx| {
            for id in &ids {
                let task = get_task_on(tx, *id)?;
                if task.deleted_at.is_none() {
                    return Err(format!("事项 {} 不在回收站中", task.permanent_number));
                }
            }
            let deleting_ids = ids.iter().copied().collect::<HashSet<_>>();
            let stamp = now();
            for id in &ids {
                let parent = get_task_on(tx, *id)?;
                for child_id in child_ids_on(tx, *id)? {
                    if !deleting_ids.contains(&child_id) {
                        add_log(
                            tx,
                            child_id,
                            "relation",
                            &format!(
                                "所属任务《{}》已永久删除，已自动解除所属关系",
                                parent.title
                            ),
                        )?;
                    }
                }
                tx.execute(
                    "UPDATE tasks SET parent_task_id=NULL,subtask_sort_order=0,updated_at=? WHERE parent_task_id=?",
                    params![stamp, id],
                )
                .map_err(display_error)?;
            }
            let mut deleted = 0;
            for id in ids {
                deleted += tx
                    .execute(
                        "DELETE FROM tasks WHERE id=? AND deleted_at IS NOT NULL",
                        [id],
                    )
                    .map_err(display_error)?;
            }
            Ok(deleted)
        })
    }

    pub fn empty_trash(&self) -> Result<usize, String> {
        self.with_transaction(|tx| {
            let detached_children = {
                let mut statement = tx
                    .prepare(
                        "SELECT child.id,parent.title FROM tasks child
                         JOIN tasks parent ON parent.id=child.parent_task_id
                         WHERE parent.deleted_at IS NOT NULL AND child.deleted_at IS NULL
                         ORDER BY child.id",
                    )
                    .map_err(display_error)?;
                let rows = statement
                    .query_map([], |row| {
                        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                    })
                    .map_err(display_error)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(display_error)?;
                rows
            };
            for (child_id, parent_title) in detached_children {
                add_log(
                    tx,
                    child_id,
                    "relation",
                    &format!(
                        "所属任务《{}》已从回收站永久删除，已自动解除所属关系",
                        parent_title
                    ),
                )?;
            }
            tx.execute(
                "UPDATE tasks SET parent_task_id=NULL,subtask_sort_order=0,updated_at=?
                 WHERE parent_task_id IN (SELECT id FROM tasks WHERE deleted_at IS NOT NULL)",
                [now()],
            )
            .map_err(display_error)?;
            tx.execute("DELETE FROM tasks WHERE deleted_at IS NOT NULL", [])
                .map_err(display_error)
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
            let task = enter_current_workflow_on(tx, id)?;
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
            ensure_direct_completion_allowed(tx, id)?;
            complete_task_on(tx, id, "本轮已完成")
        })
    }

    pub fn record_work_event(&self, input: WorkEventInput) -> Result<(), String> {
        if !is_work_event_status(&input.result_status) {
            return Err("处理结果无效".into());
        }
        validate_handled_at(&input.handled_at)?;
        self.with_transaction(|tx| {
            let mut task = get_task_on(tx, input.task_id)?;
            if task.deleted_at.is_some() {
                return Err("回收站事项不能新增处理活动".into());
            }
            if input.sync_status && (task.archived_at.is_some() || task.status == "archived") {
                return Err(
                    "已归档事项请先重新开启；也可以取消勾选同步状态，仅补录处理活动".into(),
                );
            }
            if input.sync_status
                && matches!(input.result_status.as_str(), "processed" | "completed")
            {
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

    pub fn void_work_event(&self, id: i64, confirm_historical_impact: bool) -> Result<(), String> {
        self.with_transaction(|tx| {
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
    pub fn work_calendar(&self, start: String, end: String) -> Result<WorkCalendarResult, String> {
        let start_time = chrono::DateTime::parse_from_rfc3339(&start)
            .map_err(|_| "日历开始时间格式无效".to_string())?;
        let end_time = chrono::DateTime::parse_from_rfc3339(&end)
            .map_err(|_| "日历结束时间格式无效".to_string())?;
        if end_time <= start_time {
            return Err("日历结束时间必须晚于开始时间".into());
        }
        if (end_time - start_time).num_days() > 370 {
            return Err("单次日历查询范围不能超过 370 天".into());
        }
        let start_millis = start_time.timestamp_millis();
        let end_millis = end_time.timestamp_millis();
        let generated_at = now();
        self.with_conn(|connection| {
            #[derive(Clone)]
            struct CalendarQueueRow {
                task_id: i64,
                permanent_number: String,
                title: String,
                task_type: String,
                queue_entry_id: i64,
                enqueued_at: String,
                closed_at: Option<String>,
                close_reason: String,
                round_index: i64,
            }

            let mut queue_statement = connection
                .prepare(
                    "WITH ranked AS (
                       SELECT entry.id AS queue_entry_id,entry.task_id,entry.enqueued_at,entry.closed_at,
                              entry.close_reason,tasks.permanent_number,tasks.title,tasks.task_type,
                              ROW_NUMBER() OVER(
                                PARTITION BY entry.task_id
                                ORDER BY strftime('%s',entry.enqueued_at),entry.id
                              ) AS round_index
                       FROM task_queue_entries entry
                       JOIN tasks ON tasks.id=entry.task_id
                       WHERE tasks.deleted_at IS NULL
                     )
                     SELECT task_id,permanent_number,title,task_type,queue_entry_id,enqueued_at,
                            closed_at,close_reason,round_index
                     FROM ranked
                     WHERE strftime('%s',enqueued_at)<strftime('%s',?2)
                       AND (closed_at IS NULL OR strftime('%s',closed_at)>strftime('%s',?1))
                     ORDER BY strftime('%s',enqueued_at),queue_entry_id",
                )
                .map_err(display_error)?;
            let queue_rows = queue_statement
                .query_map(params![start, end], |row| {
                    Ok(CalendarQueueRow {
                        task_id: row.get(0)?,
                        permanent_number: row.get(1)?,
                        title: row.get(2)?,
                        task_type: row.get(3)?,
                        queue_entry_id: row.get(4)?,
                        enqueued_at: row.get(5)?,
                        closed_at: row.get(6)?,
                        close_reason: row.get(7)?,
                        round_index: row.get(8)?,
                    })
                })
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;

            let mut event_statement = connection
                .prepare(
                    "SELECT event.id,event.task_id,tasks.permanent_number,tasks.title,tasks.task_type,
                            event.result_status,event.handled_at
                     FROM task_work_events event
                     JOIN tasks ON tasks.id=event.task_id
                     WHERE event.voided_at IS NULL AND tasks.deleted_at IS NULL
                       AND strftime('%s',event.handled_at)>=strftime('%s',?1)
                       AND strftime('%s',event.handled_at)<strftime('%s',?2)
                     ORDER BY strftime('%s',event.handled_at),event.id",
                )
                .map_err(display_error)?;
            let raw_events = event_statement
                .query_map(params![start, end], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                })
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;

            let parse_stamp = |value: &str| {
                chrono::DateTime::parse_from_rfc3339(value)
                    .map(|stamp| stamp.timestamp_millis())
                    .ok()
            };
            let association_events = raw_events
                .into_iter()
                .map(
                    |(event_id, task_id, permanent_number, title, task_type, result_status, handled_at)| {
                        let handled_stamp = parse_stamp(&handled_at);
                        let round_index = queue_rows
                            .iter()
                            .filter(|entry| entry.task_id == task_id)
                            .filter(|entry| {
                                let (Some(handled), Some(enqueued)) =
                                    (handled_stamp, parse_stamp(&entry.enqueued_at))
                                else {
                                    return false;
                                };
                                let closed = entry.closed_at.as_deref().and_then(parse_stamp);
                                handled >= enqueued && closed.is_none_or(|value| handled <= value)
                            })
                            .max_by_key(|entry| parse_stamp(&entry.enqueued_at))
                            .map(|entry| entry.round_index);
                        WorkCalendarEvent {
                            event_id,
                            task_id,
                            permanent_number,
                            title,
                            task_type,
                            result_status,
                            handled_at,
                            round_index,
                        }
                    },
                )
                .collect::<Vec<_>>();
            let events = association_events
                .iter()
                .filter(|event| {
                    parse_stamp(&event.handled_at)
                        .is_some_and(|stamp| stamp >= start_millis && stamp < end_millis)
                })
                .cloned()
                .collect::<Vec<_>>();

            let result_close_reasons = [
                "本轮已处理",
                "本轮已完成",
                "记录本次处理并同步状态",
                "deferred",
                "completed",
            ];
            let mut tasks = BTreeMap::<i64, WorkCalendarTask>::new();
            for entry in queue_rows {
                let (result_status, handled_at) = if entry.closed_at.is_some()
                    && result_close_reasons.contains(&entry.close_reason.as_str())
                {
                    let end_stamp = entry.closed_at.as_deref().and_then(parse_stamp);
                    let start_stamp = parse_stamp(&entry.enqueued_at);
                    association_events
                        .iter()
                        .filter(|event| event.task_id == entry.task_id)
                        .filter(|event| {
                            let (Some(handled), Some(enqueued)) =
                                (parse_stamp(&event.handled_at), start_stamp)
                            else {
                                return false;
                            };
                            handled >= enqueued && end_stamp.is_none_or(|closed| handled <= closed)
                        })
                        .max_by_key(|event| parse_stamp(&event.handled_at))
                        .map(|event| {
                            (
                                Some(event.result_status.clone()),
                                Some(event.handled_at.clone()),
                            )
                        })
                        .unwrap_or((None, None))
                } else {
                    (None, None)
                };
                tasks
                    .entry(entry.task_id)
                    .or_insert_with(|| WorkCalendarTask {
                        task_id: entry.task_id,
                        permanent_number: entry.permanent_number,
                        title: entry.title,
                        task_type: entry.task_type,
                        intervals: Vec::new(),
                    })
                    .intervals
                    .push(WorkCalendarInterval {
                        queue_entry_id: entry.queue_entry_id,
                        enqueued_at: entry.enqueued_at,
                        current_active: entry.closed_at.is_none(),
                        closed_at: entry.closed_at,
                        round_index: entry.round_index,
                        result_status,
                        handled_at,
                    });
            }

            let mut latest_events = HashMap::<i64, &WorkCalendarEvent>::new();
            for event in &events {
                let replace = latest_events.get(&event.task_id).is_none_or(|current| {
                    (parse_stamp(&event.handled_at), event.event_id)
                        > (parse_stamp(&current.handled_at), current.event_id)
                });
                if replace {
                    latest_events.insert(event.task_id, event);
                }
            }
            let summary = WorkCalendarSummary {
                handled_tasks: latest_events.len() as i64,
                handling_rounds: events.len() as i64,
                completed_tasks: latest_events
                    .values()
                    .filter(|event| event.result_status == "completed")
                    .count() as i64,
            };

            Ok(WorkCalendarResult {
                range: WorkCalendarRange {
                    start,
                    end,
                    generated_at,
                },
                summary,
                tasks: tasks.into_values().collect(),
                events,
            })
        })
    }

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
                SELECT event.id,event.task_id,event.result_status,event.handled_at,
                       tasks.task_type AS current_task_type,tasks.department AS current_department,
                       tasks.parent_task_id AS current_parent_task_id
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
                   COALESCE(sum(current_parent_task_id IS NULL),0),
                   COALESCE(sum(current_parent_task_id IS NOT NULL),0),
                   COALESCE(sum(result_status='processed'),0),
                   COALESCE(sum(result_status='completed'),0),
                   COALESCE(sum(result_status='waiting_materials'),0),
                   COALESCE(sum(result_status='waiting_confirmation'),0),
                   COALESCE(sum(result_status='waiting_counterparty_confirmation'),0)
                 FROM ranked WHERE position=1"
            );
            let values: (i64, i64, i64, i64, i64, i64, i64, i64) = connection
                .query_row(&summary_sql, params![start, end], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
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
                             AND (tasks.requested_deadline IS NULL OR (
                               strftime('%s',tasks.requested_deadline)>=strftime('%s',?1)
                               AND strftime('%s',tasks.requested_deadline)<strftime('%s',?2)
                             ))
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
                (values.4, values.0)
            };
            let summary = StatisticsSummary {
                handled_tasks: values.0,
                top_level_tasks: values.1,
                subtasks: values.2,
                processed: values.3,
                completed: values.4,
                waiting_materials: values.5,
                waiting_confirmation: values.6,
                waiting_counterparty_confirmation: values.7,
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
                 SELECT current_task_type,count(*),
                   COALESCE(sum(result_status='completed'),0),
                   COALESCE(sum(result_status<>'completed'),0)
                 FROM ranked WHERE position=1
                 GROUP BY current_task_type ORDER BY count(*) DESC,current_task_type"
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

            let department_sql = format!(
                "{cte}
                 SELECT current_department,result_status
                 FROM ranked WHERE position=1"
            );
            let mut department_statement = connection
                .prepare(&department_sql)
                .map_err(display_error)?;
            let department_rows = department_statement
                .query_map(params![start, end], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;
            let mut departments: HashMap<String, DepartmentStatistics> = HashMap::new();
            for (stored_departments, result_status) in department_rows {
                let names = parse_contacts(&stored_departments);
                let names = if names.is_empty() {
                    vec!["未分类".to_string()]
                } else {
                    names
                };
                for department in names {
                    let entry = departments
                        .entry(department.clone())
                        .or_insert(DepartmentStatistics {
                            department,
                            handled_tasks: 0,
                            completed: 0,
                            pending_follow_up: 0,
                        });
                    entry.handled_tasks += 1;
                    if result_status == "completed" {
                        entry.completed += 1;
                    } else {
                        entry.pending_follow_up += 1;
                    }
                }
            }
            let mut by_department = departments.into_values().collect::<Vec<_>>();
            by_department.sort_by(|left, right| {
                right
                    .handled_tasks
                    .cmp(&left.handled_tasks)
                    .then_with(|| left.department.cmp(&right.department))
            });

            let mut trend_statement = connection
                .prepare(
                    "SELECT event.task_id,event.handled_at,event.result_status
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
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                })
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;
            let mut buckets: BTreeMap<String, HashMap<i64, String>> = BTreeMap::new();
            for (task_id, handled_at, result_status) in raw_trend {
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
                    .insert(task_id, result_status);
            }
            let trend = buckets
                .into_iter()
                .map(|(period_start, results)| TrendPoint {
                    period_start,
                    handled_tasks: results.len() as i64,
                    processed: results.values().filter(|status| *status == "processed").count() as i64,
                    completed: results.values().filter(|status| *status == "completed").count() as i64,
                })
                .collect();
            Ok(StatisticsResult {
                range: StatisticsRange {
                    start: start.clone(),
                    end: end.clone(),
                },
                summary,
                by_task_type,
                by_department,
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
        self.statistics_details_filtered(start, end, Some(task_type), None)
    }

    pub fn statistics_trend_details(
        &self,
        start: String,
        end: String,
        result_status: Option<String>,
    ) -> Result<Vec<StatisticsDetail>, String> {
        if result_status
            .as_deref()
            .is_some_and(|value| !matches!(value, "processed" | "completed"))
        {
            return Err("趋势结果类型无效".into());
        }
        self.statistics_details_filtered(start, end, None, result_status)
    }

    fn statistics_details_filtered(
        &self,
        start: String,
        end: String,
        task_type: Option<String>,
        result_status: Option<String>,
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
                       SELECT event.id,event.task_id,event.result_status,event.handled_at
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
                         count(*) OVER(PARTITION BY task_id) AS handling_count,
                         max(CASE WHEN result_status IN ('processed','completed') THEN 1 ELSE 0 END)
                           OVER(PARTITION BY task_id) AS has_processed_or_completed
                       FROM ranged
                     )
                     SELECT tasks.id,tasks.permanent_number,tasks.title,tasks.department,tasks.contact,
                            annotated.result_status,annotated.first_handled_at,annotated.last_handled_at,annotated.handling_count,
                            tasks.task_type,annotated.has_processed_or_completed
                     FROM annotated JOIN tasks ON tasks.id=annotated.task_id
                     WHERE annotated.position=1 AND (?3 IS NULL OR tasks.task_type=?3)
                       AND (?4 IS NULL OR annotated.result_status=?4)
                     ORDER BY strftime('%s',annotated.last_handled_at) DESC,tasks.id DESC",
                )
                .map_err(display_error)?;
            let details = statement
                .query_map(params![start, end, task_type, result_status], |row| {
                    let departments = parse_contacts(&row.get::<_, String>(3)?).join("、");
                    let contacts = parse_contacts(&row.get::<_, String>(4)?).join("、");
                    Ok(StatisticsDetail {
                        task_id: row.get(0)?,
                        task_type: row.get(9)?,
                        has_processed_or_completed: row.get(10)?,
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

    pub fn report_items(
        &self,
        start: String,
        end: String,
        limit: i64,
        offset: i64,
    ) -> Result<ReportItemsPage, String> {
        let start_time = chrono::DateTime::parse_from_rfc3339(&start)
            .map_err(|_| "报告开始时间无效".to_string())?;
        let end_time = chrono::DateTime::parse_from_rfc3339(&end)
            .map_err(|_| "报告结束时间无效".to_string())?;
        if end_time <= start_time {
            return Err("报告结束时间必须晚于开始时间".into());
        }
        let limit = limit.clamp(1, 500);
        let offset = offset.max(0);
        self.with_conn(|connection| {
            let total = connection
                .query_row(
                    "SELECT count(DISTINCT event.task_id)
                     FROM task_work_events event
                     JOIN tasks ON tasks.id=event.task_id
                     WHERE event.voided_at IS NULL AND tasks.deleted_at IS NULL
                       AND strftime('%s',event.handled_at)>=strftime('%s',?1)
                       AND strftime('%s',event.handled_at)<strftime('%s',?2)",
                    params![&start, &end],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(display_error)?;
            let mut statement = connection
                .prepare(
                    "WITH ranged AS (
                       SELECT event.task_id,max(strftime('%s',event.handled_at)) AS last_handled
                       FROM task_work_events event
                       JOIN tasks ON tasks.id=event.task_id
                       WHERE event.voided_at IS NULL AND tasks.deleted_at IS NULL
                         AND strftime('%s',event.handled_at)>=strftime('%s',?1)
                         AND strftime('%s',event.handled_at)<strftime('%s',?2)
                       GROUP BY event.task_id
                     ), paged AS (
                       SELECT task_id,last_handled FROM ranged
                       ORDER BY last_handled DESC,task_id DESC LIMIT ?3 OFFSET ?4
                     )
                     SELECT tasks.id,tasks.permanent_number,tasks.title,tasks.department,
                            tasks.task_type,tasks.status,tasks.workload,tasks.completed_at,
                            event.result_status,event.handled_at,event.note
                     FROM paged
                     JOIN tasks ON tasks.id=paged.task_id
                     JOIN task_work_events event ON event.task_id=tasks.id
                     WHERE event.voided_at IS NULL
                       AND strftime('%s',event.handled_at)>=strftime('%s',?1)
                       AND strftime('%s',event.handled_at)<strftime('%s',?2)
                     ORDER BY paged.last_handled DESC,tasks.id DESC,
                              strftime('%s',event.handled_at),event.id",
                )
                .map_err(display_error)?;
            let rows = statement
                .query_map(params![&start, &end, limit, offset], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                    ))
                })
                .map_err(display_error)?;
            let mut items: Vec<ReportItem> = Vec::new();
            for row in rows {
                let (
                    task_id,
                    permanent_number,
                    title,
                    department,
                    task_type,
                    current_status,
                    workload,
                    completed_at,
                    result_status,
                    handled_at,
                    note,
                ) = row.map_err(display_error)?;
                if items.last().is_none_or(|item| item.task_id != task_id) {
                    items.push(ReportItem {
                        task_id,
                        permanent_number,
                        title,
                        departments: parse_contacts(&department),
                        task_type,
                        current_status,
                        workload,
                        completed_at,
                        work_events: Vec::new(),
                    });
                }
                items
                    .last_mut()
                    .expect("刚加入的报告事项应存在")
                    .work_events
                    .push(ReportWorkEvent {
                        result_status,
                        handled_at,
                        note,
                    });
            }
            Ok(ReportItemsPage {
                total,
                offset,
                limit,
                has_more: offset + (items.len() as i64) < total,
                items,
            })
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
fn record_urgent_values(
    connection: &Connection,
    id: i64,
    requester: &str,
    reason: &str,
    requested_deadline: Option<&str>,
) -> Result<(), String> {
    connection.execute("INSERT INTO urgent_records(task_id,requester,reason,requested_deadline,requested_at,confirmation_status,confirmed_at)
            VALUES(?,?,?,?,?,'confirmed',?)",params![id,requester,reason,requested_deadline,now(),now()]).map_err(display_error)?;
    add_log(connection, id, "urgent", &format!("标记加急：{requester}"))
}

fn record_urgent(connection: &Connection, id: i64, input: &TaskInput) -> Result<(), String> {
    record_urgent_values(
        connection,
        id,
        input.urgent_requester.trim(),
        input.urgent_reason.trim(),
        input.requested_deadline.as_deref(),
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
        if !valid_setting(&key, &value) {
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
        self.activate_due_scheduled()?;
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
            .filter(|path| {
                path.extension()
                    .is_some_and(|value| value.eq_ignore_ascii_case("db"))
            })
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
        let path = self.unique_backup_path(safe);
        self.with_conn(|connection| Self::backup_connection(connection, &path))?;
        backup_info(&path)
    }

    fn unique_backup_path(&self, kind: &str) -> PathBuf {
        let initial = self.backup_dir.join(Self::backup_name(kind));
        if !initial.exists() {
            return initial;
        }
        let stem = initial
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let mut suffix = 2;
        loop {
            let candidate = self.backup_dir.join(format!("{stem}-{suffix}.db"));
            if !candidate.exists() {
                return candidate;
            }
            suffix += 1;
        }
    }

    fn validate_backup_file(path: &Path) -> Result<(), String> {
        if !path.is_file()
            || path
                .extension()
                .is_none_or(|value| !value.eq_ignore_ascii_case("db"))
        {
            return Err("请选择 .db 格式的 In Line 备份".into());
        }
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| "备份文件无法打开".to_string())?;
        let integrity: String = connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(display_error)?;
        if integrity != "ok" {
            return Err("备份文件校验失败，当前数据未改变".into());
        }
        for table in ["tasks", "settings"] {
            let exists: i64 = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
                    [table],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            if exists == 0 {
                return Err("所选文件不是有效的 In Line 备份".into());
            }
        }
        let version = Self::schema_version(&connection)?;
        if version > 9 {
            return Err("该备份来自更高版本的 In Line，请先升级软件".into());
        }
        Ok(())
    }

    pub fn import_backup(&self, raw_path: String) -> Result<BackupInfo, String> {
        let source = fs::canonicalize(raw_path).map_err(|_| "找不到所选备份".to_string())?;
        Self::validate_backup_file(&source)?;
        let target = self.unique_backup_path("import");
        let copy_result = (|| -> Result<(), String> {
            let mut input = fs::File::open(&source).map_err(display_error)?;
            let mut output = fs::File::create(&target).map_err(display_error)?;
            io::copy(&mut input, &mut output).map_err(display_error)?;
            output.sync_all().map_err(display_error)
        })();
        if let Err(error) = copy_result {
            let _ = fs::remove_file(&target);
            return Err(error);
        }
        if let Err(error) = Self::validate_backup_file(&target) {
            let _ = fs::remove_file(&target);
            return Err(error);
        }
        backup_info(&target)
    }
    pub fn backup_directory(&self) -> PathBuf {
        self.backup_dir.clone()
    }
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>, String> {
        let mut values = fs::read_dir(&self.backup_dir)
            .map_err(display_error)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|value| value.eq_ignore_ascii_case("db"))
            })
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

    pub fn restore_backup(&self, raw_path: String) -> Result<BackupMergeResult, String> {
        let selected = fs::canonicalize(&raw_path).map_err(|_| "找不到所选备份".to_string())?;
        let backup_root = fs::canonicalize(&self.backup_dir).map_err(display_error)?;
        if !selected.starts_with(&backup_root)
            || selected
                .extension()
                .is_none_or(|value| !value.eq_ignore_ascii_case("db"))
        {
            return Err("只能恢复 In Line 备份目录中的数据库文件".into());
        }
        Self::validate_backup_file(&selected)?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(display_error)?
            .as_nanos();
        let staged = self.backup_dir.join(format!(".merge-source-{nonce}.tmp"));
        fs::copy(&selected, &staged).map_err(display_error)?;
        let merge_result = (|| -> Result<BackupMergeResult, String> {
            let mut source = Self::connect(&staged)?;
            Self::migrate(&mut source)?;
            let source_tasks = load_all_tasks(&source)?;
            let mut source_relations = source_tasks
                .iter()
                .filter_map(|task| {
                    task.parent_task_id
                        .map(|parent_task_id| (task.id, parent_task_id, task.subtask_sort_order))
                })
                .collect::<Vec<_>>();
            source_relations.sort_by_key(|(task_id, parent_task_id, order)| {
                (*parent_task_id, *order, *task_id)
            });

            let emergency = self.unique_backup_path("before-restore");
            self.with_conn(|connection| Self::backup_connection(connection, &emergency))?;

            let mut guard = self
                .connection
                .lock()
                .map_err(|_| "数据库正忙，请稍后重试".to_string())?;
            let connection = guard.as_mut().ok_or("数据库尚未打开")?;
            let transaction = connection.transaction().map_err(display_error)?;
            let mut result = BackupMergeResult {
                added_tasks: 0,
                merged_tasks: 0,
                conflict_tasks: 0,
                applied_settings: 0,
                conflicts: Vec::new(),
            };
            let mut imported_task_ids = HashMap::new();
            let mut inserted_task_ids = HashSet::new();
            merge_sequence_watermarks_on(&source, &transaction)?;

            for mut source_task in source_tasks {
                source_task.task_type = canonical_task_type(&transaction, &source_task.task_type)?;
                let same_title = tasks_with_title(&transaction, &source_task.title)?;
                let exact = same_title
                    .iter()
                    .find(|current| same_task_content(current, &source_task));
                let conflict = exact.is_none() && !same_title.is_empty();
                let (target_id, inserted) = if let Some(current) = exact {
                    result.merged_tasks += 1;
                    (current.id, false)
                } else {
                    let title = if conflict {
                        result.conflict_tasks += 1;
                        conflict_title(&transaction, &source_task.title)?
                    } else {
                        source_task.title.clone()
                    };
                    let id = insert_imported_task(
                        &transaction,
                        &source_task,
                        &title,
                        conflict || source_task.is_import_conflict,
                    )?;
                    result.added_tasks += 1;
                    (id, true)
                };
                if conflict {
                    let imported = get_task_on(&transaction, target_id)?;
                    result.conflicts.push(BackupConflictItem {
                        task_id: imported.id,
                        permanent_number: imported.permanent_number,
                        source_title: source_task.title.clone(),
                        imported_title: imported.title,
                    });
                }
                imported_task_ids.insert(source_task.id, target_id);
                if inserted {
                    inserted_task_ids.insert(target_id);
                }

                for (table, columns) in [
                    ("task_logs", &["log_type", "content", "created_at"][..]),
                    (
                        "status_history",
                        &["old_status", "new_status", "reason", "created_at"][..],
                    ),
                    (
                        "urgent_records",
                        &[
                            "requester",
                            "reason",
                            "requested_deadline",
                            "requested_at",
                            "confirmation_status",
                            "confirmed_at",
                            "cancelled_at",
                            "notes",
                        ][..],
                    ),
                    (
                        "task_work_events",
                        &[
                            "result_status",
                            "handled_at",
                            "task_type_snapshot",
                            "source",
                            "note",
                            "created_at",
                            "updated_at",
                            "voided_at",
                        ][..],
                    ),
                ] {
                    copy_unique_task_rows(
                        &source,
                        &transaction,
                        table,
                        columns,
                        source_task.id,
                        target_id,
                    )?;
                }
                copy_queue_entries(&source, &transaction, source_task.id, target_id, inserted)?;
            }

            for (source_task_id, source_parent_id, _) in source_relations {
                let (Some(&target_task_id), Some(&target_parent_id)) = (
                    imported_task_ids.get(&source_task_id),
                    imported_task_ids.get(&source_parent_id),
                ) else {
                    continue;
                };
                let current_parent: Option<i64> = transaction
                    .query_row(
                        "SELECT parent_task_id FROM tasks WHERE id=?",
                        [target_task_id],
                        |row| row.get(0),
                    )
                    .map_err(display_error)?;
                if current_parent.is_some()
                    || validate_parent_assignment(&transaction, target_task_id, target_parent_id)
                        .is_err()
                {
                    continue;
                }
                let next_order: i64 = transaction
                    .query_row(
                        "SELECT COALESCE(MAX(subtask_sort_order),0)+1
                         FROM tasks WHERE parent_task_id=?",
                        [target_parent_id],
                        |row| row.get(0),
                    )
                    .map_err(display_error)?;
                transaction
                    .execute(
                        "UPDATE tasks SET parent_task_id=?,subtask_sort_order=? WHERE id=?",
                        params![target_parent_id, next_order, target_task_id],
                    )
                    .map_err(display_error)?;
            }

            merge_number_history_on(
                &source,
                &transaction,
                &imported_task_ids,
                &inserted_task_ids,
            )?;
            merge_master_values(&source, &transaction)?;
            deduplicate_task_types(&transaction)?;
            result.applied_settings = merge_settings(&source, &transaction)?;
            transaction.commit().map_err(display_error)?;
            Ok(result)
        })();
        let _ = fs::remove_file(&staged);
        for suffix in ["-wal", "-shm"] {
            let mut sidecar = staged.as_os_str().to_os_string();
            sidecar.push(suffix);
            let _ = fs::remove_file(PathBuf::from(sidecar));
        }
        merge_result
    }
}

fn load_all_tasks(connection: &Connection) -> Result<Vec<LegalTask>, String> {
    let ids = {
        let mut statement = connection
            .prepare("SELECT id FROM tasks ORDER BY id")
            .map_err(display_error)?;
        let values = statement
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(display_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(display_error)?;
        values
    };
    ids.into_iter()
        .map(|id| get_task_on(connection, id))
        .collect()
}

fn tasks_with_title(connection: &Connection, title: &str) -> Result<Vec<LegalTask>, String> {
    let mut statement = connection
        .prepare(&format!("{SELECT_TASK} WHERE title=? ORDER BY id"))
        .map_err(display_error)?;
    let values = statement
        .query_map([title], Database::row_task)
        .map_err(display_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(display_error)?;
    Ok(values)
}

fn same_task_content(left: &LegalTask, right: &LegalTask) -> bool {
    left.department == right.department
        && left.contact == right.contact
        && left.task_type == right.task_type
        && left.title == right.title
        && left.details == right.details
        && left.status == right.status
        && left.priority == right.priority
        && left.workload == right.workload
        && left.is_urgent == right.is_urgent
        && left.urgent_requester == right.urgent_requester
        && left.urgent_reason == right.urgent_reason
        && left.requested_deadline == right.requested_deadline
        && left.requested_deadline_label == right.requested_deadline_label
        && left.internal_notes == right.internal_notes
        && left.started_at == right.started_at
        && left.completed_at == right.completed_at
        && left.archived_at == right.archived_at
        && left.deleted_at == right.deleted_at
        && left.planned_date == right.planned_date
        && left.is_scheduled == right.is_scheduled
}

fn canonical_task_type(connection: &Connection, source: &str) -> Result<String, String> {
    let normalized = source.trim();
    let names = {
        let mut statement = connection
            .prepare("SELECT name FROM master_values WHERE kind='task_type' ORDER BY id")
            .map_err(display_error)?;
        let values = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(display_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(display_error)?;
        values
    };
    Ok(names
        .into_iter()
        .find(|name| name.trim().to_lowercase() == normalized.to_lowercase())
        .map(|name| name.trim().to_string())
        .unwrap_or_else(|| normalized.to_string()))
}

fn conflict_title(connection: &Connection, source: &str) -> Result<String, String> {
    for index in 1.. {
        let candidate = if index == 1 {
            format!("{source}（冲突）")
        } else {
            format!("{source}（冲突 {index}）")
        };
        let exists: i64 = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tasks WHERE title=?)",
                [&candidate],
                |row| row.get(0),
            )
            .map_err(display_error)?;
        if exists == 0 {
            return Ok(candidate);
        }
    }
    unreachable!()
}

fn reserve_daily_sequence(
    connection: &Connection,
    date: &str,
    sequence: i64,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO daily_sequences(ticket_date,last_sequence) VALUES(?,?)
             ON CONFLICT(ticket_date) DO UPDATE SET last_sequence=max(last_sequence,excluded.last_sequence)",
            params![date, sequence],
        )
        .map_err(display_error)?;
    Ok(())
}

fn next_import_sequence(connection: &Connection, date: &str) -> Result<i64, String> {
    let current: i64 = connection
        .query_row(
            "SELECT max(value) FROM (
               SELECT COALESCE(MAX(last_sequence),0) AS value FROM daily_sequences WHERE ticket_date=?
               UNION ALL SELECT COALESCE(MAX(daily_sequence),0) FROM tasks WHERE ticket_date=?
               UNION ALL SELECT COALESCE(MAX(daily_sequence),0) FROM task_queue_entries WHERE queue_date=?
               UNION ALL SELECT COALESCE(MAX(daily_sequence),0) FROM queue_number_allocations WHERE queue_date=?
             )",
            params![date, date, date,date],
            |row| row.get(0),
        )
        .map_err(display_error)?;
    let next = current + 1;
    reserve_daily_sequence(connection, date, next)?;
    Ok(next)
}

fn imported_identity(
    connection: &Connection,
    task: &LegalTask,
) -> Result<(String, i64, String), String> {
    let pair_used: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE ticket_date=?1 AND daily_sequence=?2 UNION ALL SELECT 1 FROM queue_number_allocations WHERE queue_date=?1 AND daily_sequence=?2)",
            params![task.ticket_date, task.daily_sequence],
            |row| row.get(0),
        )
        .map_err(display_error)?;
    let number_used: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE permanent_number=?)",
            [&task.permanent_number],
            |row| row.get(0),
        )
        .map_err(display_error)?;
    if pair_used == 0 && number_used == 0 {
        reserve_daily_sequence(connection, &task.ticket_date, task.daily_sequence)?;
        return Ok((
            task.ticket_date.clone(),
            task.daily_sequence,
            task.permanent_number.clone(),
        ));
    }
    let mut sequence = next_import_sequence(connection, &task.ticket_date)?;
    loop {
        let permanent = format!("{}-{sequence:02}", task.ticket_date.replace('-', ""));
        let exists: i64 = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tasks WHERE permanent_number=?)",
                [&permanent],
                |row| row.get(0),
            )
            .map_err(display_error)?;
        if exists == 0 {
            return Ok((task.ticket_date.clone(), sequence, permanent));
        }
        sequence = next_import_sequence(connection, &task.ticket_date)?;
    }
}

fn insert_imported_task(
    connection: &Connection,
    task: &LegalTask,
    title: &str,
    is_import_conflict: bool,
) -> Result<i64, String> {
    let (ticket_date, daily_sequence, permanent_number) = imported_identity(connection, task)?;
    let order: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(custom_sort_order),0)+1 FROM tasks",
            [],
            |row| row.get(0),
        )
        .map_err(display_error)?;
    let departments = contact_storage(&task.departments)?;
    let contacts = contact_storage(&task.contacts)?;
    connection
        .execute(
            "INSERT INTO tasks(
               permanent_number,daily_sequence,ticket_date,department,contact,task_type,title,details,
               status,priority,workload,is_urgent,urgent_requester,urgent_reason,requested_deadline,
               internal_notes,created_at,updated_at,started_at,completed_at,archived_at,deleted_at,
               custom_sort_order,requested_deadline_label,is_import_conflict
             ) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            params![
                permanent_number,
                daily_sequence,
                ticket_date,
                departments,
                contacts,
                task.task_type,
                title,
                task.details,
                task.status,
                task.priority,
                task.workload,
                task.is_urgent as i64,
                task.urgent_requester,
                task.urgent_reason,
                task.requested_deadline,
                task.internal_notes,
                task.created_at,
                task.updated_at,
                task.started_at,
                task.completed_at,
                task.archived_at,
                task.deleted_at,
                order,
                task.requested_deadline_label,
                is_import_conflict as i64,
            ],
        )
        .map_err(display_error)?;
    let id = connection.last_insert_rowid();
    connection.execute("UPDATE tasks SET planned_date=?,is_scheduled=?,schedule_action=?,schedule_action_at=? WHERE id=?",params![task.planned_date,task.is_scheduled as i64,task.schedule_action,task.schedule_action_at,id]).map_err(display_error)?;
    allocate_number_on(connection, id, &ticket_date, daily_sequence)?;
    if daily_sequence != task.daily_sequence {
        add_log(
            connection,
            id,
            "backup_number_remapped",
            &format!(
                "备份原队列 {}-{:02} 已被占用；保留本地记录，导入队列改为 {}-{:02}，原固定编号 {}",
                task.ticket_date,
                task.daily_sequence,
                ticket_date,
                daily_sequence,
                task.permanent_number
            ),
        )?;
    }
    Ok(id)
}

fn copy_unique_task_rows(
    source: &Connection,
    target: &Connection,
    table: &str,
    columns: &[&str],
    source_task_id: i64,
    target_task_id: i64,
) -> Result<(), String> {
    let rows = {
        let mut statement = source
            .prepare(&format!(
                "SELECT {} FROM {table} WHERE task_id=? ORDER BY id",
                columns.join(",")
            ))
            .map_err(display_error)?;
        let values = statement
            .query_map([source_task_id], |row| {
                (0..columns.len())
                    .map(|index| row.get::<_, Value>(index))
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(display_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(display_error)?;
        values
    };
    let predicates = columns
        .iter()
        .map(|column| format!("{column} IS ?"))
        .collect::<Vec<_>>()
        .join(" AND ");
    let placeholders = std::iter::repeat_n("?", columns.len() + 1)
        .collect::<Vec<_>>()
        .join(",");
    for row in rows {
        let mut values = vec![Value::Integer(target_task_id)];
        values.extend(row);
        let exists: i64 = target
            .query_row(
                &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE task_id=? AND {predicates})"),
                params_from_iter(values.iter()),
                |result| result.get(0),
            )
            .map_err(display_error)?;
        if exists == 0 {
            target
                .execute(
                    &format!(
                        "INSERT INTO {table}(task_id,{}) VALUES({placeholders})",
                        columns.join(",")
                    ),
                    params_from_iter(values.iter()),
                )
                .map_err(display_error)?;
        }
    }
    Ok(())
}

#[derive(Debug)]
struct ImportedQueueEntry {
    queue_date: String,
    daily_sequence: i64,
    requested_deadline: Option<String>,
    requested_deadline_label: Option<String>,
    enqueued_at: String,
    closed_at: Option<String>,
    close_reason: String,
    created_at: String,
    updated_at: String,
}

fn copy_queue_entries(
    source: &Connection,
    target: &Connection,
    source_task_id: i64,
    target_task_id: i64,
    inserted_task: bool,
) -> Result<(), String> {
    let entries = {
        let mut statement = source
            .prepare(
                "SELECT queue_date,daily_sequence,requested_deadline,requested_deadline_label,
                        enqueued_at,closed_at,close_reason,created_at,updated_at
                 FROM task_queue_entries WHERE task_id=? ORDER BY id",
            )
            .map_err(display_error)?;
        let values = statement
            .query_map([source_task_id], |row| {
                Ok(ImportedQueueEntry {
                    queue_date: row.get(0)?,
                    daily_sequence: row.get(1)?,
                    requested_deadline: row.get(2)?,
                    requested_deadline_label: row.get(3)?,
                    enqueued_at: row.get(4)?,
                    closed_at: row.get(5)?,
                    close_reason: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })
            .map_err(display_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(display_error)?;
        values
    };
    for entry in entries {
        let duplicate: i64 = target
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM task_queue_entries
                 WHERE task_id=? AND queue_date=? AND requested_deadline IS ?
                   AND requested_deadline_label IS ? AND enqueued_at=? AND closed_at IS ?
                   AND close_reason=? AND created_at=? AND updated_at=?)",
                params![
                    target_task_id,
                    entry.queue_date,
                    entry.requested_deadline,
                    entry.requested_deadline_label,
                    entry.enqueued_at,
                    entry.closed_at,
                    entry.close_reason,
                    entry.created_at,
                    entry.updated_at,
                ],
                |row| row.get(0),
            )
            .map_err(display_error)?;
        if duplicate != 0 {
            continue;
        }
        if entry.closed_at.is_none() {
            let active: i64 = target
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM task_queue_entries WHERE task_id=? AND closed_at IS NULL)",
                    [target_task_id],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            if active != 0 {
                continue;
            }
        }
        let pair_used: i64 = target
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM task_queue_entries WHERE queue_date=?1 AND daily_sequence=?2 UNION ALL SELECT 1 FROM queue_number_allocations WHERE queue_date=?1 AND daily_sequence=?2 AND (task_id IS NOT ?3 OR voided_at IS NOT NULL))",
                params![entry.queue_date, entry.daily_sequence,target_task_id],
                |row| row.get(0),
            )
            .map_err(display_error)?;
        let sequence = if pair_used == 0 {
            reserve_daily_sequence(target, &entry.queue_date, entry.daily_sequence)?;
            entry.daily_sequence
        } else {
            next_import_sequence(target, &entry.queue_date)?
        };
        target
            .execute(
                "INSERT INTO task_queue_entries(
                   task_id,queue_date,daily_sequence,requested_deadline,requested_deadline_label,
                   enqueued_at,closed_at,close_reason,created_at,updated_at
                 ) VALUES(?,?,?,?,?,?,?,?,?,?)",
                params![
                    target_task_id,
                    entry.queue_date,
                    sequence,
                    entry.requested_deadline,
                    entry.requested_deadline_label,
                    entry.enqueued_at,
                    entry.closed_at,
                    entry.close_reason,
                    entry.created_at,
                    entry.updated_at,
                ],
            )
            .map_err(display_error)?;
        target.execute("INSERT OR IGNORE INTO queue_number_allocations(task_id,permanent_number,queue_date,daily_sequence,allocated_at,activated_at,voided_at,void_reason) SELECT id,permanent_number,?,?,?, ?,?,? FROM tasks WHERE id=?",params![entry.queue_date,sequence,entry.created_at,entry.enqueued_at,entry.closed_at,if entry.closed_at.is_some(){"导入历史队列"}else{""},target_task_id]).map_err(display_error)?;
        target.execute("UPDATE queue_number_allocations SET activated_at=? WHERE task_id=? AND queue_date=? AND daily_sequence=?",params![entry.enqueued_at,target_task_id,entry.queue_date,sequence]).map_err(display_error)?;
        if inserted_task && entry.closed_at.is_none() {
            target
                .execute(
                    "UPDATE tasks SET ticket_date=?,daily_sequence=?,requested_deadline=?,requested_deadline_label=? WHERE id=?",
                    params![
                        entry.queue_date,
                        sequence,
                        entry.requested_deadline,
                        entry.requested_deadline_label,
                        target_task_id,
                    ],
                )
                .map_err(display_error)?;
        }
    }
    Ok(())
}

fn merge_master_values(source: &Connection, target: &Connection) -> Result<(), String> {
    let values = {
        let mut statement = source
            .prepare(
                "SELECT kind,name,sort_order,is_active,usage_count,manual_order FROM master_values ORDER BY id",
            )
            .map_err(display_error)?;
        let values = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                ))
            })
            .map_err(display_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(display_error)?;
        values
    };
    for (kind, name, sort_order, is_active, usage_count, manual_order) in values {
        target
            .execute(
                "INSERT INTO master_values(kind,name,sort_order,is_active,usage_count,manual_order)
                 VALUES(?,?,?,?,?,?) ON CONFLICT(kind,name) DO UPDATE SET
                   is_active=max(is_active,excluded.is_active),
                   usage_count=max(usage_count,excluded.usage_count),
                   manual_order=COALESCE(master_values.manual_order,excluded.manual_order)",
                params![kind, name, sort_order, is_active, usage_count, manual_order],
            )
            .map_err(display_error)?;
    }
    Ok(())
}

fn deduplicate_task_types(connection: &Connection) -> Result<(), String> {
    let task_types = {
        let mut statement = connection
            .prepare("SELECT DISTINCT task_type FROM tasks WHERE trim(task_type)<>''")
            .map_err(display_error)?;
        let values = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(display_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(display_error)?;
        values
    };
    for name in task_types {
        connection
            .execute(
                "INSERT OR IGNORE INTO master_values(kind,name,sort_order,is_active) VALUES('task_type',?,999,1)",
                [name],
            )
            .map_err(display_error)?;
    }

    let rows = {
        let mut statement = connection
            .prepare(
                "SELECT id,name,sort_order,is_active,usage_count,manual_order
                 FROM master_values WHERE kind='task_type' ORDER BY id",
            )
            .map_err(display_error)?;
        let values = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                ))
            })
            .map_err(display_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(display_error)?;
        values
    };
    let mut groups: BTreeMap<String, Vec<(i64, String, i64, i64, i64, Option<i64>)>> =
        BTreeMap::new();
    for row in rows {
        groups
            .entry(row.1.trim().to_lowercase())
            .or_default()
            .push(row);
    }
    for group in groups.into_values() {
        let canonical_name = group[0].1.trim().to_string();
        let keep = group
            .iter()
            .find(|row| row.1 == canonical_name)
            .unwrap_or(&group[0]);
        let active = group.iter().map(|row| row.3).max().unwrap_or(1);
        let sort_order = group.iter().map(|row| row.2).min().unwrap_or(999);
        let manual_order = group.iter().filter_map(|row| row.5).min();
        for row in &group {
            connection
                .execute(
                    "UPDATE tasks SET task_type=? WHERE task_type=?",
                    params![canonical_name, row.1],
                )
                .map_err(display_error)?;
            connection
                .execute(
                    "UPDATE task_work_events SET task_type_snapshot=? WHERE task_type_snapshot=?",
                    params![canonical_name, row.1],
                )
                .map_err(display_error)?;
            if row.0 != keep.0 {
                connection
                    .execute("DELETE FROM master_values WHERE id=?", [row.0])
                    .map_err(display_error)?;
            }
        }
        let usage_count: i64 = connection
            .query_row(
                "SELECT count(*) FROM tasks WHERE task_type=?",
                [&canonical_name],
                |row| row.get(0),
            )
            .map_err(display_error)?;
        connection
            .execute(
                "UPDATE master_values SET name=?,sort_order=?,is_active=?,usage_count=?,manual_order=? WHERE id=?",
                params![
                    canonical_name,
                    sort_order,
                    active,
                    usage_count,
                    manual_order,
                    keep.0
                ],
            )
            .map_err(display_error)?;
    }
    Ok(())
}

fn merge_settings(source: &Connection, target: &Connection) -> Result<usize, String> {
    let settings = {
        let mut statement = source
            .prepare("SELECT key,value FROM settings ORDER BY key")
            .map_err(display_error)?;
        let values = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(display_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(display_error)?;
        values
    };
    let mut applied = 0;
    for (key, value) in settings {
        if !valid_setting(&key, &value) {
            continue;
        }
        target
            .execute(
                "INSERT INTO settings(key,value) VALUES(?,?)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![key, value],
            )
            .map_err(display_error)?;
        applied += 1;
    }
    Ok(applied)
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

    #[test]
    fn ticket_colors_persist_across_reopen_and_invalid_saves_preserve_all_roles() {
        let root = std::env::temp_dir().join(format!(
            "inline-colors-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let db = Database::open_root(root.clone()).unwrap();
        assert!(!db.settings().unwrap().contains_key("ticket_colors"));
        let colors = serde_json::json!({"normal":"#0B3A82","future":"#F3D98B","urgent":"#A7446A"})
            .to_string();
        db.set_setting("ticket_colors".into(), colors.clone())
            .unwrap();
        db.set_setting("week_start_day".into(), "sunday".into())
            .unwrap();
        for invalid in [
            "null".into(), "{}".into(), "not-json".into(),
            serde_json::json!({"normal":"#fff","future":"#F3D98B","urgent":"#A7446A"}).to_string(),
            serde_json::json!({"normal":"#0B3A82","future":"url(x)","urgent":"#A7446A"}).to_string(),
            serde_json::json!({"normal":"#0B3A82","future":123,"urgent":"#A7446A"}).to_string(),
            serde_json::json!({"normal":"#0B3A82","future":"#F3D98B","urgent":"#A7446A","extra":"#FFFFFF"}).to_string(),
            format!("{colors}{}", " ".repeat(256)),
        ] {
            assert!(db.set_setting("ticket_colors".into(), invalid).is_err());
            assert_eq!(db.settings().unwrap().get("ticket_colors"), Some(&colors));
        }
        drop(db);
        let db = Database::open_root(root.clone()).unwrap();
        assert_eq!(db.settings().unwrap().get("ticket_colors"), Some(&colors));
        let defaults =
            serde_json::json!({"normal":"#0B3A82","future":"#3F766E","urgent":"#C43D4B"})
                .to_string();
        db.set_setting("ticket_colors".into(), defaults.clone())
            .unwrap();
        assert_eq!(db.settings().unwrap().get("ticket_colors"), Some(&defaults));
        assert_eq!(
            db.settings()
                .unwrap()
                .get("week_start_day")
                .map(String::as_str),
            Some("sunday")
        );
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn backup_restores_valid_ticket_colors_and_skips_corrupt_color_settings() {
        let root = std::env::temp_dir().join(format!(
            "inline-color-backup-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let source = Database::open_root(root.join("source")).unwrap();
        source.save_task(sample("配色备份")).unwrap();
        let colors = serde_json::json!({"normal":"#68717D","future":"#F3D98B","urgent":"#9B4055"})
            .to_string();
        source
            .set_setting("ticket_colors".into(), colors.clone())
            .unwrap();
        let backup = source.create_backup("manual").unwrap();
        let target = Database::open_root(root.join("target")).unwrap();
        let imported = target.import_backup(backup.path).unwrap();
        target.restore_backup(imported.path.clone()).unwrap();
        assert_eq!(
            target.settings().unwrap().get("ticket_colors"),
            Some(&colors)
        );
        let local = serde_json::json!({"normal":"#536C8F","future":"#3F766E","urgent":"#C43D4B"})
            .to_string();
        target
            .set_setting("ticket_colors".into(), local.clone())
            .unwrap();
        let corrupt_backup = Connection::open(&imported.path).unwrap();
        corrupt_backup
            .execute(
                "UPDATE settings SET value='invalid' WHERE key='ticket_colors'",
                [],
            )
            .unwrap();
        drop(corrupt_backup);
        target.restore_backup(imported.path).unwrap();
        assert_eq!(
            target.settings().unwrap().get("ticket_colors"),
            Some(&local)
        );
        drop(source);
        drop(target);
        fs::remove_dir_all(root).unwrap();
    }

    fn create_v7_database(path: &Path) {
        let connection = Connection::open(path).unwrap();
        connection
            .execute_batch(
                "PRAGMA foreign_keys=ON;
                 CREATE TABLE schema_meta(version INTEGER NOT NULL);
                 INSERT INTO schema_meta(version) VALUES(7);
                 CREATE TABLE daily_sequences(ticket_date TEXT PRIMARY KEY,last_sequence INTEGER NOT NULL);
                 CREATE TABLE tasks(
                   id INTEGER PRIMARY KEY AUTOINCREMENT, permanent_number TEXT NOT NULL UNIQUE,
                   daily_sequence INTEGER NOT NULL, ticket_date TEXT NOT NULL, department TEXT NOT NULL,
                   contact TEXT NOT NULL, task_type TEXT NOT NULL, title TEXT NOT NULL, details TEXT NOT NULL,
                   status TEXT NOT NULL DEFAULT 'pending', priority TEXT NOT NULL DEFAULT 'normal',
                   workload TEXT NOT NULL DEFAULT 'standard', is_urgent INTEGER NOT NULL DEFAULT 0,
                   urgent_requester TEXT NOT NULL DEFAULT '', urgent_reason TEXT NOT NULL DEFAULT '',
                   requested_deadline TEXT, internal_notes TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL,
                   updated_at TEXT NOT NULL, started_at TEXT, completed_at TEXT, archived_at TEXT,
                   deleted_at TEXT, custom_sort_order INTEGER NOT NULL DEFAULT 0,
                   requested_deadline_label TEXT, is_import_conflict INTEGER NOT NULL DEFAULT 0,
                   UNIQUE(ticket_date,daily_sequence));
                 CREATE TABLE task_queue_entries(
                   id INTEGER PRIMARY KEY AUTOINCREMENT, task_id INTEGER NOT NULL,
                   queue_date TEXT NOT NULL,daily_sequence INTEGER NOT NULL,requested_deadline TEXT,
                   requested_deadline_label TEXT,enqueued_at TEXT NOT NULL,closed_at TEXT,
                   close_reason TEXT NOT NULL DEFAULT '',created_at TEXT NOT NULL,updated_at TEXT NOT NULL,
                   FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE,
                   UNIQUE(queue_date,daily_sequence));
                 CREATE TABLE task_work_events(
                   id INTEGER PRIMARY KEY AUTOINCREMENT,task_id INTEGER NOT NULL,result_status TEXT NOT NULL,
                   handled_at TEXT NOT NULL,task_type_snapshot TEXT NOT NULL,source TEXT NOT NULL,
                   note TEXT NOT NULL DEFAULT '',created_at TEXT NOT NULL,updated_at TEXT NOT NULL,voided_at TEXT,
                   FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE);
                 INSERT INTO tasks(
                   permanent_number,daily_sequence,ticket_date,department,contact,task_type,title,details,
                   status,priority,workload,created_at,updated_at,custom_sort_order,requested_deadline_label,
                   is_import_conflict
                 ) VALUES(
                   '20260927-01',1,'2026-09-27','产品组','小林','任务处理','v7 事项','旧数据',
                   'pending','normal','standard','2026-09-27T09:00:00+08:00',
                   '2026-09-27T09:00:00+08:00',1,NULL,0
                 );
                 INSERT INTO task_queue_entries(
                   task_id,queue_date,daily_sequence,enqueued_at,created_at,updated_at
                 ) VALUES(1,'2026-09-27',1,'2026-09-27T09:00:00+08:00',
                   '2026-09-27T09:00:00+08:00','2026-09-27T09:00:00+08:00');",
            )
            .unwrap();
    }

    fn sample(title: &str) -> TaskInput {
        TaskInput {
            planned_date: None,
            confirm_schedule_change: false,
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

    struct TestClock;
    impl TestClock {
        fn at(value: &str) -> Self {
            TEST_TIME.with(|clock| {
                *clock.borrow_mut() = Some(chrono::DateTime::parse_from_rfc3339(value).unwrap())
            });
            Self
        }
    }
    impl Drop for TestClock {
        fn drop(&mut self) {
            TEST_TIME.with(|clock| *clock.borrow_mut() = None);
        }
    }

    #[test]
    fn planned_creation_edit_confirmation_deadline_and_number_reservation() {
        let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
        let root = std::env::temp_dir().join(format!(
            "inline-plan-edit-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        let mut input = sample("提前取号");
        input.planned_date = Some("2026-10-05".into());
        let first = db.save_task(input.clone()).unwrap();
        let second = db.save_task(input.clone()).unwrap();
        assert!(first.is_scheduled && !first.has_active_queue);
        assert_eq!(first.ticket_date, "2026-10-05");
        assert_eq!(first.daily_sequence, 1);
        assert_eq!(second.daily_sequence, 2);
        assert!(first.permanent_number.starts_with("20261002-"));
        input.id = Some(first.id);
        input.planned_date = Some("2026-10-08".into());
        assert!(db.save_task(input.clone()).unwrap_err().contains("确认"));
        input.confirm_schedule_change = true;
        let changed = db.save_task(input.clone()).unwrap();
        assert_eq!(changed.permanent_number, first.permanent_number);
        assert_eq!(changed.ticket_date, "2026-10-08");
        input.id = None;
        input.planned_date = Some("2026-10-05".into());
        assert_eq!(db.save_task(input.clone()).unwrap().daily_sequence, 3);
        input.planned_date = Some("2026-10-01".into());
        assert!(db.save_task(input.clone()).is_err());
        input.planned_date = Some("2026-10-08".into());
        input.requested_deadline = Some("2026-10-07T12:00:00+08:00".into());
        assert!(db.save_task(input.clone()).is_err());
        input.id = Some(first.id);
        input.planned_date = Some("2026-10-02".into());
        input.requested_deadline = None;
        let early = db.save_task(input).unwrap();
        assert!(early.has_active_queue && !early.is_scheduled);
        assert_eq!(early.permanent_number, first.permanent_number);
        db.with_conn(|connection| {
            let count:i64=connection.query_row("SELECT count(*) FROM queue_number_allocations WHERE task_id=? AND voided_at IS NOT NULL",[first.id],|row|row.get(0)).map_err(display_error)?;
            assert_eq!(count,2);Ok(())
        }).unwrap();
    }

    #[test]
    fn due_scheduled_is_idempotent_keeps_reserved_order_and_skips_non_pending() {
        let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
        let root = std::env::temp_dir().join(format!(
            "inline-plan-due-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        let mut input = sample("计划入队");
        input.planned_date = Some("2026-10-05".into());
        let first = db.save_task(input.clone()).unwrap();
        input.is_urgent = true;
        input.urgent_requester = "负责人".into();
        input.urgent_reason = "加急计划".into();
        let second = db.save_task(input.clone()).unwrap();
        input.is_urgent = false;
        let paused = db.save_task(input.clone()).unwrap();
        let terminal = db.save_task(input).unwrap();
        db.set_status(paused.id, "paused".into()).unwrap();
        db.archive(terminal.id).unwrap();
        TEST_TIME.with(|clock| {
            *clock.borrow_mut() =
                Some(chrono::DateTime::parse_from_rfc3339("2026-10-05T08:00:00+08:00").unwrap())
        });
        let normal = db.save_task(sample("当日新增")).unwrap();
        assert_eq!(normal.daily_sequence, 5);
        assert_eq!(db.activate_due_scheduled().unwrap(), 2);
        assert_eq!(db.activate_due_scheduled().unwrap(), 0);
        let active = db
            .list_tasks(TaskView::Queue)
            .unwrap()
            .into_iter()
            .filter(|task| task.has_active_queue)
            .collect::<Vec<_>>();
        assert_eq!(
            active.iter().map(|task| task.id).collect::<Vec<_>>(),
            vec![first.id, second.id, normal.id]
        );
        let after = db.get_task(first.id).unwrap();
        assert_eq!(after.daily_sequence, first.daily_sequence);
        assert_eq!(after.ticket_date, first.ticket_date);
        assert_eq!(after.schedule_action, "planned");
        assert!(!db.get_task(paused.id).unwrap().has_active_queue);
        assert!(!db.get_task(terminal.id).unwrap().has_active_queue);
        assert_eq!(
            db.get_logs(first.id)
                .unwrap()
                .iter()
                .filter(|log| log.log_type == "scheduled_activated")
                .count(),
            1
        );
    }

    #[test]
    fn delayed_startup_keeps_original_date_and_attributes_real_work_to_operation_day() {
        let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
        let root = std::env::temp_dir().join(format!(
            "inline-plan-late-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("inline.db");
        let db = Database::open_at(path.clone()).unwrap();
        let mut input = sample("延迟入队");
        input.planned_date = Some("2026-10-05".into());
        let task = db.save_task(input).unwrap();
        assert_eq!(
            db.statistics(
                "2026-10-02T00:00:00+08:00".into(),
                "2026-10-03T00:00:00+08:00".into(),
                480
            )
            .unwrap()
            .summary
            .rate_denominator,
            0
        );
        drop(db);
        TEST_TIME.with(|clock| {
            *clock.borrow_mut() =
                Some(chrono::DateTime::parse_from_rfc3339("2026-10-07T09:00:00+08:00").unwrap())
        });
        let db = Database::open_at(path.clone()).unwrap();
        assert_eq!(db.activate_due_scheduled().unwrap(), 1);
        assert_eq!(db.activate_due_scheduled().unwrap(), 0);
        let after = db.get_task(task.id).unwrap();
        assert_eq!(after.ticket_date, "2026-10-05");
        assert_eq!(after.daily_sequence, task.daily_sequence);
        assert_eq!(after.schedule_action, "late");
        assert_eq!(
            db.statistics(
                "2026-10-05T00:00:00+08:00".into(),
                "2026-10-06T00:00:00+08:00".into(),
                480
            )
            .unwrap()
            .summary
            .rate_denominator,
            1
        );
        db.process_round(task.id).unwrap();
        let old = db
            .statistics(
                "2026-10-05T00:00:00+08:00".into(),
                "2026-10-06T00:00:00+08:00".into(),
                480,
            )
            .unwrap();
        assert_eq!(old.summary.handled_tasks, 0);
        let real = db
            .statistics(
                "2026-10-07T00:00:00+08:00".into(),
                "2026-10-08T00:00:00+08:00".into(),
                480,
            )
            .unwrap();
        assert_eq!(real.summary.processed, 1);
        let reader = Database::open_reporting_at(path).unwrap();
        assert_eq!(reader.activate_due_scheduled().unwrap(), 0);
        assert!(db
            .get_logs(task.id)
            .unwrap()
            .iter()
            .any(|log| log.log_type == "scheduled_late"
                && log.content.contains("2026-10-05")
                && log.content.contains("2026-10-07")));
    }

    #[test]
    fn future_early_workflow_all_entry_points_and_defer_override() {
        let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
        let root = std::env::temp_dir().join(format!(
            "inline-plan-early-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        for mode in [
            "enqueue",
            "process",
            "complete",
            "processed-status",
            "completed-status",
            "processed-edit",
            "completed-edit",
            "processing-status",
            "processing-edit",
        ] {
            let mut input = sample(mode);
            input.planned_date = Some("2026-10-08".into());
            let original = db.save_task(input.clone()).unwrap();
            match mode {
                "enqueue" => db
                    .enqueue_task(QueueInput {
                        id: original.id,
                        inherit_deadline: false,
                        reason: "提前处理".into(),
                    })
                    .unwrap(),
                "process" => db.process_round(original.id).unwrap(),
                "complete" => db.complete_round(original.id).unwrap(),
                "processed-status" => db.set_status(original.id, "processed".into()).unwrap(),
                "completed-status" => db.set_status(original.id, "completed".into()).unwrap(),
                "processing-status" => db.set_status(original.id, "processing".into()).unwrap(),
                _ => {
                    input.id = Some(original.id);
                    input.status = if mode == "processing-edit" {
                        "processing"
                    } else if mode == "processed-edit" {
                        "processed"
                    } else {
                        "completed"
                    }
                    .into();
                    db.save_task(input).unwrap();
                }
            }
            let after = db.get_task(original.id).unwrap();
            assert!(!after.is_scheduled);
            assert_eq!(after.ticket_date, "2026-10-02");
            assert_eq!(after.schedule_action, "early");
            assert_eq!(after.permanent_number, original.permanent_number);
            let logs = db.get_logs(original.id).unwrap();
            assert!(logs.iter().any(|log| log.log_type == "queue_number_voided"));
            assert!(logs.iter().any(|log| log.log_type == "scheduled_early"));
            if mode.starts_with("processing") {
                assert_eq!(after.status, "processing");
                assert!(after.has_active_queue);
                assert!(after.started_at.is_some());
                assert!(db.list_work_events(original.id).unwrap().is_empty());
            } else if mode != "enqueue" {
                assert_eq!(db.list_work_events(original.id).unwrap().len(), 1);
            }
        }
        let mut input = sample("主动暂停");
        input.planned_date = Some("2026-10-05".into());
        let paused = db.save_task(input.clone()).unwrap();
        db.set_status(paused.id, "paused".into()).unwrap();
        assert!(!db.get_task(paused.id).unwrap().is_scheduled);
        input.id = Some(paused.id);
        input.planned_date = Some("2026-10-06".into());
        input.confirm_schedule_change = true;
        assert!(db.save_task(input).unwrap().is_scheduled);
        TEST_TIME.with(|clock| {
            *clock.borrow_mut() =
                Some(chrono::DateTime::parse_from_rfc3339("2026-10-06T09:00:00+08:00").unwrap())
        });
        assert_eq!(db.activate_due_scheduled().unwrap(), 1);
        let old_day = db
            .statistics(
                "2026-10-02T00:00:00+08:00".into(),
                "2026-10-03T00:00:00+08:00".into(),
                480,
            )
            .unwrap();
        assert_eq!(old_day.summary.handled_tasks, 6);
    }

    #[test]
    fn reactivation_retains_historical_statistics_clears_deadline_and_starts_new_cycle() {
        let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
        let root = std::env::temp_dir().join(format!(
            "inline-plan-reactivate-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        let mut input = sample("重新激活");
        input.details = "原详情".into();
        input.internal_notes = "原备注".into();
        input.priority = "critical".into();
        input.requested_deadline = Some("2026-10-03T18:00:00+08:00".into());
        let original = db.save_task(input.clone()).unwrap();
        db.complete_round(original.id).unwrap();
        db.archive(original.id).unwrap();
        input.id = Some(original.id);
        input.planned_date = Some("2026-10-08".into());
        input.confirm_schedule_change = true;
        let future = db.save_task(input.clone()).unwrap();
        assert_eq!(future.status, "pending");
        assert!(future.is_scheduled);
        assert!(future.requested_deadline.is_none());
        assert!(future.archived_at.is_none());
        assert!(future.completed_at.is_none());
        assert_eq!(future.permanent_number, original.permanent_number);
        assert_eq!(future.details, original.details);
        assert_eq!(future.internal_notes, original.internal_notes);
        assert_eq!(future.priority, original.priority);
        assert_eq!(future.departments, original.departments);
        assert_eq!(
            db.statistics(
                "2026-10-02T00:00:00+08:00".into(),
                "2026-10-03T00:00:00+08:00".into(),
                480
            )
            .unwrap()
            .summary
            .completed,
            1
        );
        TEST_TIME.with(|clock| {
            *clock.borrow_mut() =
                Some(chrono::DateTime::parse_from_rfc3339("2026-10-08T09:00:00+08:00").unwrap())
        });
        assert_eq!(db.activate_due_scheduled().unwrap(), 1);
        db.complete_round(original.id).unwrap();
        assert_eq!(db.list_work_events(original.id).unwrap().len(), 2);
        assert_eq!(
            db.statistics(
                "2026-10-08T00:00:00+08:00".into(),
                "2026-10-09T00:00:00+08:00".into(),
                480
            )
            .unwrap()
            .summary
            .completed,
            1
        );
        let reopened = db.save_task(input).unwrap();
        assert!(reopened.has_active_queue && !reopened.is_scheduled);
        assert_eq!(reopened.status, "pending");
        assert_eq!(
            db.get_logs(original.id)
                .unwrap()
                .iter()
                .filter(|log| log.log_type == "scheduled_reactivated")
                .count(),
            2
        );
        db.process_round(original.id).unwrap();
        let mut processed = sample("重新激活");
        processed.id = Some(original.id);
        processed.planned_date = Some("2026-10-10".into());
        processed.confirm_schedule_change = true;
        assert!(db.save_task(processed).unwrap().is_scheduled);
        assert_eq!(db.list_work_events(original.id).unwrap().len(), 3);
    }

    #[test]
    fn scheduling_v9_rollback_and_deleted_number_history() {
        let nonce = Utc::now().timestamp_nanos_opt().unwrap();
        let root = std::env::temp_dir().join(format!("inline-v9-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("rollback.db");
        create_v7_database(&path);
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("CREATE TRIGGER reject_v9 BEFORE INSERT ON schema_meta WHEN NEW.version=9 BEGIN SELECT RAISE(ABORT,'blocked v9'); END;").unwrap();
        drop(connection);
        assert!(Database::open_at(path.clone()).is_err());
        let connection = Connection::open(path).unwrap();
        assert_eq!(Database::schema_version(&connection).unwrap(), 7);
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM pragma_table_info('tasks') WHERE name='planned_date'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            0
        );
        let db = Database::open_at(root.join("history.db")).unwrap();
        let task = db.save_task(sample("永不回收")).unwrap();
        db.with_transaction(|tx| {
            void_number_on(tx, task.id, "测试作废")?;
            Ok(())
        })
        .unwrap();
        db.soft_delete(task.id).unwrap();
        db.permanently_delete_tasks(vec![task.id]).unwrap();
        db.with_conn(|connection| {
            let record: (Option<i64>, String, Option<String>) = connection
                .query_row(
                    "SELECT task_id,permanent_number,voided_at FROM queue_number_allocations",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(display_error)?;
            assert_eq!(record.0, None);
            assert_eq!(record.1, task.permanent_number);
            assert!(record.2.is_some());
            assert_eq!(
                next_daily_sequence(connection, &task.ticket_date)?,
                task.daily_sequence + 1
            );
            Ok(())
        })
        .unwrap();
    }

    fn subtask_sample(parent_task_id: i64, title: &str) -> CreateSubtaskInput {
        CreateSubtaskInput {
            parent_task_id,
            planned_date: None,
            title: title.into(),
            details: "子任务测试".into(),
            task_type: None,
            departments: None,
            contacts: None,
            priority: None,
            workload: None,
            is_urgent: false,
            urgent_requester: String::new(),
            urgent_reason: String::new(),
            requested_deadline: None,
            requested_deadline_label: None,
            internal_notes: String::new(),
            enqueue_today: true,
        }
    }

    #[test]
    fn scheduled_subtasks_inherit_dates_and_remain_independent() {
        let _clock = TestClock::at("2026-12-31T23:59:00+08:00");
        let root = std::env::temp_dir().join(format!(
            "inline-future-child-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        let mut input = sample("未来父任务");
        input.planned_date = Some("2027-01-01".into());
        let parent = db.save_task(input.clone()).unwrap();
        let child = db
            .create_subtask(subtask_sample(parent.id, "继承日期子任务"))
            .unwrap();
        assert!(child.is_scheduled && !child.has_active_queue);
        assert_eq!(child.planned_date, "2027-01-01");
        assert_eq!(child.daily_sequence, parent.daily_sequence + 1);
        let mut own = subtask_sample(parent.id, "独立日期子任务");
        own.planned_date = Some("2027-01-02".into());
        let own = db.create_subtask(own).unwrap();
        assert_eq!(own.daily_sequence, 1);
        input.id = Some(parent.id);
        input.planned_date = Some("2027-01-03".into());
        input.confirm_schedule_change = true;
        db.save_task(input).unwrap();
        assert_eq!(db.get_task(child.id).unwrap().planned_date, "2027-01-01");
        assert_eq!(db.get_task(own.id).unwrap().planned_date, "2027-01-02");
        let mut invalid = subtask_sample(parent.id, "截止时间非法");
        invalid.requested_deadline = Some("2027-01-02T12:00:00+08:00".into());
        assert!(db.create_subtask(invalid).is_err());
        let _next = TestClock::at("2027-01-01T00:00:01+08:00");
        assert_eq!(db.activate_due_scheduled().unwrap(), 1);
        assert_eq!(
            db.get_task(child.id).unwrap().daily_sequence,
            child.daily_sequence
        );
        assert!(db.get_task(parent.id).unwrap().is_scheduled);
        assert!(db.get_task(own.id).unwrap().is_scheduled);
        let _later = TestClock::at("2027-01-05T10:00:00+08:00");
        let clamped = db
            .create_subtask(subtask_sample(parent.id, "继承过去日期夹到今天"))
            .unwrap();
        assert_eq!(clamped.planned_date, "2027-01-05");
        assert!(clamped.has_active_queue);
        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn production_upgrade_from_v8_preserves_data_and_creates_pre_migration_backup() {
        let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
        let root = std::env::temp_dir().join(format!(
            "inline-production-upgrade-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        let parent = db.save_task(sample("v8升级前父任务")).unwrap();
        let child = db
            .create_subtask(subtask_sample(parent.id, "v8子任务"))
            .unwrap();
        db.process_round(child.id).unwrap();
        db.with_conn(|conn|conn.execute_batch("DROP INDEX idx_scheduled_due; DROP TABLE queue_number_allocations; ALTER TABLE tasks DROP COLUMN planned_date; ALTER TABLE tasks DROP COLUMN is_scheduled; ALTER TABLE tasks DROP COLUMN schedule_action; ALTER TABLE tasks DROP COLUMN schedule_action_at; UPDATE schema_meta SET version=8;").map_err(display_error)).unwrap();
        drop(db);
        let db = Database::open_root(root.clone()).unwrap();
        assert_eq!(db.with_conn(Database::schema_version).unwrap(), 9);
        let after = db.get_task(child.id).unwrap();
        assert_eq!(after.parent_task_id, Some(parent.id));
        assert_eq!(after.permanent_number, child.permanent_number);
        assert_eq!(after.status, "processed");
        assert!(!after.is_scheduled);
        assert_eq!(after.planned_date, child.ticket_date);
        assert_eq!(db.list_work_events(child.id).unwrap().len(), 1);
        let backups = db.list_backups().unwrap();
        let migration = backups
            .iter()
            .find(|item| item.name.contains("before-migration"))
            .unwrap();
        let saved = Connection::open(&migration.path).unwrap();
        assert_eq!(Database::schema_version(&saved).unwrap(), 8);
        drop(saved);
        let mut future = sample("升级后预约");
        future.planned_date = Some("2026-10-05".into());
        let future = db.save_task(future).unwrap();
        drop(db);
        let db = Database::open_root(root.clone()).unwrap();
        assert!(db.get_task(future.id).unwrap().is_scheduled);
        assert_eq!(db.list_work_events(child.id).unwrap().len(), 1);
        assert_eq!(
            db.list_backups()
                .unwrap()
                .iter()
                .filter(|item| item.name.contains("before-migration"))
                .count(),
            1
        );
        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn future_backup_preserves_reservations_deleted_history_and_rolls_back_atomically() {
        let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
        let root = std::env::temp_dir().join(format!(
            "inline-future-restore-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let source = Database::open_at(root.join("source.db")).unwrap();
        let mut input = sample("备份未来父");
        input.planned_date = Some("2026-10-05".into());
        let parent = source.save_task(input.clone()).unwrap();
        let _child = source
            .create_subtask(subtask_sample(parent.id, "备份未来子"))
            .unwrap();
        input.title = "备份暂停".into();
        let paused = source.save_task(input.clone()).unwrap();
        source
            .set_status(paused.id, "waiting_materials".into())
            .unwrap();
        input.title = "删除占号".into();
        let deleted = source.save_task(input.clone()).unwrap();
        source.soft_delete(deleted.id).unwrap();
        source.permanently_delete_tasks(vec![deleted.id]).unwrap();
        let backup = source.create_backup("manual").unwrap();
        let target = Database::open_at(root.join("target.db")).unwrap();
        input.title = "本地未来不可覆盖".into();
        let local = target.save_task(input.clone()).unwrap();
        let imported = target.import_backup(backup.path).unwrap();
        target.with_conn(|tx|tx.execute_batch("CREATE TRIGGER reject_merge BEFORE INSERT ON queue_number_allocations WHEN NEW.permanent_number<>'' BEGIN SELECT RAISE(ABORT,'restore rollback probe'); END").map_err(display_error)).unwrap();
        assert!(target.restore_backup(imported.path.clone()).is_err());
        assert_eq!(target.list_tasks(TaskView::Queue).unwrap().len(), 1);
        target
            .with_conn(|tx| {
                tx.execute_batch("DROP TRIGGER reject_merge")
                    .map_err(display_error)
            })
            .unwrap();
        let result = target.restore_backup(imported.path.clone()).unwrap();
        assert_eq!(result.added_tasks, 3);
        let items = target.list_tasks(TaskView::Queue).unwrap();
        let restored_parent = items
            .iter()
            .find(|task| task.title == "备份未来父")
            .unwrap();
        let restored_child = items
            .iter()
            .find(|task| task.title == "备份未来子")
            .unwrap();
        assert_eq!(restored_child.parent_task_id, Some(restored_parent.id));
        assert!(restored_parent.is_scheduled && !restored_parent.has_active_queue);
        assert_eq!(restored_parent.planned_date, "2026-10-05");
        assert_ne!(restored_parent.daily_sequence, local.daily_sequence);
        assert!(
            !items
                .iter()
                .find(|task| task.title == "备份暂停")
                .unwrap()
                .is_scheduled
        );
        assert_eq!(
            target.get_task(local.id).unwrap().daily_sequence,
            local.daily_sequence
        );
        let count = target
            .with_conn(|tx| {
                tx.query_row("SELECT count(*) FROM queue_number_allocations", [], |row| {
                    row.get::<_, i64>(0)
                })
                .map_err(display_error)
            })
            .unwrap();
        let second = target.restore_backup(imported.path).unwrap();
        assert_eq!(second.added_tasks, 0);
        assert_eq!(
            target
                .with_conn(|tx| tx
                    .query_row("SELECT count(*) FROM queue_number_allocations", [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .map_err(display_error))
                .unwrap(),
            count
        );
        target.with_conn(|tx|{
            let orphan:i64=tx.query_row("SELECT count(*) FROM queue_number_allocations WHERE task_id IS NULL AND permanent_number=?",[deleted.permanent_number],|row|row.get(0)).map_err(display_error)?;assert!(orphan>0);
            assert_eq!(tx.query_row("SELECT count(*) FROM task_queue_entries",[],|row|row.get::<_,i64>(0)).map_err(display_error)?,0);
            assert_eq!(tx.query_row("SELECT count(*) FROM pragma_foreign_key_check",[],|row|row.get::<_,i64>(0)).map_err(display_error)?,0);
            Ok(())
        }).unwrap();
        input.title = "导入后新预约".into();
        let next = target.save_task(input).unwrap();
        assert!(
            next.daily_sequence > deleted.daily_sequence
                && next.daily_sequence > restored_child.daily_sequence
        );
        drop(source);
        drop(target);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_midnight_checks_activate_exactly_once_and_manual_work_enters_first() {
        let _clock = TestClock::at("2028-02-28T23:59:00+08:00");
        let root = std::env::temp_dir().join(format!(
            "inline-future-concurrent-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = std::sync::Arc::new(Database::open_at(root.join("inline.db")).unwrap());
        let mut input = sample("闰日计划");
        input.planned_date = Some("2028-02-29".into());
        let task = db.save_task(input).unwrap();
        let handles = (0..4)
            .map(|_| {
                let db = db.clone();
                std::thread::spawn(move || {
                    let _clock = TestClock::at("2028-02-29T00:00:01+08:00");
                    db.activate_due_scheduled().unwrap()
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .sum::<usize>(),
            1
        );
        assert_eq!(
            db.get_task(task.id).unwrap().daily_sequence,
            task.daily_sequence
        );
        let mut input = sample("补录同步未来事项");
        input.planned_date = Some("2028-03-01".into());
        let task = db.save_task(input).unwrap();
        db.record_work_event(WorkEventInput {
            task_id: task.id,
            result_status: "processed".into(),
            handled_at: now(),
            note: "同步状态".into(),
            sync_status: true,
        })
        .unwrap();
        let result = db.get_task(task.id).unwrap();
        assert!(!result.is_scheduled);
        assert_eq!(result.ticket_date, "2028-02-28");
        assert_eq!(result.schedule_action, "early");
        assert_eq!(db.list_work_events(task.id).unwrap().len(), 1);
        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn backup_history_cannot_void_an_existing_local_current_number() {
        let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
        let root = std::env::temp_dir().join(format!(
            "inline-backup-local-number-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(root.join("source")).unwrap();
        let local = Database::open_at(root.join("inline.db")).unwrap();
        let task = local.save_task(sample("本地队列号优先")).unwrap();
        let snapshot = local.create_backup("manual").unwrap();
        fs::copy(snapshot.path, root.join("source/inline.db")).unwrap();
        let source = Database::open_at(root.join("source/inline.db")).unwrap();
        source.process_round(task.id).unwrap();
        source
            .enqueue_task(QueueInput {
                id: task.id,
                inherit_deadline: false,
                reason: "备份来源办理后重新入队".into(),
            })
            .unwrap();
        let backup = source.create_backup("manual").unwrap();
        let backup = local.import_backup(backup.path).unwrap();
        assert_eq!(local.restore_backup(backup.path).unwrap().added_tasks, 0);
        let current = local.get_task(task.id).unwrap();
        assert!(current.has_active_queue);
        assert_eq!(current.daily_sequence, task.daily_sequence);
        local.with_conn(|conn|{let voided:Option<String>=conn.query_row("SELECT voided_at FROM queue_number_allocations WHERE task_id=? AND queue_date=? AND daily_sequence=?",params![task.id,task.ticket_date,task.daily_sequence],|row|row.get(0)).map_err(display_error)?;assert_eq!(voided,None);Ok(())}).unwrap();
        assert_eq!(local.list_work_events(task.id).unwrap().len(), 1);
        drop(local);
        drop(source);
        let _ = fs::remove_dir_all(root);
    }

    #[derive(Debug, PartialEq)]
    struct RelationInvariantSnapshot {
        status: String,
        daily_sequence: i64,
        ticket_date: String,
        custom_sort_order: i64,
        requested_deadline: Option<String>,
        requested_deadline_label: Option<String>,
        is_urgent: i64,
        urgent_requester: String,
        urgent_reason: String,
        department: String,
        contact: String,
        task_type: String,
        queue_date: String,
        queue_daily_sequence: i64,
        enqueued_at: String,
        queue_deadline: Option<String>,
        queue_deadline_label: Option<String>,
        queue_closed_at: Option<String>,
    }

    fn relation_invariant_snapshot(db: &Database, task_id: i64) -> RelationInvariantSnapshot {
        db.with_conn(|connection| {
            connection
                .query_row(
                    "SELECT tasks.status,tasks.daily_sequence,tasks.ticket_date,
                            tasks.custom_sort_order,tasks.requested_deadline,
                            tasks.requested_deadline_label,tasks.is_urgent,tasks.urgent_requester,
                            tasks.urgent_reason,tasks.department,tasks.contact,tasks.task_type,
                            queue.queue_date,queue.daily_sequence,queue.enqueued_at,
                            queue.requested_deadline,queue.requested_deadline_label,queue.closed_at
                     FROM tasks JOIN task_queue_entries queue ON queue.task_id=tasks.id
                     WHERE tasks.id=? AND queue.closed_at IS NULL",
                    [task_id],
                    |row| {
                        Ok(RelationInvariantSnapshot {
                            status: row.get(0)?,
                            daily_sequence: row.get(1)?,
                            ticket_date: row.get(2)?,
                            custom_sort_order: row.get(3)?,
                            requested_deadline: row.get(4)?,
                            requested_deadline_label: row.get(5)?,
                            is_urgent: row.get(6)?,
                            urgent_requester: row.get(7)?,
                            urgent_reason: row.get(8)?,
                            department: row.get(9)?,
                            contact: row.get(10)?,
                            task_type: row.get(11)?,
                            queue_date: row.get(12)?,
                            queue_daily_sequence: row.get(13)?,
                            enqueued_at: row.get(14)?,
                            queue_deadline: row.get(15)?,
                            queue_deadline_label: row.get(16)?,
                            queue_closed_at: row.get(17)?,
                        })
                    },
                )
                .map_err(display_error)
        })
        .unwrap()
    }

    #[test]
    fn migration_v8_adds_self_reference_and_rolls_back_atomically() {
        let nonce = Utc::now().timestamp_nanos_opt().unwrap();
        let upgraded_root = std::env::temp_dir().join(format!("inline-v8-upgrade-{nonce}"));
        fs::create_dir_all(&upgraded_root).unwrap();
        let upgraded_path = upgraded_root.join("inline.db");
        create_v7_database(&upgraded_path);

        let upgraded = Database::open_at(upgraded_path.clone()).unwrap();
        assert_eq!(upgraded.with_conn(Database::schema_version).unwrap(), 9);
        let task = upgraded.get_task(1).unwrap();
        assert_eq!(task.parent_task_id, None);
        assert_eq!(task.subtask_sort_order, 0);
        upgraded
            .with_conn(|connection| {
                let foreign_key = {
                    let mut statement = connection
                        .prepare("PRAGMA foreign_key_list('tasks')")
                        .map_err(display_error)?;
                    let rows = statement
                        .query_map([], |row| {
                            Ok((row.get::<_, String>(3)?, row.get::<_, String>(6)?))
                        })
                        .map_err(display_error)?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(display_error)?;
                    rows
                };
                assert!(foreign_key.iter().any(
                    |(column, on_delete)| column == "parent_task_id" && on_delete == "SET NULL"
                ));
                let index_exists: i64 = connection
                    .query_row(
                        "SELECT count(*) FROM sqlite_master
                         WHERE type='index' AND name='idx_tasks_parent'",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(display_error)?;
                assert_eq!(index_exists, 1);
                let integrity: String = connection
                    .query_row("PRAGMA integrity_check", [], |row| row.get(0))
                    .map_err(display_error)?;
                assert_eq!(integrity, "ok");
                let foreign_key_errors: i64 = connection
                    .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                        row.get(0)
                    })
                    .map_err(display_error)?;
                assert_eq!(foreign_key_errors, 0);
                Ok(())
            })
            .unwrap();
        drop(upgraded);

        let rollback_root = std::env::temp_dir().join(format!("inline-v8-rollback-{nonce}"));
        fs::create_dir_all(&rollback_root).unwrap();
        let rollback_path = rollback_root.join("inline.db");
        create_v7_database(&rollback_path);
        let connection = Connection::open(&rollback_path).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_v8 BEFORE INSERT ON schema_meta
                 WHEN NEW.version=8 BEGIN SELECT RAISE(ABORT,'blocked v8'); END;",
            )
            .unwrap();
        drop(connection);
        assert!(Database::open_at(rollback_path.clone()).is_err());
        let rolled_back = Connection::open(rollback_path).unwrap();
        let version: i64 = rolled_back
            .query_row("SELECT max(version) FROM schema_meta", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 7);
        let new_columns: i64 = rolled_back
            .query_row(
                "SELECT count(*) FROM pragma_table_info('tasks')
                 WHERE name IN ('parent_task_id','subtask_sort_order')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(new_columns, 0);
        drop(rolled_back);

        let _ = fs::remove_dir_all(upgraded_root);
        let _ = fs::remove_dir_all(rollback_root);
    }

    #[test]
    fn parent_relationships_enforce_two_levels_preserve_queue_and_support_ordering() {
        let root = std::env::temp_dir().join(format!(
            "inline-parent-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        let parent = db.save_task(sample("父任务 A")).unwrap();
        let other_parent = db.save_task(sample("父任务 B")).unwrap();
        let mut archived_input = sample("已归档父任务");
        archived_input.status = "archived".into();
        let archived_parent = db.save_task(archived_input).unwrap();
        let trashed_parent = db.save_task(sample("回收站父任务")).unwrap();
        db.soft_delete(trashed_parent.id).unwrap();
        let mut first_input = urgent_sample("子任务一");
        first_input.requested_deadline = Some("2026-10-10T09:00:00+08:00".into());
        first_input.requested_deadline_label = Some("约定期限".into());
        let first = db.save_task(first_input).unwrap();
        let second = db.save_task(sample("子任务二")).unwrap();

        let queue_snapshot = relation_invariant_snapshot(&db, first.id);

        let candidates = db.list_parent_task_candidates(first.id).unwrap();
        assert!(candidates.iter().any(|task| task.id == parent.id));
        assert!(candidates.iter().any(|task| task.id == other_parent.id));
        assert!(candidates.iter().any(|task| task.id == archived_parent.id));
        assert!(!candidates.iter().any(|task| task.id == trashed_parent.id));
        assert!(!candidates.iter().any(|task| task.id == first.id));

        assert!(db.set_parent_task(first.id, Some(first.id)).is_err());
        db.set_parent_task(first.id, Some(parent.id)).unwrap();
        db.set_parent_task(first.id, Some(parent.id)).unwrap();
        db.set_parent_task(second.id, Some(parent.id)).unwrap();
        assert!(db
            .set_parent_task(parent.id, Some(other_parent.id))
            .is_err());
        assert!(db.set_parent_task(parent.id, Some(first.id)).is_err());
        assert!(db.set_parent_task(other_parent.id, Some(first.id)).is_err());
        assert!(db
            .list_parent_task_candidates(parent.id)
            .unwrap()
            .is_empty());

        db.reorder_subtasks(ReorderSubtasksInput {
            parent_task_id: parent.id,
            task_ids: vec![second.id, first.id],
        })
        .unwrap();
        let reordered = db.list_subtasks(parent.id).unwrap();
        assert_eq!(
            reordered.iter().map(|task| task.id).collect::<Vec<_>>(),
            vec![second.id, first.id]
        );
        assert_eq!(reordered[0].subtask_sort_order, 1);
        assert_eq!(reordered[1].subtask_sort_order, 2);
        assert!(db
            .reorder_subtasks(ReorderSubtasksInput {
                parent_task_id: parent.id,
                task_ids: vec![first.id],
            })
            .is_err());

        db.set_parent_task(first.id, Some(other_parent.id)).unwrap();
        assert_eq!(
            db.list_subtasks(parent.id).unwrap()[0].subtask_sort_order,
            1
        );
        db.set_parent_task(first.id, None).unwrap();
        let relationship_logs = db
            .get_logs(first.id)
            .unwrap()
            .into_iter()
            .filter(|log| log.log_type == "relation")
            .map(|log| log.content)
            .collect::<Vec<_>>();
        assert_eq!(relationship_logs.len(), 3);
        assert!(relationship_logs
            .iter()
            .any(|value| value.starts_with("设置所属任务")));
        assert!(relationship_logs
            .iter()
            .any(|value| value.starts_with("更换所属任务")));
        assert!(relationship_logs
            .iter()
            .any(|value| value.starts_with("解除所属任务")));
        let (status_history_count, work_event_count) = db
            .with_conn(|connection| {
                Ok((
                    connection
                        .query_row(
                            "SELECT count(*) FROM status_history WHERE task_id=?",
                            [first.id],
                            |row| row.get::<_, i64>(0),
                        )
                        .map_err(display_error)?,
                    connection
                        .query_row(
                            "SELECT count(*) FROM task_work_events WHERE task_id=?",
                            [first.id],
                            |row| row.get::<_, i64>(0),
                        )
                        .map_err(display_error)?,
                ))
            })
            .unwrap();
        assert_eq!(status_history_count, 1);
        assert_eq!(work_event_count, 0);
        assert!(db
            .get_logs(first.id)
            .unwrap()
            .iter()
            .any(|log| log.log_type == "created"));

        let unchanged_queue = relation_invariant_snapshot(&db, first.id);
        assert_eq!(unchanged_queue, queue_snapshot);

        assert!(db
            .with_conn(|connection| {
                connection
                    .execute(
                        "UPDATE tasks SET parent_task_id=999999 WHERE id=?",
                        [first.id],
                    )
                    .map(|_| ())
                    .map_err(display_error)
            })
            .is_err());

        db.set_parent_task(first.id, Some(other_parent.id)).unwrap();
        db.delete_task_group(DeleteTaskInput {
            task_id: other_parent.id,
            include_subtasks: true,
        })
        .unwrap();
        assert_eq!(
            db.get_task(first.id).unwrap().parent_task_id,
            Some(other_parent.id)
        );
        db.permanently_delete_tasks(vec![other_parent.id]).unwrap();
        assert_eq!(db.get_task(first.id).unwrap().parent_task_id, None);

        drop(db);
        let _ = fs::remove_dir_all(root);
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
        db.set_setting("ui_font_family".into(), "测试字体 Family".into())
            .unwrap();
        assert_eq!(
            db.settings()
                .unwrap()
                .get("ui_font_family")
                .map(String::as_str),
            Some("测试字体 Family")
        );
        assert!(db
            .set_setting("ui_font_family".into(), "bad\nfont".into())
            .is_err());
        assert!(db
            .set_setting("ui_font_family".into(), "a".repeat(257))
            .is_err());
        db.set_setting("ui_font_family".into(), String::new())
            .unwrap();
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
        assert_eq!(db.backup_directory(), root.join("backups"));
        assert!(db.backup_directory().is_dir());
        let backup = db.create_backup("manual").unwrap();
        assert!(backup.name.starts_with("InLine-backup-"));
        assert!(backup.name.ends_with("-manual.db"));
        db.delete_backup(backup.path).unwrap();
        drop(db);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn imported_backup_uses_import_time_and_is_listed_first() {
        let source_root = std::env::temp_dir().join(format!(
            "inline-import-source-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let target_root = std::env::temp_dir().join(format!(
            "inline-import-target-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&source_root).unwrap();
        fs::create_dir_all(&target_root).unwrap();
        let source = Database::open_at(source_root.join("inline.db")).unwrap();
        source.save_task(sample("待导入事项")).unwrap();
        let source_backup = source.create_backup("manual").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));

        let target = Database::open_at(target_root.join("inline.db")).unwrap();
        target.create_backup("manual").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let imported = target.import_backup(source_backup.path).unwrap();
        let listed = target.list_backups().unwrap();
        assert_eq!(
            listed.first().map(|backup| backup.path.as_str()),
            Some(imported.path.as_str())
        );
        assert!(imported.name.ends_with("-import.db"));

        drop(source);
        drop(target);
        let _ = fs::remove_dir_all(source_root);
        let _ = fs::remove_dir_all(target_root);
    }
    #[test]
    fn permanent_delete_is_limited_to_trash_and_empty_trash_is_scoped() {
        let root = std::env::temp_dir().join(format!(
            "inline-trash-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        let active = db.save_task(sample("保留事项")).unwrap();
        let first = db.save_task(sample("永久删除事项")).unwrap();
        let second = db.save_task(sample("清空事项")).unwrap();
        assert!(db.permanently_delete_tasks(vec![active.id]).is_err());
        db.soft_delete(first.id).unwrap();
        db.soft_delete(second.id).unwrap();
        assert_eq!(db.permanently_delete_tasks(vec![first.id]).unwrap(), 1);
        assert!(db.get_task(first.id).is_err());
        assert_eq!(db.empty_trash().unwrap(), 1);
        assert!(db.get_task(second.id).is_err());
        assert_eq!(db.get_task(active.id).unwrap().title, "保留事项");
        drop(db);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn reporting_reader_is_read_only_and_excludes_private_fields() {
        let root = std::env::temp_dir().join(format!(
            "inline-report-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("inline.db");
        let db = Database::open_at(path.clone()).unwrap();
        let mut input = sample("完成报告功能");
        input.details = "不应暴露的事项详情".into();
        input.internal_notes = "不应暴露的内部备注".into();
        input.contact = "不应暴露的联系人".into();
        input.contacts = vec!["不应暴露的联系人".into()];
        let task = db.save_task(input).unwrap();
        db.record_work_event(WorkEventInput {
            task_id: task.id,
            result_status: "completed".into(),
            handled_at: "2026-08-08T09:00:00+08:00".into(),
            note: "完成 MCP 只读查询".into(),
            sync_status: true,
        })
        .unwrap();
        drop(db);

        let reader = Database::open_reporting_at(path).unwrap();
        let page = reader
            .report_items(
                "2026-08-08T00:00:00+08:00".into(),
                "2026-08-09T00:00:00+08:00".into(),
                100,
                0,
            )
            .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].title, "完成报告功能");
        assert_eq!(page.items[0].departments, vec!["产品组"]);
        assert_eq!(page.items[0].work_events[0].note, "完成 MCP 只读查询");
        let json = serde_json::to_string(&page).unwrap();
        assert!(!json.contains("不应暴露的联系人"));
        assert!(!json.contains("不应暴露的事项详情"));
        assert!(!json.contains("不应暴露的内部备注"));
        assert!(reader
            .set_setting("week_start_day".into(), "sunday".into())
            .is_err());
        drop(reader);
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
        let calendar = db
            .work_calendar(
                (Utc::now() - chrono::Duration::hours(1)).to_rfc3339(),
                (Utc::now() + chrono::Duration::hours(2)).to_rfc3339(),
            )
            .unwrap();
        assert!(calendar
            .tasks
            .iter()
            .any(|task| task.task_id == target.id && task.intervals.len() == 2));
        assert!(!calendar.tasks.iter().any(|task| task.task_id == source.id));
        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn work_calendar_uses_real_queue_rounds_and_effective_events() {
        let root = std::env::temp_dir().join(format!(
            "inline-calendar-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        let task = db.save_task(sample("三轮事项")).unwrap();
        let active = db.save_task(sample("当前活动事项")).unwrap();
        let corrected = db.save_task(sample("已纠错事项")).unwrap();
        let cross_month = db.save_task(sample("跨月事项")).unwrap();

        db.with_conn(|connection| {
            let first_id: i64 = connection
                .query_row(
                    "SELECT id FROM task_queue_entries WHERE task_id=?",
                    [task.id],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            connection
                .execute(
                    "UPDATE task_queue_entries SET enqueued_at=?,closed_at=?,close_reason='本轮已处理' WHERE id=?",
                    params!["2026-08-24T09:00:00+08:00", "2026-08-26T11:00:00+08:00", first_id],
                )
                .map_err(display_error)?;
            for (queue_date, sequence, enqueued_at, closed_at, reason) in [
                ("2026-08-27", 91, "2026-08-27T09:30:00+08:00", Some("2026-08-27T12:00:00+08:00"), "本轮已处理"),
                ("2026-08-28", 92, "2026-08-28T13:00:00+08:00", Some("2026-08-28T16:00:00+08:00"), "本轮已完成"),
            ] {
                connection.execute(
                    "INSERT INTO task_queue_entries(task_id,queue_date,daily_sequence,enqueued_at,closed_at,close_reason,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)",
                    params![task.id,queue_date,sequence,enqueued_at,closed_at,reason,enqueued_at,closed_at.unwrap()],
                ).map_err(display_error)?;
            }
            for (status, handled_at) in [
                ("waiting_materials", "2026-08-26T10:59:00+08:00"),
                ("processed", "2026-08-27T11:59:00+08:00"),
                ("completed", "2026-08-28T15:59:00+08:00"),
            ] {
                connection.execute(
                    "INSERT INTO task_work_events(task_id,result_status,handled_at,task_type_snapshot,source,note,created_at,updated_at) VALUES(?,?,?,?, 'quick_action','',?,?)",
                    params![task.id,status,handled_at,"任务处理",handled_at,handled_at],
                ).map_err(display_error)?;
            }
            connection.execute(
                "UPDATE task_queue_entries SET enqueued_at='2026-08-27T08:00:00+08:00' WHERE task_id=?",
                [active.id],
            ).map_err(display_error)?;
            let corrected_entry: i64 = connection.query_row(
                "SELECT id FROM task_queue_entries WHERE task_id=?",[corrected.id],|row|row.get(0),
            ).map_err(display_error)?;
            connection.execute(
                "UPDATE task_queue_entries SET enqueued_at='2026-08-29T09:00:00+08:00',closed_at='2026-08-29T10:00:00+08:00',close_reason='本轮已处理' WHERE id=?",
                [corrected_entry],
            ).map_err(display_error)?;
            connection.execute(
                "INSERT INTO task_work_events(task_id,result_status,handled_at,task_type_snapshot,source,note,created_at,updated_at,voided_at) VALUES(?,'processed','2026-08-29T09:59:00+08:00','任务处理','quick_action','','2026-08-29T09:59:00+08:00','2026-08-29T10:01:00+08:00','2026-08-29T10:01:00+08:00')",
                [corrected.id],
            ).map_err(display_error)?;
            connection.execute(
                "UPDATE task_queue_entries SET enqueued_at='2026-07-31T16:00:00+08:00',closed_at='2026-08-02T10:00:00+08:00',close_reason='本轮已处理' WHERE task_id=?",
                [cross_month.id],
            ).map_err(display_error)?;
            connection.execute(
                "INSERT INTO task_work_events(task_id,result_status,handled_at,task_type_snapshot,source,note,created_at,updated_at) VALUES(?,'processed','2026-08-02T09:59:00+08:00','任务处理','quick_action','','2026-08-02T09:59:00+08:00','2026-08-02T09:59:00+08:00')",
                [cross_month.id],
            ).map_err(display_error)?;
            Ok(())
        }).unwrap();

        let calendar = db
            .work_calendar(
                "2026-08-24T00:00:00+08:00".into(),
                "2026-08-31T00:00:00+08:00".into(),
            )
            .unwrap();
        assert_eq!(calendar.summary.handled_tasks, 1);
        assert_eq!(calendar.summary.handling_rounds, 3);
        assert_eq!(calendar.summary.completed_tasks, 1);
        let rounds = calendar
            .tasks
            .iter()
            .find(|row| row.task_id == task.id)
            .unwrap();
        assert_eq!(rounds.intervals.len(), 3);
        assert_eq!(
            rounds
                .intervals
                .iter()
                .map(|entry| entry.round_index)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(
            rounds.intervals[0].result_status.as_deref(),
            Some("waiting_materials")
        );
        assert_eq!(
            rounds.intervals[2].result_status.as_deref(),
            Some("completed")
        );
        assert!(
            calendar
                .tasks
                .iter()
                .find(|row| row.task_id == active.id)
                .unwrap()
                .intervals[0]
                .current_active
        );
        assert!(calendar
            .tasks
            .iter()
            .find(|row| row.task_id == corrected.id)
            .unwrap()
            .intervals[0]
            .result_status
            .is_none());

        let cross_week = db
            .work_calendar(
                "2026-08-25T00:00:00+08:00".into(),
                "2026-08-27T00:00:00+08:00".into(),
            )
            .unwrap();
        assert_eq!(
            cross_week
                .tasks
                .iter()
                .find(|row| row.task_id == task.id)
                .unwrap()
                .intervals[0]
                .round_index,
            1
        );
        let month_boundary = db
            .work_calendar(
                "2026-08-01T00:00:00+08:00".into(),
                "2026-08-03T00:00:00+08:00".into(),
            )
            .unwrap();
        assert_eq!(
            month_boundary
                .tasks
                .iter()
                .find(|row| row.task_id == cross_month.id)
                .unwrap()
                .intervals[0]
                .round_index,
            1
        );
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
    fn deferred_entry_time_only_changes_after_leaving_and_reentering() {
        let root = std::env::temp_dir().join(format!(
            "inline-deferred-entry-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        let created = db.save_task(sample("暂缓排序事项")).unwrap();

        db.set_status(created.id, "waiting_materials".into())
            .unwrap();
        db.with_conn(|connection| {
            connection.execute(
                "UPDATE status_history SET created_at='2026-08-07T09:00:00Z' WHERE id=(SELECT max(id) FROM status_history WHERE task_id=?)",
                [created.id],
            ).map(|_| ()).map_err(display_error)
        }).unwrap();
        assert_eq!(
            db.get_task(created.id)
                .unwrap()
                .deferred_entered_at
                .as_deref(),
            Some("2026-08-07T09:00:00Z")
        );

        db.set_status(created.id, "waiting_confirmation".into())
            .unwrap();
        db.with_conn(|connection| {
            connection.execute(
                "UPDATE status_history SET created_at='2026-08-07T10:00:00Z' WHERE id=(SELECT max(id) FROM status_history WHERE task_id=?)",
                [created.id],
            ).map(|_| ()).map_err(display_error)
        }).unwrap();
        assert_eq!(
            db.get_task(created.id)
                .unwrap()
                .deferred_entered_at
                .as_deref(),
            Some("2026-08-07T09:00:00Z")
        );

        db.set_status(created.id, "pending".into()).unwrap();
        db.set_status(created.id, "waiting_counterparty_confirmation".into())
            .unwrap();
        db.with_conn(|connection| {
            connection.execute(
                "UPDATE status_history SET created_at='2026-08-07T11:00:00Z' WHERE id=(SELECT max(id) FROM status_history WHERE task_id=?)",
                [created.id],
            ).map(|_| ()).map_err(display_error)
        }).unwrap();
        assert_eq!(
            db.get_task(created.id)
                .unwrap()
                .deferred_entered_at
                .as_deref(),
            Some("2026-08-07T11:00:00Z")
        );

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
    fn quick_status_and_urgent_actions_preserve_business_rules() {
        let _clock = TestClock::at("2026-10-01T08:00:00+08:00");
        let root = std::env::temp_dir().join(format!(
            "inline-quick-actions-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();

        let mut input = sample("快捷操作事项");
        input.requested_deadline = Some("2026-10-01T10:00:00Z".into());
        input.requested_deadline_label = Some("国庆前".into());
        let created = db.save_task(input).unwrap();

        db.set_urgent(created.id, true, "测试申请人".into(), "需要优先处理".into())
            .unwrap();
        let urgent = db.get_task(created.id).unwrap();
        assert!(urgent.is_urgent);
        assert_eq!(urgent.urgent_requester, "测试申请人");
        assert_eq!(urgent.urgent_reason, "需要优先处理");

        db.set_status(created.id, "waiting_confirmation".into())
            .unwrap();
        let deferred = db.get_task(created.id).unwrap();
        assert!(!deferred.is_urgent);
        assert!(!deferred.has_active_queue);

        db.set_status(created.id, "pending".into()).unwrap();
        let requeued = db.get_task(created.id).unwrap();
        assert!(requeued.has_active_queue);
        assert_eq!(
            requeued.requested_deadline.as_deref(),
            Some("2026-10-01T10:00:00Z")
        );
        assert_eq!(requeued.requested_deadline_label.as_deref(), Some("国庆前"));

        db.set_urgent(created.id, true, "再次申请".into(), "仍需优先处理".into())
            .unwrap();
        db.set_urgent(created.id, false, "".into(), "".into())
            .unwrap();
        let normal = db.get_task(created.id).unwrap();
        assert!(!normal.is_urgent);
        assert!(normal.urgent_requester.is_empty());
        assert!(normal.urgent_reason.is_empty());

        let active_urgent_records: i64 = db
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
        assert!(db
            .set_urgent(created.id, true, "".into(), "".into())
            .is_err());

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
        assert_eq!(statistics.by_department.len(), 1);
        assert_eq!(statistics.by_department[0].department, "产品组");
        assert_eq!(statistics.by_department[0].handled_tasks, 1);
        db.save_task(sample("本周期尚未处理事项")).unwrap();
        let processing_statistics = db.statistics(start.clone(), end.clone(), 480).unwrap();
        assert_eq!(processing_statistics.summary.rate_mode, "processing");
        assert_eq!(processing_statistics.summary.rate_numerator, 1);
        assert_eq!(processing_statistics.summary.rate_denominator, 2);
        assert_eq!(processing_statistics.summary.completion_rate, 0.5);

        let mut future_deadline_input = sample("远期截止事项");
        future_deadline_input.requested_deadline =
            Some((Utc::now() + chrono::Duration::days(1)).to_rfc3339());
        let future_deadline = db.save_task(future_deadline_input).unwrap();
        let future_excluded = db.statistics(start.clone(), end.clone(), 480).unwrap();
        assert_eq!(future_excluded.summary.rate_denominator, 2);

        let mut in_range_deadline_input = sample("周期内截止事项");
        in_range_deadline_input.requested_deadline = Some(Utc::now().to_rfc3339());
        db.save_task(in_range_deadline_input).unwrap();
        let in_range_included = db.statistics(start.clone(), end.clone(), 480).unwrap();
        assert_eq!(in_range_included.summary.rate_denominator, 3);

        let mut cleared_deadline_input = sample("远期截止事项");
        cleared_deadline_input.id = Some(future_deadline.id);
        db.save_task(cleared_deadline_input).unwrap();
        let cleared_deadline_included = db.statistics(start.clone(), end.clone(), 480).unwrap();
        assert_eq!(cleared_deadline_included.summary.rate_denominator, 4);

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

        let mut reclassified = sample("等待材料事项");
        reclassified.id = Some(created.id);
        reclassified.status = "waiting_materials".into();
        reclassified.task_type = "法律咨询".into();
        db.save_task(reclassified).unwrap();
        let reclassified_statistics = db.statistics(start.clone(), end.clone(), 480).unwrap();
        assert_eq!(reclassified_statistics.by_task_type.len(), 1);
        assert_eq!(
            reclassified_statistics.by_task_type[0].task_type,
            "法律咨询"
        );
        assert_eq!(
            db.statistics_details(start.clone(), end.clone(), "任务处理".into())
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            db.statistics_details(start.clone(), end.clone(), "法律咨询".into())
                .unwrap()
                .len(),
            1
        );

        let first = db
            .list_work_events(created.id)
            .unwrap()
            .into_iter()
            .find(|event| event.is_first_valid)
            .unwrap();
        db.void_work_event(first.id, true).unwrap();
        assert_eq!(db.list_work_events(created.id).unwrap().len(), 2);
        assert_eq!(db.get_task(created.id).unwrap().processing_rounds, 2);

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
    fn trend_details_match_bucket_counts_and_historical_results() {
        let root = std::env::temp_dir().join(format!(
            "inline-trend-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        let first = db.save_task(sample("同日重复办理")).unwrap();
        let second = db.save_task(sample("跨日办理")).unwrap();
        let waiting = db.save_task(sample("等待事项")).unwrap();
        let voided = db.save_task(sample("作废记录")).unwrap();
        let deleted = db.save_task(sample("已删除事项")).unwrap();
        db.with_conn(|connection| {
            for (id, status, at) in [
                (first.id, "processed", "2026-09-20T16:00:00Z"),
                (first.id, "completed", "2026-09-20T16:00:00Z"),
                (second.id, "processed", "2026-09-21T23:59:59+08:00"),
                (second.id, "completed", "2026-09-22T00:00:00+08:00"),
                (waiting.id, "waiting_materials", "2026-09-21T13:00:00+08:00"),
                (voided.id, "completed", "2026-09-21T13:00:00+08:00"),
                (deleted.id, "processed", "2026-09-21T13:00:00+08:00"),
            ] {
                record_work_event_on(connection, id, status, at, "任务处理", "quick_action", "")?;
            }
            connection
                .execute(
                    "UPDATE task_work_events SET voided_at=? WHERE task_id=?",
                    params![now(), voided.id],
                )
                .map_err(display_error)?;
            Ok(())
        })
        .unwrap();
        db.soft_delete(deleted.id).unwrap();
        let start = "2026-09-21T00:00:00+08:00".to_string();
        let end = "2026-09-22T00:00:00+08:00".to_string();
        let day = db.statistics(start.clone(), end.clone(), 480).unwrap();
        assert_eq!(day.trend[0].period_start, "2026-09-21");
        assert_eq!(
            (
                day.trend[0].handled_tasks,
                day.trend[0].processed,
                day.trend[0].completed
            ),
            (3, 1, 1)
        );
        for (status, expected) in [(None, 3), (Some("processed"), 1), (Some("completed"), 1)] {
            let details = db
                .statistics_trend_details(start.clone(), end.clone(), status.map(str::to_string))
                .unwrap();
            assert_eq!(details.len(), expected);
            if let Some(status) = status {
                assert!(details.iter().all(|item| item.result_status == status));
            }
        }
        let completed = db
            .statistics_trend_details(start.clone(), end.clone(), Some("completed".into()))
            .unwrap();
        assert_eq!(completed[0].task_id, first.id);
        assert_eq!(completed[0].handling_count, 2);
        assert_eq!(db.get_task(first.id).unwrap().status, "pending");
        assert_eq!(
            db.statistics_details(start.clone(), end.clone(), "任务处理".into())
                .unwrap()
                .len(),
            3
        );
        let next = db
            .statistics_trend_details(
                end.clone(),
                "2026-09-23T00:00:00+08:00".into(),
                Some("completed".into()),
            )
            .unwrap();
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].task_id, second.id);
        let weekly = db
            .statistics(start.clone(), "2026-12-01T00:00:00+08:00".into(), 480)
            .unwrap();
        assert_eq!(weekly.trend_granularity, "week");
        assert_eq!(
            (
                weekly.trend[0].handled_tasks,
                weekly.trend[0].processed,
                weekly.trend[0].completed
            ),
            (3, 0, 2)
        );
        assert_eq!(
            db.statistics_trend_details(
                start.clone(),
                "2026-09-28T00:00:00+08:00".into(),
                Some("completed".into())
            )
            .unwrap()
            .len(),
            2
        );
        assert!(db
            .statistics_trend_details(start, end, Some("pending".into()))
            .is_err());
        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn trend_copy_eligibility_uses_any_valid_result_within_the_selected_range() {
        let root = std::env::temp_dir().join(format!(
            "inline-trend-copy-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        let mut input = sample("处理后等待");
        input.task_type = "法律咨询".into();
        input.departments = vec!["产品组".into(), "业务组".into()];
        let eligible = db.save_task(input).unwrap();
        let waiting = db.save_task(sample("仅等待")).unwrap();
        let outside = db.save_task(sample("范围外处理")).unwrap();
        let voided = db.save_task(sample("已作废处理后等待")).unwrap();
        let deleted = db.save_task(sample("回收站")).unwrap();
        let weekly = db.save_task(sample("同周前日完成")).unwrap();
        let end_boundary = db.save_task(sample("下一周")).unwrap();
        db.with_conn(|connection| {
            for (id, status, at) in [
                (eligible.id,"processed","2026-09-22T00:00:00+08:00"),
                (eligible.id,"completed","2026-09-22T08:00:00+08:00"),
                (eligible.id,"waiting_confirmation","2026-09-22T10:00:00+08:00"),
                (waiting.id,"waiting_materials","2026-09-22T10:00:00+08:00"),
                (outside.id,"processed","2026-09-20T23:59:59+08:00"),
                (outside.id,"waiting_materials","2026-09-22T10:00:00+08:00"),
                (voided.id,"processed","2026-09-22T08:00:00+08:00"),
                (voided.id,"waiting_materials","2026-09-22T10:00:00+08:00"),
                (deleted.id,"completed","2026-09-22T10:00:00+08:00"),
                (weekly.id,"completed","2026-09-21T09:00:00+08:00"),
                (weekly.id,"waiting_materials","2026-09-22T10:00:00+08:00"),
                (end_boundary.id,"completed","2026-09-28T00:00:00+08:00"),
            ] {
                record_work_event_on(connection,id,status,at,"任务处理","quick_action","")?;
            }
            connection.execute("UPDATE task_work_events SET voided_at=? WHERE task_id=? AND result_status='processed'",params![now(),voided.id]).map_err(display_error)?;
            Ok(())
        }).unwrap();
        db.soft_delete(deleted.id).unwrap();
        let day = db
            .statistics_trend_details(
                "2026-09-22T00:00:00+08:00".into(),
                "2026-09-23T00:00:00+08:00".into(),
                None,
            )
            .unwrap();
        assert_eq!(day.len(), 5);
        let copy: Vec<_> = day
            .iter()
            .filter(|item| item.has_processed_or_completed)
            .collect();
        assert_eq!(copy.len(), 1);
        assert_eq!(copy[0].task_id, eligible.id);
        assert_eq!(copy[0].task_type, "法律咨询");
        assert_eq!(copy[0].department, "产品组、业务组");
        assert_eq!(copy[0].result_status, "waiting_confirmation");
        assert_eq!(copy[0].handling_count, 3);
        assert_eq!(db.get_task(eligible.id).unwrap().status, "pending");
        let week = db
            .statistics_trend_details(
                "2026-09-21T00:00:00+08:00".into(),
                "2026-09-28T00:00:00+08:00".into(),
                None,
            )
            .unwrap();
        let mut ids: Vec<_> = week
            .iter()
            .filter(|item| item.has_processed_or_completed)
            .map(|item| item.task_id)
            .collect();
        ids.sort();
        let mut expected = vec![eligible.id, weekly.id];
        expected.sort();
        assert_eq!(ids, expected);
        assert!(!week
            .iter()
            .any(|item| item.task_id == deleted.id || item.task_id == end_boundary.id));
        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn statistics_distinguish_top_level_tasks_and_subtasks() {
        let root = std::env::temp_dir().join(format!(
            "inline-statistics-structure-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();

        let parent = db.save_task(sample("统计父任务")).unwrap();
        let child = db
            .create_subtask(subtask_sample(parent.id, "统计子任务"))
            .unwrap();
        db.process_round(parent.id).unwrap();
        db.process_round(child.id).unwrap();

        let start = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let end = (Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        let statistics = db.statistics(start, end, 480).unwrap();
        assert_eq!(statistics.summary.handled_tasks, 2);
        assert_eq!(statistics.summary.top_level_tasks, 1);
        assert_eq!(statistics.summary.subtasks, 1);

        drop(db);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn migration_to_v8_backfills_queue_events_and_clears_handled_urgency() {
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
        assert_eq!(migrated.with_conn(Database::schema_version).unwrap(), 9);
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
        let migrated_calendar = migrated
            .work_calendar(
                (Utc::now() - chrono::Duration::days(1)).to_rfc3339(),
                (Utc::now() + chrono::Duration::days(1)).to_rfc3339(),
            )
            .unwrap();
        assert_eq!(migrated_calendar.summary.handling_rounds, 1);
        assert!(migrated_calendar
            .tasks
            .iter()
            .any(|row| row.task_id == created.id && row.intervals[0].round_index == 1));

        drop(migrated);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn backup_restore_maps_parent_ids_in_two_passes_without_overwriting_current_relation() {
        let nonce = Utc::now().timestamp_nanos_opt().unwrap();
        let source_root = std::env::temp_dir().join(format!("inline-relation-source-{nonce}"));
        let target_root = std::env::temp_dir().join(format!("inline-relation-target-{nonce}"));
        fs::create_dir_all(&source_root).unwrap();
        fs::create_dir_all(&target_root).unwrap();

        let source = Database::open_at(source_root.join("inline.db")).unwrap();
        let source_parent = source.save_task(sample("备份父任务")).unwrap();
        let source_first = source.save_task(sample("共享子任务")).unwrap();
        let source_second = source.save_task(sample("仅备份子任务")).unwrap();
        source
            .set_parent_task(source_first.id, Some(source_parent.id))
            .unwrap();
        source
            .set_parent_task(source_second.id, Some(source_parent.id))
            .unwrap();
        source
            .reorder_subtasks(ReorderSubtasksInput {
                parent_task_id: source_parent.id,
                task_ids: vec![source_second.id, source_first.id],
            })
            .unwrap();
        let source_backup = source.create_backup("manual").unwrap();
        drop(source);

        let target = Database::open_at(target_root.join("inline.db")).unwrap();
        let target_parent = target.save_task(sample("备份父任务")).unwrap();
        let current_parent = target.save_task(sample("现库所属任务")).unwrap();
        let target_first = target.save_task(sample("共享子任务")).unwrap();
        target
            .set_parent_task(target_first.id, Some(current_parent.id))
            .unwrap();

        let imported = target.import_backup(source_backup.path).unwrap();
        let result = target.restore_backup(imported.path).unwrap();
        assert_eq!(result.added_tasks, 1);
        assert_eq!(result.merged_tasks, 2);
        assert_eq!(
            target.get_task(target_first.id).unwrap().parent_task_id,
            Some(current_parent.id),
            "现库已有关系必须优先"
        );
        let imported_child = target
            .list_tasks(TaskView::Queue)
            .unwrap()
            .into_iter()
            .find(|task| task.title == "仅备份子任务")
            .unwrap();
        assert_eq!(imported_child.parent_task_id, Some(target_parent.id));
        assert_eq!(imported_child.subtask_sort_order, 1);
        assert_eq!(
            target
                .list_subtasks(target_parent.id)
                .unwrap()
                .iter()
                .map(|task| task.id)
                .collect::<Vec<_>>(),
            vec![imported_child.id]
        );

        drop(target);
        let _ = fs::remove_dir_all(source_root);
        let _ = fs::remove_dir_all(target_root);
    }

    #[test]
    fn backup_restore_merges_tasks_preserves_conflicts_and_applies_settings() {
        let nonce = Utc::now().timestamp_nanos_opt().unwrap();
        let source_root = std::env::temp_dir().join(format!("inline-merge-source-{nonce}"));
        let target_root = std::env::temp_dir().join(format!("inline-merge-target-{nonce}"));
        fs::create_dir_all(&source_root).unwrap();
        fs::create_dir_all(&target_root).unwrap();

        let source = Database::open_at(source_root.join("inline.db")).unwrap();
        source
            .set_setting("week_start_day".into(), "sunday".into())
            .unwrap();
        source
            .set_setting("launch_at_login".into(), "true".into())
            .unwrap();
        // Valid selection can be absent on a different device: storage survives,
        // runtime resolver falls back without destructive configuration rewriting.
        source
            .set_setting("ui_font_family".into(), "Cross-device Missing Font".into())
            .unwrap();
        source.save_task(sample("完全一致事项")).unwrap();
        source
            .with_conn(|connection| {
                connection
                    .execute(
                        "UPDATE tasks SET task_type=' 任务处理 ' WHERE title='完全一致事项'",
                        [],
                    )
                    .map_err(display_error)?;
                connection
                    .execute(
                        "INSERT INTO master_values(kind,name,sort_order,is_active) VALUES('task_type',' 任务处理 ',998,1)",
                        [],
                    )
                    .map_err(display_error)?;
                Ok(())
            })
            .unwrap();
        let mut source_conflict = sample("同名事项");
        source_conflict.details = "备份中的详情更多".into();
        source.save_task(source_conflict).unwrap();
        source.save_task(sample("仅备份中存在")).unwrap();
        let source_backup = source.create_backup("manual").unwrap();
        drop(source);

        let target = Database::open_at(target_root.join("inline.db")).unwrap();
        target
            .set_setting("week_start_day".into(), "monday".into())
            .unwrap();
        let identical = target.save_task(sample("完全一致事项")).unwrap();
        let mut target_conflict = sample("同名事项");
        target_conflict.details = "当前数据中的详情".into();
        target.save_task(target_conflict).unwrap();

        let imported = target.import_backup(source_backup.path).unwrap();
        assert!(imported.name.contains("-import"));
        let result = target.restore_backup(imported.path).unwrap();
        assert_eq!(result.added_tasks, 2);
        assert_eq!(result.merged_tasks, 1);
        assert_eq!(result.conflict_tasks, 1);
        assert_eq!(result.applied_settings, 3);
        assert_eq!(result.conflicts.len(), 1);

        let all = [
            target.list_tasks(TaskView::Queue).unwrap(),
            target.list_tasks(TaskView::Archive).unwrap(),
            target.list_tasks(TaskView::Trash).unwrap(),
        ]
        .concat();
        assert_eq!(all.len(), 4);
        assert!(all.iter().any(|task| task.title == "同名事项"));
        assert!(all.iter().any(|task| task.title == "同名事项（冲突）"));
        assert!(
            !all.iter()
                .find(|task| task.title == "同名事项")
                .unwrap()
                .is_import_conflict
        );
        let imported_conflict = all
            .iter()
            .find(|task| task.title == "同名事项（冲突）")
            .unwrap();
        assert!(imported_conflict.is_import_conflict);
        assert_eq!(result.conflicts[0].task_id, imported_conflict.id);
        target
            .resolve_import_conflict(imported_conflict.id)
            .unwrap();
        assert!(
            !target
                .get_task(imported_conflict.id)
                .unwrap()
                .is_import_conflict
        );
        assert!(all.iter().any(|task| task.title == "仅备份中存在"));
        assert!(all
            .iter()
            .all(|task| task.task_type == task.task_type.trim()));
        assert_eq!(
            target
                .masters()
                .unwrap()
                .task_types
                .iter()
                .filter(|name| name.trim() == "任务处理")
                .count(),
            1
        );
        assert_eq!(
            target
                .settings()
                .unwrap()
                .get("week_start_day")
                .map(String::as_str),
            Some("sunday")
        );
        assert_eq!(
            target
                .settings()
                .unwrap()
                .get("launch_at_login")
                .map(String::as_str),
            Some("true")
        );
        assert!(target.get_logs(identical.id).unwrap().len() >= 2);
        assert_eq!(
            target
                .settings()
                .unwrap()
                .get("ui_font_family")
                .map(String::as_str),
            Some("Cross-device Missing Font")
        );
        drop(target);
        let target = Database::open_at(target_root.join("inline.db")).unwrap();
        assert_eq!(
            target
                .settings()
                .unwrap()
                .get("ui_font_family")
                .map(String::as_str),
            Some("Cross-device Missing Font")
        );
        assert!(target
            .list_backups()
            .unwrap()
            .iter()
            .any(|backup| backup.name.contains("before-restore")));

        drop(target);
        let _ = fs::remove_dir_all(source_root);
        let _ = fs::remove_dir_all(target_root);
    }

    #[test]
    fn create_subtask_inherits_defaults_but_keeps_queue_deadline_and_urgency_independent() {
        let _clock = TestClock::at("2026-10-01T08:00:00+08:00");
        let root = std::env::temp_dir().join(format!(
            "inline-create-subtask-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        let mut parent_input = sample("所属任务");
        parent_input.departments = vec!["产品组".into(), "法务组".into()];
        parent_input.contacts = vec!["小林".into(), "小周".into()];
        parent_input.task_type = "沟通协调".into();
        parent_input.is_urgent = true;
        parent_input.urgent_requester = "负责人".into();
        parent_input.urgent_reason = "父任务加急".into();
        parent_input.requested_deadline = Some("2026-10-01T09:00:00+08:00".into());
        let parent = db.save_task(parent_input).unwrap();

        let mut first_input = subtask_sample(parent.id, "默认继承子任务");
        first_input.enqueue_today = false;
        let first = db.create_subtask(first_input).unwrap();
        assert_eq!(first.parent_task_id, Some(parent.id));
        assert_eq!(first.subtask_sort_order, 1);
        assert_eq!(first.departments, parent.departments);
        assert_eq!(first.contacts, parent.contacts);
        assert_eq!(first.task_type, parent.task_type);
        assert_eq!(first.priority, "normal");
        assert_eq!(first.workload, "standard");
        assert!(!first.is_urgent);
        assert_eq!(first.requested_deadline, None);
        assert!(!first.has_active_queue);

        let mut second_input = subtask_sample(parent.id, "独立配置子任务");
        second_input.task_type = Some("文本起草".into());
        second_input.departments = Some(vec!["外部团队".into()]);
        second_input.contacts = Some(vec!["小郑".into()]);
        second_input.priority = Some("critical".into());
        second_input.workload = Some("major".into());
        second_input.is_urgent = true;
        second_input.urgent_requester = "小郑".into();
        second_input.urgent_reason = "单独加急".into();
        second_input.requested_deadline = Some("2026-10-02T18:00:00+08:00".into());
        second_input.requested_deadline_label = Some("独立期限".into());
        let second = db.create_subtask(second_input).unwrap();
        assert_eq!(second.subtask_sort_order, 2);
        assert_eq!(second.departments, vec!["外部团队"]);
        assert_eq!(second.contacts, vec!["小郑"]);
        assert_eq!(second.task_type, "文本起草");
        assert_eq!(second.priority, "critical");
        assert_eq!(second.workload, "major");
        assert!(second.is_urgent);
        assert!(second.has_active_queue);
        assert_eq!(
            second.requested_deadline.as_deref(),
            Some("2026-10-02T18:00:00+08:00")
        );
        assert!(db
            .create_subtask(subtask_sample(first.id, "非法第三级"))
            .is_err());
        assert!(db
            .get_logs(first.id)
            .unwrap()
            .iter()
            .any(|log| log.log_type == "relation"));

        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn completion_requires_an_explicit_parent_choice_and_reports_aggregate_state() {
        let root = std::env::temp_dir().join(format!(
            "inline-subtask-completion-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();

        let parent_only = db.save_task(sample("仅完成父任务")).unwrap();
        let child = db
            .create_subtask(subtask_sample(parent_only.id, "仍待处理子任务"))
            .unwrap();
        assert!(db.complete_round(parent_only.id).is_err());
        assert!(db.set_status(parent_only.id, "completed".into()).is_err());
        let mut edited_parent = sample("仅完成父任务");
        edited_parent.id = Some(parent_only.id);
        edited_parent.status = "completed".into();
        assert!(db.save_task(edited_parent).is_err());
        assert!(db
            .record_work_event(WorkEventInput {
                task_id: parent_only.id,
                result_status: "completed".into(),
                handled_at: now(),
                note: "直接补录完成".into(),
                sync_status: true,
            })
            .is_err());
        let parent_only_result = db
            .complete_task(CompleteTaskInput {
                task_id: parent_only.id,
                include_eligible_subtasks: false,
            })
            .unwrap();
        assert_eq!(parent_only_result.completed_task_ids, vec![parent_only.id]);
        assert_eq!(db.get_task(child.id).unwrap().status, "pending");

        let parent = db.save_task(sample("批量完成父任务")).unwrap();
        let active = db
            .create_subtask(subtask_sample(parent.id, "有效待办"))
            .unwrap();
        let deferred = db
            .create_subtask(subtask_sample(parent.id, "有效暂缓"))
            .unwrap();
        db.set_status(deferred.id, "waiting_materials".into())
            .unwrap();
        let completed = db
            .create_subtask(subtask_sample(parent.id, "已完成"))
            .unwrap();
        db.complete_round(completed.id).unwrap();
        let cancelled = db
            .create_subtask(subtask_sample(parent.id, "已取消"))
            .unwrap();
        db.set_status(cancelled.id, "cancelled".into()).unwrap();
        let archived = db
            .create_subtask(subtask_sample(parent.id, "已归档"))
            .unwrap();
        db.archive(archived.id).unwrap();
        let deleted = db
            .create_subtask(subtask_sample(parent.id, "已删除"))
            .unwrap();
        db.soft_delete(deleted.id).unwrap();

        let before = db.subtask_completion_state(parent.id).unwrap().unwrap();
        assert_eq!(before.total_subtasks, 6);
        assert_eq!(before.completed_subtasks, 1);
        assert_eq!(before.eligible_subtasks, 3);
        assert_eq!(before.completed_eligible_subtasks, 1);
        assert!(!before.all_eligible_subtasks_completed);
        assert!(before.parent_can_be_completed);

        let result = db
            .complete_task(CompleteTaskInput {
                task_id: parent.id,
                include_eligible_subtasks: true,
            })
            .unwrap();
        assert_eq!(
            result.completed_task_ids,
            vec![active.id, deferred.id, parent.id]
        );
        assert_eq!(db.get_task(active.id).unwrap().status, "completed");
        assert_eq!(db.get_task(deferred.id).unwrap().status, "completed");
        assert_eq!(db.get_task(completed.id).unwrap().status, "completed");
        assert_eq!(db.get_task(cancelled.id).unwrap().status, "cancelled");
        assert_eq!(db.get_task(archived.id).unwrap().status, "archived");
        assert!(db.get_task(deleted.id).unwrap().deleted_at.is_some());
        let after = result.completion_state.unwrap();
        assert!(after.all_eligible_subtasks_completed);
        assert!(!after.parent_can_be_completed);

        let prompt_parent = db.save_task(sample("完成提示父任务")).unwrap();
        let prompt_child = db
            .create_subtask(subtask_sample(prompt_parent.id, "最后一个子任务"))
            .unwrap();
        let prompt = db
            .complete_task(CompleteTaskInput {
                task_id: prompt_child.id,
                include_eligible_subtasks: false,
            })
            .unwrap()
            .completion_state
            .unwrap();
        assert_eq!(prompt.parent_task_id, prompt_parent.id);
        assert!(prompt.all_eligible_subtasks_completed);
        assert!(prompt.parent_can_be_completed);

        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_delete_restore_and_permanent_cleanup_apply_only_the_selected_scope() {
        let root = std::env::temp_dir().join(format!(
            "inline-subtask-lifecycle-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();

        let archive_parent = db.save_task(sample("归档父任务")).unwrap();
        let completed_child = db
            .create_subtask(subtask_sample(archive_parent.id, "已完成子任务"))
            .unwrap();
        db.complete_round(completed_child.id).unwrap();
        let active_child = db
            .create_subtask(subtask_sample(archive_parent.id, "未完成子任务"))
            .unwrap();
        let archived = db
            .archive_task_group(ArchiveTaskInput {
                task_id: archive_parent.id,
                include_completed_subtasks: true,
            })
            .unwrap();
        assert_eq!(
            archived.archived_task_ids,
            vec![completed_child.id, archive_parent.id]
        );
        assert_eq!(db.get_task(active_child.id).unwrap().status, "pending");
        assert_eq!(
            db.get_task(active_child.id).unwrap().parent_task_id,
            Some(archive_parent.id)
        );

        let detach_parent = db.save_task(sample("仅删除父任务")).unwrap();
        let detached_child = db
            .create_subtask(subtask_sample(detach_parent.id, "保留子任务"))
            .unwrap();
        let deleted = db
            .delete_task_group(DeleteTaskInput {
                task_id: detach_parent.id,
                include_subtasks: false,
            })
            .unwrap();
        assert_eq!(deleted.trashed_task_ids, vec![detach_parent.id]);
        assert_eq!(deleted.detached_subtask_ids, vec![detached_child.id]);
        let detached = db.get_task(detached_child.id).unwrap();
        assert_eq!(detached.parent_task_id, None);
        assert_eq!(detached.subtask_sort_order, 0);
        assert!(detached.deleted_at.is_none());

        let grouped_parent = db.save_task(sample("整组删除父任务")).unwrap();
        let grouped_child = db
            .create_subtask(subtask_sample(grouped_parent.id, "整组删除子任务"))
            .unwrap();
        let grouped = db
            .delete_task_group(DeleteTaskInput {
                task_id: grouped_parent.id,
                include_subtasks: true,
            })
            .unwrap();
        assert_eq!(
            grouped.trashed_task_ids,
            vec![grouped_child.id, grouped_parent.id]
        );
        assert!(grouped.detached_subtask_ids.is_empty());
        assert_eq!(
            db.get_task(grouped_child.id).unwrap().parent_task_id,
            Some(grouped_parent.id)
        );
        db.restore(grouped_child.id).unwrap();
        assert!(db.get_task(grouped_parent.id).unwrap().deleted_at.is_some());
        assert_eq!(
            db.get_task(grouped_child.id).unwrap().parent_task_id,
            Some(grouped_parent.id)
        );
        db.permanently_delete_tasks(vec![grouped_parent.id])
            .unwrap();
        let cleaned_child = db.get_task(grouped_child.id).unwrap();
        assert_eq!(cleaned_child.parent_task_id, None);
        assert_eq!(cleaned_child.subtask_sort_order, 0);
        assert!(db
            .get_logs(grouped_child.id)
            .unwrap()
            .iter()
            .any(|log| log.content.contains("已永久删除")));

        let restore_parent = db.save_task(sample("仅恢复父任务")).unwrap();
        let still_trashed_child = db
            .create_subtask(subtask_sample(restore_parent.id, "仍在回收站的子任务"))
            .unwrap();
        db.delete_task_group(DeleteTaskInput {
            task_id: restore_parent.id,
            include_subtasks: true,
        })
        .unwrap();
        db.restore(restore_parent.id).unwrap();
        assert!(db.get_task(restore_parent.id).unwrap().deleted_at.is_none());
        assert!(db
            .get_task(still_trashed_child.id)
            .unwrap()
            .deleted_at
            .is_some());

        let empty_parent = db.save_task(sample("清空回收站父任务")).unwrap();
        let empty_restored_child = db
            .create_subtask(subtask_sample(empty_parent.id, "清空前恢复的子任务"))
            .unwrap();
        db.delete_task_group(DeleteTaskInput {
            task_id: empty_parent.id,
            include_subtasks: true,
        })
        .unwrap();
        db.restore(empty_restored_child.id).unwrap();
        db.empty_trash().unwrap();
        assert!(db.get_task(still_trashed_child.id).is_err());
        let empty_cleaned_child = db.get_task(empty_restored_child.id).unwrap();
        assert_eq!(empty_cleaned_child.parent_task_id, None);
        assert_eq!(empty_cleaned_child.subtask_sort_order, 0);
        assert!(db
            .get_logs(empty_restored_child.id)
            .unwrap()
            .iter()
            .any(|log| log.content.contains("从回收站永久删除")));

        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn merging_a_parent_appends_its_children_and_rejects_a_child_target() {
        let root = std::env::temp_dir().join(format!(
            "inline-subtask-merge-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("inline.db")).unwrap();
        let target = db.save_task(sample("合并目标")).unwrap();
        let existing = db
            .create_subtask(subtask_sample(target.id, "目标原子任务"))
            .unwrap();
        let source = db.save_task(sample("合并来源")).unwrap();
        let first = db
            .create_subtask(subtask_sample(source.id, "来源子任务一"))
            .unwrap();
        let second = db
            .create_subtask(subtask_sample(source.id, "来源子任务二"))
            .unwrap();
        db.merge_tasks(MergeTaskInput {
            target_task_id: target.id,
            source_task_id: source.id,
            deduplicate_records: true,
            trash_source: false,
        })
        .unwrap();
        let children = db.list_subtasks(target.id).unwrap();
        assert_eq!(
            children.iter().map(|task| task.id).collect::<Vec<_>>(),
            vec![existing.id, first.id, second.id]
        );
        assert_eq!(
            children
                .iter()
                .map(|task| task.subtask_sort_order)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(db.list_subtasks(source.id).unwrap().is_empty());
        assert_eq!(db.get_task(source.id).unwrap().status, "archived");
        assert!(db
            .get_logs(first.id)
            .unwrap()
            .iter()
            .any(|log| log.content.starts_with("所属任务因合并更换")));

        let blocked_source = db.save_task(sample("不可合并来源")).unwrap();
        let blocked_source_child = db
            .create_subtask(subtask_sample(blocked_source.id, "不可合并来源子任务"))
            .unwrap();
        let child_target_parent = db.save_task(sample("子目标所属任务")).unwrap();
        let child_target = db
            .create_subtask(subtask_sample(child_target_parent.id, "非法子目标"))
            .unwrap();
        assert!(db
            .merge_tasks(MergeTaskInput {
                target_task_id: child_target.id,
                source_task_id: blocked_source.id,
                deduplicate_records: false,
                trash_source: true,
            })
            .is_err());
        assert_eq!(db.get_task(blocked_source.id).unwrap().status, "pending");
        assert_eq!(
            db.get_task(blocked_source_child.id).unwrap().parent_task_id,
            Some(blocked_source.id)
        );

        drop(db);
        let _ = fs::remove_dir_all(root);
    }
}
