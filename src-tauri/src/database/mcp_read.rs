//! Copies a consistent SQLite image into bounded memory. No production writes
//! or database locks survive a request; subsequent pages use the frozen image.
use super::*;
use crate::mcp::{basis::DataVersion, contract::McpError, scope::Scope};

pub(super) fn migrate_basis(tx: &Transaction<'_>, version: i64) -> Result<(), String> {
    if version >= 11 {
        return Ok(());
    }
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS mcp_data_basis(
        singleton INTEGER PRIMARY KEY CHECK(singleton=1), database_uuid TEXT NOT NULL,
        data_generation INTEGER NOT NULL, commit_sequence INTEGER NOT NULL);
        INSERT OR IGNORE INTO mcp_data_basis VALUES(1,lower(hex(randomblob(32))),1,0);",
    )
    .map_err(display_error)?;
    // The sequence orders committed row mutations, not the number of transactions.
    // Triggers run inside every existing GUI/import/domain transaction and roll back with it.
    for table in [
        "tasks",
        "task_logs",
        "status_history",
        "urgent_records",
        "task_queue_entries",
        "task_work_events",
        "daily_sequences",
        "master_values",
        "settings",
    ] {
        for action in ["INSERT", "UPDATE", "DELETE"] {
            tx.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS mcp_basis_{table}_{action} AFTER {action} ON {table}
                BEGIN UPDATE mcp_data_basis SET commit_sequence=commit_sequence+1 WHERE singleton=1; END;"))
                .map_err(display_error)?;
        }
    }
    tx.execute_batch("DELETE FROM schema_meta; INSERT INTO schema_meta VALUES(11);")
        .map_err(display_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn root() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "inline-p2-basis-{}",
            crate::mcp::platform::random_secret().unwrap()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
    fn input() -> TaskInput {
        serde_json::from_value(json!({"title":"synthetic","department":"A","departments":["A"],"contact":"fixture","contacts":["fixture"],"taskType":"T","details":"","status":"pending","priority":"normal","workload":"standard","isUrgent":false,"urgentRequester":"","urgentReason":"","internalNotes":""})).unwrap()
    }
    #[test]
    fn basis_changes_with_gui_writes_but_rolls_back_atomically_and_reads_never_write() {
        let db = Database::open_at(root().join("inline.db")).unwrap();
        let before = db.mcp_basis().unwrap();
        let task = db.save_task(input()).unwrap();
        let after = db.mcp_basis().unwrap();
        assert_eq!(before.database_uuid, after.database_uuid);
        assert!(after.commit_sequence > before.commit_sequence);
        db.with_conn(|conn| {
            conn.execute_batch("BEGIN IMMEDIATE;")
                .map_err(display_error)?;
            conn.execute("UPDATE tasks SET title='rolled-back' WHERE id=?", [task.id])
                .map_err(display_error)?;
            conn.execute_batch("ROLLBACK;").map_err(display_error)
        })
        .unwrap();
        assert_eq!(
            db.mcp_basis().unwrap().commit_sequence,
            after.commit_sequence
        );
        assert_eq!(db.get_task(task.id).unwrap().title, "synthetic");
        let copy = db.mcp_snapshot().unwrap();
        assert_eq!(
            copy.mcp_basis().unwrap().commit_sequence,
            after.commit_sequence
        );
        copy.mcp_keep_tasks(&[]).unwrap();
        assert_eq!(
            db.mcp_basis().unwrap().commit_sequence,
            after.commit_sequence
        );
        assert!(copy.mcp_tasks().unwrap().is_empty());
        assert_eq!(db.mcp_tasks().unwrap().len(), 1);
    }
    #[test]
    fn schema10_upgrade_preserves_business_identity_color_and_creates_backup() {
        let root = root();
        let db = Database::open_root(root.clone()).unwrap();
        let task = db.save_task(input()).unwrap();
        db.set_task_ticket_color(task.id, Some("#008800".into()))
            .unwrap();
        let original = serde_json::to_value(db.get_task(task.id).unwrap()).unwrap();
        db.with_conn(|conn|{for table in ["tasks","task_logs","status_history","urgent_records","task_queue_entries","task_work_events","daily_sequences","master_values","settings"]{for action in ["INSERT","UPDATE","DELETE"]{conn.execute_batch(&format!("DROP TRIGGER mcp_basis_{table}_{action};")).map_err(display_error)?;}}conn.execute_batch("DROP TABLE mcp_data_basis;DELETE FROM schema_meta;INSERT INTO schema_meta VALUES(10);").map_err(display_error)}).unwrap();
        drop(db);
        let upgraded = Database::open_root(root).unwrap();
        assert_eq!(
            serde_json::to_value(upgraded.get_task(task.id).unwrap()).unwrap(),
            original
        );
        assert_eq!(upgraded.mcp_basis().unwrap().schema_version, 11);
        assert!(!upgraded.list_backups().unwrap().is_empty());
    }
    #[test]
    fn oversized_snapshot_is_rejected_instead_of_truncating_history() {
        let db = Database::open_at(root().join("inline.db")).unwrap();
        let task = db.save_task(input()).unwrap();
        db.with_conn(|conn| {
            conn.execute(
                "UPDATE tasks SET details=hex(zeroblob(17*1024*1024)) WHERE id=?",
                [task.id],
            )
            .map_err(display_error)?;
            Ok(())
        })
        .unwrap();
        assert_eq!(db.mcp_snapshot().err().unwrap().code, "resource_limit");
        assert_eq!(db.mcp_tasks().unwrap().len(), 1);
    }
}

