use crate::models::*;
use chrono::{Local, Utc};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

const SELECT_TASK: &str = "SELECT id, permanent_number, daily_sequence, ticket_date, department, contact, task_type, title, details, status, priority, workload, is_urgent, urgent_requester, urgent_reason, requested_deadline, internal_notes, created_at, updated_at, started_at, completed_at, archived_at, deleted_at, custom_sort_order, requested_deadline_label FROM tasks";

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
        if existed && Self::schema_version(&connection)? < 3 {
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
               sort_order INTEGER NOT NULL DEFAULT 0, is_active INTEGER NOT NULL DEFAULT 1, UNIQUE(kind,name));
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
        transaction
            .execute(
                "INSERT OR IGNORE INTO master_values(kind,name,sort_order,is_active)
             SELECT 'contact',contact,999,1 FROM tasks WHERE trim(contact)<>''",
                [],
            )
            .map_err(display_error)?;
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
        Ok(LegalTask {
            id: row.get(0)?,
            permanent_number: row.get(1)?,
            daily_sequence: row.get(2)?,
            ticket_date: row.get(3)?,
            department: row.get(4)?,
            contact: row.get(5)?,
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
        })
    }

    pub fn list_tasks(&self, view: TaskView) -> Result<Vec<LegalTask>, String> {
        self.with_conn(|connection| {
            let condition = match view {
                TaskView::Queue => "deleted_at IS NULL AND archived_at IS NULL AND status NOT IN ('completed','cancelled','archived')",
                TaskView::Archive => "deleted_at IS NULL AND (archived_at IS NOT NULL OR status IN ('completed','cancelled','archived'))",
                TaskView::Trash => "deleted_at IS NOT NULL",
            };
            let order = if matches!(view, TaskView::Queue) { "custom_sort_order ASC,id ASC" } else { "updated_at DESC" };
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

fn now() -> String {
    Utc::now().to_rfc3339()
}
fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}
fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

impl Database {
    pub fn save_task(&self, input: TaskInput) -> Result<LegalTask, String> {
        validate_task_input(&input)?;
        let mut guard = self
            .connection
            .lock()
            .map_err(|_| "数据库正忙，请稍后重试".to_string())?;
        let connection = guard.as_mut().ok_or("数据库尚未打开")?;
        let transaction = connection.transaction().map_err(display_error)?;
        let stamp = now();
        let id = if let Some(id) = input.id {
            let previous = get_task_on(&transaction, id)?;
            let started = if input.status == "processing" && previous.started_at.is_none() {
                Some(stamp.clone())
            } else {
                previous.started_at
            };
            let completed = if input.status == "completed" {
                previous.completed_at.or_else(|| Some(stamp.clone()))
            } else {
                None
            };
            transaction.execute(
                "UPDATE tasks SET department=?,contact=?,task_type=?,title=?,details=?,status=?,priority=?,workload=?,
                 is_urgent=?,urgent_requester=?,urgent_reason=?,requested_deadline=?,requested_deadline_label=?,internal_notes=?,updated_at=?,
                 started_at=?,completed_at=? WHERE id=?",
                params![input.department.trim(),input.contact.trim(),input.task_type.trim(),input.title.trim(),
                input.details.trim(),input.status,input.priority,input.workload,input.is_urgent as i64,
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
            }
            if previous.is_urgent != input.is_urgent {
                record_urgent(&transaction, id, &input, previous.is_urgent)?;
                if input.is_urgent {
                    promote_one(&transaction, id)?;
                }
            }
            add_log(&transaction, id, "updated", "更新事项信息")?;
            id
        } else {
            let date = today();
            let sequence: i64 = transaction
                .query_row(
                    "SELECT last_sequence FROM daily_sequences WHERE ticket_date=?",
                    [&date],
                    |row| row.get(0),
                )
                .optional()
                .map_err(display_error)?
                .unwrap_or(0)
                + 1;
            transaction
                .execute(
                    "INSERT INTO daily_sequences(ticket_date,last_sequence) VALUES(?,?)
                 ON CONFLICT(ticket_date) DO UPDATE SET last_sequence=excluded.last_sequence",
                    params![date, sequence],
                )
                .map_err(display_error)?;
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
                params![permanent,sequence,date,input.department.trim(),input.contact.trim(),input.task_type.trim(),
                input.title.trim(),input.details.trim(),input.status,input.priority,input.workload,input.is_urgent as i64,
                input.urgent_requester.trim(),input.urgent_reason.trim(),input.requested_deadline,input.requested_deadline_label,input.internal_notes.trim(),
                stamp,stamp,if input.status=="processing"{Some(now())}else{None},if input.status=="completed"{Some(now())}else{None},order]
            ).map_err(display_error)?;
            let id = transaction.last_insert_rowid();
            add_log(
                &transaction,
                id,
                "created",
                &format!("创建事项并取号：{permanent}"),
            )?;
            add_status(&transaction, id, None, &input.status, "创建事项")?;
            if input.is_urgent {
                record_urgent(&transaction, id, &input, false)?;
                promote_one(&transaction, id)?;
            }
            id
        };
        ensure_master(&transaction, "department", &input.department)?;
        ensure_master(&transaction, "task_type", &input.task_type)?;
        ensure_master(&transaction, "contact", &input.contact)?;
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
            add_log(transaction, id, "status", &format!("状态变更为：{status}"))
        })
    }

    pub fn move_task(&self, id: i64, direction: MoveDirection) -> Result<(), String> {
        self.with_transaction(|transaction| {
            let task = get_task_on(transaction,id)?;
            let comparison = if matches!(direction,MoveDirection::Up){"<"}else{">"};
            let order = if matches!(direction,MoveDirection::Up){"DESC"}else{"ASC"};
            let sql = format!("SELECT id,custom_sort_order FROM tasks WHERE deleted_at IS NULL AND archived_at IS NULL
                AND status NOT IN ('completed','cancelled','archived') AND custom_sort_order {comparison} ?
                ORDER BY custom_sort_order {order},id {order} LIMIT 1");
            let adjacent: Option<(i64,i64)> = transaction.query_row(&sql,[task.custom_sort_order],|row|Ok((row.get(0)?,row.get(1)?)))
                .optional().map_err(display_error)?;
            if let Some((other_id,other_order))=adjacent {
                transaction.execute("UPDATE tasks SET custom_sort_order=? WHERE id=?",params![other_order,id]).map_err(display_error)?;
                transaction.execute("UPDATE tasks SET custom_sort_order=? WHERE id=?",params![task.custom_sort_order,other_id]).map_err(display_error)?;
            }
            Ok(())
        })
    }

    pub fn soft_delete(&self, id: i64) -> Result<(), String> {
        self.simple_change(
            id,
            "UPDATE tasks SET deleted_at=?,updated_at=? WHERE id=?",
            "deleted",
            "事项移入回收站",
        )
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
            add_status(tx, id, Some(&old.status), "archived", "归档事项")?;
            add_log(tx, id, "archived", "事项已归档")
        })
    }
    pub fn restore(&self, id: i64) -> Result<(), String> {
        self.with_transaction(|tx|{
            let task=get_task_on(tx,id)?;
            let order:i64=tx.query_row("SELECT COALESCE(MAX(custom_sort_order),0)+1 FROM tasks",[],|row|row.get(0)).map_err(display_error)?;
            let status=if task.status=="archived"||task.status=="completed"||task.status=="cancelled"{"pending"}else{&task.status};
            tx.execute("UPDATE tasks SET deleted_at=NULL,archived_at=NULL,status=?,custom_sort_order=?,updated_at=? WHERE id=?",
                params![status,order,now(),id]).map_err(display_error)?;
            add_log(tx,id,"restored","事项已恢复并排到队尾")
        })
    }

    fn simple_change(&self, id: i64, sql: &str, kind: &str, content: &str) -> Result<(), String> {
        self.with_transaction(|tx| {
            get_task_on(tx, id)?;
            let stamp = now();
            tx.execute(sql, params![stamp, stamp, id])
                .map_err(display_error)?;
            add_log(tx, id, kind, content)
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
             AND status NOT IN ('completed','cancelled','archived')",
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
             AND status NOT IN ('completed','cancelled','archived') AND id<>? AND custom_sort_order<?
             ORDER BY custom_sort_order DESC,id DESC LIMIT 1",
            params![id, order],
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
fn record_urgent(
    connection: &Connection,
    id: i64,
    input: &TaskInput,
    was_urgent: bool,
) -> Result<(), String> {
    if input.is_urgent {
        connection.execute("INSERT INTO urgent_records(task_id,requester,reason,requested_deadline,requested_at,confirmation_status,confirmed_at)
            VALUES(?,?,?,?,?,'confirmed',?)",params![id,input.urgent_requester.trim(),input.urgent_reason.trim(),input.requested_deadline,now(),now()]).map_err(display_error)?;
        add_log(
            connection,
            id,
            "urgent",
            &format!("标记加急：{}", input.urgent_requester.trim()),
        )
    } else if was_urgent {
        connection
            .execute(
                "UPDATE urgent_records SET cancelled_at=? WHERE task_id=? AND cancelled_at IS NULL",
                params![now(), id],
            )
            .map_err(display_error)?;
        add_log(connection, id, "urgent", "取消加急")
    } else {
        Ok(())
    }
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
            let mut statement=connection.prepare("SELECT kind,name FROM master_values WHERE is_active=1 ORDER BY sort_order,name").map_err(display_error)?;
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
    pub fn queue_ahead(&self, id: i64) -> Result<i64, String> {
        self.with_conn(|connection| {
            let task = get_task_on(connection, id)?;
            connection
                .query_row(
                    "SELECT count(*) FROM tasks WHERE deleted_at IS NULL AND archived_at IS NULL
                     AND status NOT IN ('completed','cancelled','archived')
                     AND (custom_sort_order < ? OR (custom_sort_order = ? AND id < ?))",
                    params![task.custom_sort_order, task.custom_sort_order, id],
                    |row| row.get(0),
                )
                .map_err(display_error)
        })
    }
    pub fn ticket_snapshot(&self, id: i64) -> Result<TicketSnapshot, String> {
        self.with_conn(|connection| {
            let task = get_task_on(connection, id)?;
            let queue_ahead = connection
                .query_row(
                    "SELECT count(*) FROM tasks WHERE deleted_at IS NULL AND archived_at IS NULL
                     AND status NOT IN ('completed','cancelled','archived')
                     AND (custom_sort_order < ? OR (custom_sort_order = ? AND id < ?))",
                    params![task.custom_sort_order, task.custom_sort_order, id],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            let queue_total = connection
                .query_row(
                    "SELECT count(*) FROM tasks WHERE deleted_at IS NULL AND archived_at IS NULL
                     AND status NOT IN ('completed','cancelled','archived')",
                    [],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            Ok(TicketSnapshot {
                task,
                queue_ahead,
                queue_total,
            })
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
            contact: "小林".into(),
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
        assert_eq!(snapshot.queue_total, 3);
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
}
