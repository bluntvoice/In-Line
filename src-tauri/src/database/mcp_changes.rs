//! P5 derived events share the authoritative GUI/MCP transaction's sequence.
//! Rows contain no task bodies, contact names, notes or credentials.
use super::*;
use crate::mcp::contract::McpError;
use serde_json::{json, Value as Json};

const TABLES: &[(&str, &str)] = &[
    ("tasks", "id"),
    ("task_logs", "task_id"),
    ("status_history", "task_id"),
    ("urgent_records", "task_id"),
    ("task_queue_entries", "task_id"),
    ("task_work_events", "task_id"),
    ("queue_number_allocations", "task_id"),
];
pub(super) fn install(tx: &Connection) -> Result<(), String> {
    for (table, id) in TABLES {
        for action in ["INSERT", "UPDATE", "DELETE"] {
            let row = if action == "DELETE" { "OLD" } else { "NEW" };
            let task = if *table == "queue_number_allocations" && action == "UPDATE" {
                "COALESCE(NEW.task_id,OLD.task_id)".to_owned()
            } else {
                format!("{row}.{id}")
            };
            tx.execute_batch(&format!("DROP TRIGGER IF EXISTS mcp_basis_{table}_{action}; CREATE TRIGGER mcp_basis_{table}_{action} AFTER {action} ON {table} BEGIN
          UPDATE mcp_data_basis SET commit_sequence=commit_sequence+1 WHERE singleton=1;
          UPDATE mcp_change_index SET total_events=total_events+1 WHERE singleton=1 AND {task} IS NOT NULL;
          INSERT INTO mcp_change_events(sequence,generation,task_id,source,operation,changed_at) SELECT commit_sequence,data_generation,{task},'{table}','{action}',strftime('%Y-%m-%dT%H:%M:%fZ','now') FROM mcp_data_basis WHERE singleton=1 AND {task} IS NOT NULL;
          END;" )).map_err(display_error)?;
        }
    }
    Ok(())
}
pub(super) fn migrate(tx: &Transaction<'_>, version: i64) -> Result<(), String> {
    if version >= 14 {
        return Ok(());
    }
    tx.execute_batch("CREATE TABLE IF NOT EXISTS mcp_change_index(singleton INTEGER PRIMARY KEY CHECK(singleton=1),epoch TEXT NOT NULL,history_floor INTEGER NOT NULL,total_events INTEGER NOT NULL DEFAULT 0);
      INSERT OR IGNORE INTO mcp_change_index SELECT 1,lower(hex(randomblob(32))),commit_sequence,0 FROM mcp_data_basis WHERE singleton=1;
      CREATE TABLE IF NOT EXISTS mcp_change_events(sequence INTEGER PRIMARY KEY,generation INTEGER NOT NULL,task_id INTEGER NOT NULL,source TEXT NOT NULL,operation TEXT NOT NULL,changed_at TEXT NOT NULL);
      CREATE INDEX IF NOT EXISTS mcp_change_task_sequence ON mcp_change_events(task_id,sequence);") .map_err(display_error)?;
    install(tx)?;
    tx.execute_batch("DELETE FROM schema_meta; INSERT INTO schema_meta VALUES(14);")
        .map_err(display_error)
}
impl Database {
    pub(crate) fn mcp_change_index(&self) -> Result<Json, McpError> {
        let index=self.with_conn(|c|c.query_row("SELECT epoch,history_floor,(SELECT COUNT(*) FROM mcp_change_events),(SELECT commit_sequence FROM mcp_data_basis WHERE singleton=1),total_events FROM mcp_change_index WHERE singleton=1",[],|r|Ok(json!({"epoch":r.get::<_,String>(0)?,"historyFloor":r.get::<_,i64>(1)?,"eventCount":r.get::<_,i64>(2)?,"watermark":r.get::<_,i64>(3)?,"expectedEvents":r.get::<_,i64>(4)?}))).map_err(display_error)).map_err(McpError::from)?;
        if index["eventCount"] != index["expectedEvents"] {
            return Err(McpError::new("resync_required"));
        }
        Ok(index)
    }
    pub(crate) fn mcp_changes_after(
        &self,
        after: i64,
        watermark: i64,
        count: i64,
    ) -> Result<Vec<Json>, McpError> {
        self.with_conn(|c|{let mut s=c.prepare("SELECT sequence,generation,task_id,source,operation,changed_at FROM mcp_change_events WHERE sequence>? AND sequence<=? ORDER BY sequence LIMIT ?").map_err(display_error)?;
            let rows=s.query_map(params![after,watermark,count],|r|Ok(json!({"sequence":r.get::<_,i64>(0)?,"generation":r.get::<_,i64>(1)?,"taskId":r.get::<_,i64>(2)?,"source":r.get::<_,String>(3)?,"operation":r.get::<_,String>(4)?,"at":r.get::<_,String>(5)?}))).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;Ok(rows)}).map_err(Into::into)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gui_transactions_and_failed_batches_do_not_leave_derived_events() {
        let path = std::env::temp_dir()
            .join(format!(
                "inline-p5-events-{}",
                crate::mcp::platform::random_secret().unwrap()
            ))
            .join("inline.db");
        let db = Database::open_at(path).unwrap();
        let task:TaskInput=serde_json::from_value(json!({"title":"synthetic","department":"A","departments":["A"],"contact":"C","contacts":["C"],"taskType":"T","status":"pending","priority":"normal","workload":"standard","isUrgent":false,"details":"","internalNotes":"","urgentRequester":"","urgentReason":""})).unwrap();
        let task = db.save_task(task).unwrap();
        let index = db.mcp_change_index().unwrap();
        let basis = db.mcp_basis().unwrap();
        db.with_conn(|c| {
            c.execute_batch(&format!(
                "BEGIN IMMEDIATE;UPDATE tasks SET title='rolled back' WHERE id={};ROLLBACK;",
                task.id
            ))
            .map_err(display_error)
        })
        .unwrap();
        assert_eq!(db.mcp_basis().unwrap(), basis);
        assert_eq!(db.mcp_change_index().unwrap(), index);
        let rows = db
            .mcp_changes_after(0, basis.commit_sequence as i64, 1000)
            .unwrap();
        assert!(!rows.is_empty());
        assert!(rows
            .windows(2)
            .all(|w| w[0]["sequence"].as_i64() < w[1]["sequence"].as_i64()));
        assert!(!serde_json::to_string(&rows).unwrap().contains("synthetic"));
        db.soft_delete(task.id).unwrap();
        assert_eq!(db.permanently_delete_tasks(vec![task.id]).unwrap(), 1);
        let index = db.mcp_change_index().unwrap();
        assert_eq!(index["eventCount"], index["expectedEvents"]);
    }
}