impl Database {
    pub(crate) fn mcp_size(&self) -> Result<usize, McpError> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT page_count*page_size FROM pragma_page_count(),pragma_page_size()",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n as usize)
            .map_err(display_error)
        })
        .map_err(Into::into)
    }
    pub(crate) fn mcp_basis(&self) -> Result<DataVersion, McpError> {
        self.with_conn(|conn| conn.query_row("SELECT database_uuid,data_generation,commit_sequence FROM mcp_data_basis WHERE singleton=1",[],|r|Ok(DataVersion{
            database_uuid:r.get(0)?,data_generation:r.get(1)?,commit_sequence:r.get(2)?,
            schema_version:11,query_schema_version:1,statistics_definition_version:1,
        })).map_err(display_error)).map_err(Into::into)
    }
    pub(crate) fn mcp_snapshot(&self) -> Result<Self, McpError> {
        self.with_conn(|conn| {
            let bytes: i64 = conn
                .query_row(
                    "SELECT page_count*page_size FROM pragma_page_count(),pragma_page_size()",
                    [],
                    |r| r.get(0),
                )
                .map_err(display_error)?;
            if bytes > 32 * 1024 * 1024 {
                return Err("mcp_resource_limit".into());
            }
            let mut copy = Connection::open_in_memory().map_err(display_error)?;
            let backup = rusqlite::backup::Backup::new(conn, &mut copy).map_err(display_error)?;
            backup
                .run_to_completion(256, std::time::Duration::ZERO, None)
                .map_err(display_error)?;
            drop(backup);
            copy.execute_batch("PRAGMA foreign_keys=ON;")
                .map_err(display_error)?;
            Ok(Self {
                backup_dir: PathBuf::new(),
                connection: Mutex::new(Some(copy)),
            })
        })
        .map_err(|e| {
            if e == "mcp_resource_limit" {
                McpError::new("resource_limit")
            } else {
                e.into()
            }
        })
    }
    pub(crate) fn mcp_tasks(&self) -> Result<Vec<LegalTask>, McpError> {
        self.with_conn(|conn| {
            let mut stmt = conn
                .prepare(&format!("{SELECT_TASK} ORDER BY tasks.id"))
                .map_err(display_error)?;
            let result = stmt
                .query_map([], Self::row_task)
                .map_err(display_error)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(display_error)?;
            Ok(result)
        })
        .map_err(Into::into)
    }
    pub(crate) fn mcp_keep_tasks(&self, ids: &[i64]) -> Result<(), McpError> {
        self.with_conn(|conn| {
            // This is an isolated image. Disabling FK actions preserves the
            // original parent IDs for statistics; all exposed records join
            // surviving tasks, so out-of-scope rows cannot enter results.
            conn.execute_batch("PRAGMA foreign_keys=OFF;")
                .map_err(display_error)?;
            conn.execute_batch("CREATE TEMP TABLE mcp_selected(id INTEGER PRIMARY KEY);")
                .map_err(display_error)?;
            for id in ids {
                conn.execute("INSERT INTO mcp_selected VALUES(?)", [id])
                    .map_err(display_error)?;
            }
            conn.execute(
                "DELETE FROM tasks WHERE id NOT IN (SELECT id FROM mcp_selected)",
                [],
            )
            .map_err(display_error)?;
            Ok(())
        })
        .map_err(Into::into)
    }
    pub(crate) fn mcp_in_scope(&self, ids: &[i64], scope: &Scope) -> Result<bool, McpError> {
        self.with_conn(|conn| {
            for id in ids {
                let task = conn
                    .query_row(
                        "SELECT department,task_type FROM tasks WHERE id=?",
                        [id],
                        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                    )
                    .optional()
                    .map_err(display_error)?;
                if !task.is_some_and(|(dep, kind)| scope.allows(&dep, &kind)) {
                    return Ok(false);
                }
            }
            Ok(true)
        })
        .map_err(Into::into)
    }
    pub(crate) fn mcp_history(
        &self,
        id: i64,
        full: bool,
        voided: bool,
        redact_cross_task: bool,
    ) -> Result<Vec<serde_json::Value>, McpError> {
        use serde_json::json;
        self.with_conn(|conn|{
            let mut entries=vec![];
            for (kind,sql) in [
                ("status","SELECT id,created_at,old_status,new_status,reason FROM status_history WHERE task_id=? ORDER BY id"),
                ("queue","SELECT id,enqueued_at,closed_at,close_reason,'' FROM task_queue_entries WHERE task_id=? ORDER BY id"),
                ("work","SELECT id,handled_at,result_status,COALESCE(voided_at,''),note FROM task_work_events WHERE task_id=? ORDER BY id"),
                ("log","SELECT id,created_at,log_type,'',content FROM task_logs WHERE task_id=? ORDER BY id"),
                ("urgent","SELECT id,requested_at,confirmation_status,COALESCE(cancelled_at,''),reason FROM urgent_records WHERE task_id=? ORDER BY id"),
            ]{
                let mut stmt=conn.prepare(sql).map_err(display_error)?;
                let values=stmt.query_map([id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?))).map_err(display_error)?;
                for row in values {
                    let (event_id,at,from,to,text)=row.map_err(display_error)?;
                    if kind=="work"&&!voided&&!to.is_empty(){continue;}
                    if kind=="log"&&!full{continue;}
                    let mut value=json!({"id":event_id,"taskId":id,"kind":kind,"at":at,"from":from,"to":to});
                    if kind=="queue"{value["phase"]=json!("enqueued");value["enqueuedAt"]=json!(at);value["closedAt"]=json!(from);}
                    if kind=="work"{value["voidedAt"]=if to.is_empty(){serde_json::Value::Null}else{json!(to)};}
                    if full{
                        // Legacy relationship/merge logs contain other task titles
                        // but lack stable referenced IDs. Limited clients must not
                        // receive that text even when they can read this task fully.
                        let cross_task=kind=="log"&&from.as_deref().is_some_and(|k|k.contains("relation")||k.contains("merge"));
                        if redact_cross_task&&cross_task{value["redactedFields"]=json!(["text"]);}else{value["text"]=json!(text);}
                    }
                    if full&&kind=="urgent"{
                        value["details"]=conn.query_row("SELECT requester,requested_deadline,confirmed_at,notes FROM urgent_records WHERE id=?",[event_id],|r|Ok(json!({"requester":r.get::<_,String>(0)?,"requestedDeadline":r.get::<_,Option<String>>(1)?,"confirmedAt":r.get::<_,Option<String>>(2)?,"notes":r.get::<_,String>(3)?}))).map_err(display_error)?;
                    }
                    if full&&kind=="work"{
                        value["details"]=conn.query_row("SELECT source,task_type_snapshot,created_at,updated_at FROM task_work_events WHERE id=?",[event_id],|r|Ok(json!({"source":r.get::<_,String>(0)?,"taskTypeSnapshot":r.get::<_,String>(1)?,"createdAt":r.get::<_,String>(2)?,"updatedAt":r.get::<_,String>(3)?}))).map_err(display_error)?;
                    }
                    entries.push(value);
                    if kind=="queue"{if let Some(closed)=from{let mut closure=entries.last().unwrap().clone();closure["at"]=json!(closed);closure["phase"]=json!("closed");entries.push(closure);}}
                }
            }
            let timestamp=|value:&serde_json::Value|value["at"].as_str().and_then(|s|chrono::DateTime::parse_from_rfc3339(s).ok()).map(|t|t.timestamp_millis());
            entries.sort_by(|a,b|timestamp(b).cmp(&timestamp(a)).then_with(||a["kind"].as_str().cmp(&b["kind"].as_str())).then_with(||b["id"].as_i64().cmp(&a["id"].as_i64())).then_with(||a["phase"].as_str().cmp(&b["phase"].as_str())));
            Ok(entries)
        }).map_err(Into::into)
    }
}
