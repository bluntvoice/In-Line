//! Schema v12, shared across supported compilation targets.
use super::*;
pub(super) const FIELDS: &[(&str, &str)] = &[
    ("title", "title"),
    ("departments", "department"),
    ("contacts", "contact"),
    ("taskType", "task_type"),
    ("details", "details"),
    ("internalNotes", "internal_notes"),
    ("priority", "priority"),
    ("workload", "workload"),
    ("requestedDeadline", "requested_deadline"),
    ("requestedDeadlineLabel", "requested_deadline_label"),
];
pub(super) fn migrate(tx: &Transaction<'_>, version: i64) -> Result<(), String> {
    if version >= 12 {
        return Ok(());
    }
    tx.execute_batch("CREATE TABLE IF NOT EXISTS mcp_task_versions(task_id INTEGER PRIMARY KEY,version INTEGER NOT NULL DEFAULT 0);
        CREATE TABLE IF NOT EXISTS mcp_field_versions(task_id INTEGER NOT NULL,field TEXT NOT NULL,version INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(task_id,field));
        CREATE TABLE IF NOT EXISTS mcp_preference_versions(key TEXT PRIMARY KEY,version INTEGER NOT NULL DEFAULT 0);
        INSERT OR IGNORE INTO mcp_preference_versions SELECT key,0 FROM settings;
        CREATE TRIGGER IF NOT EXISTS mcp_preference_insert AFTER INSERT ON settings BEGIN INSERT INTO mcp_preference_versions VALUES(NEW.key,1) ON CONFLICT(key) DO UPDATE SET version=version+1; END;
        CREATE TRIGGER IF NOT EXISTS mcp_preference_update AFTER UPDATE ON settings WHEN OLD.value IS NOT NEW.value BEGIN INSERT INTO mcp_preference_versions VALUES(NEW.key,1) ON CONFLICT(key) DO UPDATE SET version=version+1; END;
        CREATE TRIGGER IF NOT EXISTS mcp_preference_delete AFTER DELETE ON settings BEGIN INSERT INTO mcp_preference_versions VALUES(OLD.key,1) ON CONFLICT(key) DO UPDATE SET version=version+1; END;
        CREATE TABLE IF NOT EXISTS mcp_ai_audit(id INTEGER PRIMARY KEY AUTOINCREMENT,client_id TEXT NOT NULL,action TEXT NOT NULL,task_id INTEGER,reason TEXT NOT NULL,intent TEXT NOT NULL,before_json TEXT NOT NULL,after_json TEXT NOT NULL,request_json TEXT NOT NULL,created_at TEXT NOT NULL,undo_of INTEGER);
        CREATE TABLE IF NOT EXISTS mcp_receipts(client_id TEXT NOT NULL,request_key TEXT NOT NULL,request_hash TEXT NOT NULL,task_id INTEGER,audit_id INTEGER,result_json TEXT NOT NULL,PRIMARY KEY(client_id,request_key));
        CREATE TABLE IF NOT EXISTS mcp_undo_requests(id INTEGER PRIMARY KEY AUTOINCREMENT,audit_id INTEGER NOT NULL UNIQUE,client_id TEXT NOT NULL,reason TEXT NOT NULL,status TEXT NOT NULL DEFAULT 'pending',created_at TEXT NOT NULL,resolved_at TEXT);
        INSERT OR IGNORE INTO mcp_task_versions SELECT id,0 FROM tasks;
        CREATE TRIGGER IF NOT EXISTS mcp_task_version_insert AFTER INSERT ON tasks BEGIN INSERT OR REPLACE INTO mcp_task_versions VALUES(NEW.id,0); END;
        CREATE TRIGGER IF NOT EXISTS mcp_task_version_update AFTER UPDATE ON tasks BEGIN INSERT INTO mcp_task_versions VALUES(NEW.id,1) ON CONFLICT(task_id) DO UPDATE SET version=version+1; END;
        ") .map_err(display_error)?;
    for (field, column) in FIELDS {
        tx.execute_batch(&format!("INSERT OR IGNORE INTO mcp_field_versions SELECT id,'{field}',0 FROM tasks;
            CREATE TRIGGER IF NOT EXISTS mcp_field_{column}_insert AFTER INSERT ON tasks BEGIN INSERT OR REPLACE INTO mcp_field_versions VALUES(NEW.id,'{field}',0); END;
            CREATE TRIGGER IF NOT EXISTS mcp_field_{column}_update AFTER UPDATE OF {column} ON tasks WHEN OLD.{column} IS NOT NEW.{column} BEGIN
            INSERT INTO mcp_field_versions VALUES(NEW.id,'{field}',1) ON CONFLICT(task_id,field) DO UPDATE SET version=version+1; END;")) .map_err(display_error)?;
    }
    for table in [
        "task_work_events",
        "task_queue_entries",
        "status_history",
        "urgent_records",
    ] {
        for action in ["INSERT", "UPDATE", "DELETE"] {
            let row = if action == "DELETE" { "OLD" } else { "NEW" };
            tx.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS mcp_version_{table}_{action} AFTER {action} ON {table} BEGIN UPDATE mcp_task_versions SET version=version+1 WHERE task_id={row}.task_id; END;")) .map_err(display_error)?;
        }
    }
    tx.execute_batch("DELETE FROM schema_meta; INSERT INTO schema_meta VALUES(12);")
        .map_err(display_error)
}
