//! Derived metadata repair is a separate, validated transaction. No business mutation.
use super::*;
use crate::mcp::contract::McpError;
use serde_json::{json, Value};
fn definitions() -> Vec<(String, String)> {
    let mut all = super::mcp_changes::definitions();
    for table in ["daily_sequences", "master_values", "settings"] {
        for action in ["INSERT", "UPDATE", "DELETE"] {
            let name = format!("mcp_basis_{table}_{action}");
            all.push((name.clone(),format!("CREATE TRIGGER {name} AFTER {action} ON {table} BEGIN UPDATE mcp_data_basis SET commit_sequence=commit_sequence+1 WHERE singleton=1; END;")));
        }
    }
    all
}
fn normalize(sql: &str) -> String {
    sql.trim()
        .trim_end_matches(';')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}
pub(super) fn health(c: &Connection) -> Result<Value, String> {
    let mut missing = vec![];
    for name in ["mcp_data_basis", "mcp_change_events", "mcp_change_index"] {
        let exists: bool = c
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
                [name],
                |r| r.get(0),
            )
            .map_err(display_error)?;
        if !exists {
            missing.push(name);
        }
    }
    if !missing.is_empty() {
        return Ok(
            json!({"healthy":false,"repairable":!missing.contains(&"mcp_data_basis"),"issues":["derived_tables_missing"],"missing":missing}),
        );
    }
    let mut issues = vec![];
    let index_sql: Option<String> = c
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='index' AND name='mcp_change_task_sequence'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(display_error)?;
    if index_sql.as_deref().is_none_or(|sql| {
        normalize(sql)
            != normalize(
                "CREATE INDEX mcp_change_task_sequence ON mcp_change_events(task_id,sequence)",
            )
    }) {
        issues.push("derived_lookup_index_invalid");
    }
    let mut mismatched = vec![];
    for (name, expected) in definitions() {
        let sql: Option<String> = c
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type='trigger' AND name=?",
                [&name],
                |r| r.get(0),
            )
            .optional()
            .map_err(display_error)?;
        if sql
            .as_deref()
            .is_none_or(|sql| normalize(sql) != normalize(&expected))
        {
            mismatched.push(name);
        }
    }
    let trigger_count: i64 = c
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name GLOB 'mcp_basis_*'",
            [],
            |r| r.get(0),
        )
        .map_err(display_error)?;
    if !mismatched.is_empty() || trigger_count != 30 {
        issues.push("basis_triggers_invalid");
    }
    let meta: Option<(String, i64, i64)> = c
        .query_row(
            "SELECT epoch,history_floor,total_events FROM mcp_change_index WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(display_error)?;
    let (watermark, generation): (i64, i64) = c
        .query_row(
            "SELECT commit_sequence,data_generation FROM mcp_data_basis WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(display_error)?;
    let (count,min,max,min_gen,max_gen):(i64,Option<i64>,Option<i64>,Option<i64>,Option<i64>)=c.query_row("SELECT COUNT(*),MIN(sequence),MAX(sequence),MIN(generation),MAX(generation) FROM mcp_change_events",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(display_error)?;
    if let Some((epoch, floor, total)) = &meta {
        if epoch.len() != 64
            || !epoch.bytes().all(|b| b.is_ascii_hexdigit())
            || *floor < 0
            || *floor > watermark
            || *total != count
            || min.is_some_and(|n| n <= *floor)
            || max.is_some_and(|n| n > watermark)
            || min_gen.is_some_and(|n| n < 1)
            || max_gen.is_some_and(|n| n > generation)
        {
            issues.push("derived_continuity_invalid");
        }
    } else {
        issues.push("derived_metadata_missing");
    }
    Ok(
        json!({"healthy":issues.is_empty(),"repairable":true,"issues":issues,"mismatchedTriggers":mismatched,"indexEpoch":meta.as_ref().map(|m|&m.0),"historyFloor":meta.as_ref().map(|m|m.1),"watermark":watermark,"dataGeneration":generation,"businessDataAutomaticallyRepaired":false,"knownGaps":["Events before the index history floor are unavailable; missing original business records are never reconstructed"]}),
    )
}
pub(super) fn repair(c: &mut Connection) -> Result<Value, String> {
    let initial = health(c)?;
    if initial["healthy"] == true {
        return Ok(json!({"repaired":false,"health":initial}));
    }
    if initial["repairable"] != true {
        return Err("权威数据基准缺失，未自动修复业务库".into());
    }
    let tx = c.transaction().map_err(display_error)?;
    let before:(String,i64,i64)=tx.query_row("SELECT database_uuid,data_generation,commit_sequence FROM mcp_data_basis WHERE singleton=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(display_error)?;
    let triggers = {
        let mut s = tx
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='trigger' AND name GLOB 'mcp_basis_*'",
            )
            .map_err(display_error)?;
        let result = s
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(display_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(display_error)?;
        result
    };
    for name in triggers {
        tx.execute_batch(&format!("DROP TRIGGER \"{}\";", name.replace('"', "\"\"")))
            .map_err(display_error)?;
    }
    tx.execute_batch("DROP TABLE IF EXISTS mcp_change_events;DROP TABLE IF EXISTS mcp_change_index;
        CREATE TABLE mcp_change_index(singleton INTEGER PRIMARY KEY CHECK(singleton=1),epoch TEXT NOT NULL,history_floor INTEGER NOT NULL,total_events INTEGER NOT NULL DEFAULT 0);
        INSERT INTO mcp_change_index SELECT 1,lower(hex(randomblob(32))),commit_sequence,0 FROM mcp_data_basis WHERE singleton=1;
        CREATE TABLE mcp_change_events(sequence INTEGER PRIMARY KEY,generation INTEGER NOT NULL,task_id INTEGER NOT NULL,source TEXT NOT NULL,operation TEXT NOT NULL,changed_at TEXT NOT NULL);
        CREATE INDEX mcp_change_task_sequence ON mcp_change_events(task_id,sequence);") .map_err(display_error)?;
    for (_, definition) in definitions() {
        tx.execute_batch(&definition).map_err(display_error)?;
    }
    let after:(String,i64,i64)=tx.query_row("SELECT database_uuid,data_generation,commit_sequence FROM mcp_data_basis WHERE singleton=1",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(display_error)?;
    let checked = health(&tx)?;
    if before != after || checked["healthy"] != true {
        return Err("派生索引校验失败，事务已回滚".into());
    }
    tx.commit().map_err(display_error)?;
    Ok(
        json!({"repaired":true,"oldIndexEpoch":initial["indexEpoch"],"health":checked,"historyReconstructed":false,"businessRecordsChanged":false,"oldCursorsRequireFullSync":true}),
    )
}
impl Database {
    pub(crate) fn mcp_quality_health(&self) -> Result<Value, McpError> {
        self.with_conn(health).map_err(Into::into)
    }
    pub fn mcp_repair_derived(&self) -> Result<Value, McpError> {
        let mut guard = self
            .connection
            .lock()
            .map_err(|_| McpError::new("internal_error"))?;
        let c = guard
            .as_mut()
            .ok_or_else(|| McpError::new("host_unavailable"))?;
        repair(c).map_err(Into::into)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Database {
        let root = std::env::temp_dir().join(format!(
            "inline-p6-quality-{}",
            crate::mcp::platform::random_secret().unwrap()
        ));
        Database::open_at(root.join("inline.db")).unwrap()
    }
    fn seed(db: &Database) {
        let input=serde_json::from_value(json!({"title":"synthetic","department":"A","departments":["A"],"contact":"fixture","contacts":["fixture"],"taskType":"T","status":"pending","priority":"normal","workload":"standard","isUrgent":false,"details":"protected","urgentRequester":"","urgentReason":"","internalNotes":""})).unwrap();
        db.save_task(input).unwrap();
    }
    fn protected(db: &Database) -> Vec<(String, Vec<Vec<String>>)> {
        db.with_conn(|c|{
            let mut names=c.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'mcp_change_%' ORDER BY name").map_err(display_error)?;
            let names=names.query_map([],|r|r.get::<_,String>(0)).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;
            let mut all=vec![];
            for name in names{let mut s=c.prepare(&format!("SELECT * FROM \"{}\" ORDER BY rowid",name.replace('"',"\"\""))).map_err(display_error)?;let columns=s.column_count();let rows=s.query_map([],|r|{(0..columns).map(|i|r.get::<_,rusqlite::types::Value>(i).map(|v|format!("{v:?}"))).collect::<Result<Vec<_>,_>>()}).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;all.push((name,rows));}Ok(all)
        }).unwrap()
    }
    #[test]
    fn derived_loss_detection_repair_validates_epoch_and_preserves_all_sources() {
        let db = fixture();
        seed(&db);
        assert_eq!(db.mcp_quality_health().unwrap()["healthy"], true);
        let basis = db.mcp_basis().unwrap();
        let before = protected(&db);
        db.with_conn(|c|c.execute_batch("DELETE FROM mcp_change_events WHERE sequence=(SELECT MIN(sequence) FROM mcp_change_events);DROP TRIGGER mcp_basis_tasks_UPDATE;").map_err(display_error)).unwrap();
        let unhealthy = db.mcp_quality_health().unwrap();
        assert_eq!(unhealthy["healthy"], false);
        assert!(unhealthy["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "basis_triggers_invalid"));
        let result = db.mcp_repair_derived().unwrap();
        assert_eq!(result["repaired"], true);
        assert_eq!(result["historyReconstructed"], false);
        let after = db.mcp_basis().unwrap();
        assert_ne!(basis.index_epoch, after.index_epoch);
        assert_eq!(basis.commit_sequence, after.commit_sequence);
        assert_eq!(basis.database_uuid, after.database_uuid);
        assert_eq!(basis.data_generation, after.data_generation);
        assert_eq!(before, protected(&db));
        let index = db.mcp_change_index().unwrap();
        assert_eq!(index["eventCount"], 0);
        assert_eq!(index["historyFloor"], basis.commit_sequence);
        assert_eq!(db.mcp_repair_derived().unwrap()["repaired"], false);
        assert_eq!(after, db.mcp_basis().unwrap());
        seed(&db);
        assert_eq!(db.mcp_quality_health().unwrap()["healthy"], true);
        assert!(
            db.mcp_change_index().unwrap()["eventCount"]
                .as_i64()
                .unwrap()
                > 0
        );
    }
    #[test]
    fn failed_rebuild_rolls_back_partial_ddl_and_never_overwrites_business_index() {
        let db = fixture();
        seed(&db);
        db.with_conn(|c|c.execute_batch("DROP INDEX mcp_change_task_sequence;CREATE INDEX mcp_change_task_sequence ON tasks(title);").map_err(display_error)).unwrap();
        assert_eq!(db.mcp_quality_health().unwrap()["healthy"], false);
        let before = protected(&db);
        let basis = db.mcp_basis().unwrap();
        let index = db.mcp_change_index().unwrap();
        assert!(db.mcp_repair_derived().is_err());
        assert_eq!(before, protected(&db));
        assert_eq!(basis, db.mcp_basis().unwrap());
        assert_eq!(index, db.mcp_change_index().unwrap());
        db.with_conn(|c| {
            assert_eq!(
                c.query_row(
                    "SELECT tbl_name FROM sqlite_master WHERE name='mcp_change_task_sequence'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
                "tasks"
            );
            Ok(())
        })
        .unwrap();
    }
    #[test]
    fn startup_repairs_only_derived_tables_and_never_fabricates_history() {
        let db = fixture();
        seed(&db);
        let root = db.backup_dir.parent().unwrap().to_owned();
        let before = protected(&db);
        db.with_conn(|c| {
            c.execute_batch("DROP TRIGGER mcp_basis_settings_UPDATE;")
                .map_err(display_error)
        })
        .unwrap();
        drop(db);
        let reopened = Database::open_root(root).unwrap();
        assert_eq!(reopened.mcp_quality_health().unwrap()["healthy"], true);
        assert_eq!(before, protected(&reopened));
        assert_eq!(reopened.mcp_change_index().unwrap()["eventCount"], 0);
    }
    #[test]
    fn master_noop_is_stable_and_inactive_values_still_follow_existing_reactivation_rule() {
        let db = fixture();
        seed(&db);
        let before = protected(&db);
        let basis = db.mcp_basis().unwrap();
        db.with_conn(|c| ensure_master(c, "contact", "fixture"))
            .unwrap();
        assert_eq!(before, protected(&db));
        assert_eq!(basis, db.mcp_basis().unwrap());
        db.with_conn(|c| {
            c.execute(
                "UPDATE master_values SET is_active=0 WHERE kind='contact' AND name='fixture'",
                [],
            )
            .map_err(display_error)?;
            ensure_master(c, "contact", "fixture")?;
            assert_eq!(
                c.query_row(
                    "SELECT is_active FROM master_values WHERE kind='contact' AND name='fixture'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                1
            );
            Ok(())
        })
        .unwrap();
    }
}
