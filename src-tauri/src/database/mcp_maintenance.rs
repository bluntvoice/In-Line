//! Local operator maintenance; never callable as an MCP mutation.
use super::*;
use crate::mcp::contract::McpError;
use serde_json::{json, Value as Json};
fn inventory(conn: &Connection) -> Result<Json, String> {
    let count: i64 = conn
        .query_row("SELECT count(*) FROM mcp_ai_audit", [], |r| r.get(0))
        .map_err(display_error)?;
    if count > 10000 {
        return Err("审计超过10000条，请先缩小维护范围；未清理记录".into());
    }
    let mut q=conn.prepare("WITH RECURSIVE protected(id) AS (SELECT audit_id FROM mcp_undo_requests WHERE status='pending' UNION SELECT a.id FROM mcp_ai_audit a JOIN protected p ON a.undo_of=p.id UNION SELECT a.undo_of FROM mcp_ai_audit a JOIN protected p ON a.id=p.id WHERE a.undo_of IS NOT NULL) SELECT a.id,a.client_id,a.action,a.task_id,a.reason,a.intent,a.created_at,a.before_json,a.after_json,a.undo_of,EXISTS(SELECT 1 FROM protected p WHERE p.id=a.id) FROM mcp_ai_audit a ORDER BY a.id").map_err(display_error)?;
    let rows=q.query_map([],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"clientId":r.get::<_,String>(1)?,"action":r.get::<_,String>(2)?,"taskId":r.get::<_,Option<i64>>(3)?,"reason":r.get::<_,String>(4)?,"intent":r.get::<_,String>(5)?,"createdAt":r.get::<_,String>(6)?,"before":serde_json::from_str::<Json>(&r.get::<_,String>(7)?).unwrap_or(Json::Null),"after":serde_json::from_str::<Json>(&r.get::<_,String>(8)?).unwrap_or(Json::Null),"undoOf":r.get::<_,Option<i64>>(9)?,"protected":r.get::<_,bool>(10)?}))).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;
    let result = json!({"items":rows,"count":count,"protected":rows.iter().filter(|r|r["protected"]==true).count()});
    if serde_json::to_vec(&result).map_err(display_error)?.len() > 16 * 1024 * 1024 {
        return Err("审计内容超过16MiB，未生成不完整预览或清理记录".into());
    }
    Ok(result)
}
impl Database {
    pub fn mcp_audit_inventory(&self) -> Result<Json, String> {
        self.with_conn(inventory)
    }
    pub fn mcp_clear_audit_confirmed(&self, expected: &Json) -> Result<Json, McpError> {
        self.mcp_transaction(|conn|{
            let current=inventory(conn).map_err(McpError::from)?;
            if &current!=expected{return Err(McpError::new("conflict"));}
            let ids=current["items"].as_array().ok_or_else(||McpError::new("internal_error"))?.iter().filter(|r|r["protected"]!=true).filter_map(|r|r["id"].as_i64()).collect::<Vec<_>>();
            for id in &ids{conn.execute("DELETE FROM mcp_undo_requests WHERE audit_id=? AND status!='pending'",[id]).map_err(|_|McpError::new("internal_error"))?;conn.execute("DELETE FROM mcp_ai_audit WHERE id=?",[id]).map_err(|_|McpError::new("internal_error"))?;}
            if !ids.is_empty(){conn.execute("UPDATE mcp_data_basis SET data_generation=data_generation+1,commit_sequence=commit_sequence+1 WHERE singleton=1",[]).map_err(|_|McpError::new("internal_error"))?;}
            if !ids.is_empty(){conn.execute_batch("DELETE FROM mcp_change_events; UPDATE mcp_change_index SET epoch=lower(hex(randomblob(32))),history_floor=(SELECT commit_sequence FROM mcp_data_basis WHERE singleton=1),total_events=0 WHERE singleton=1;").map_err(|_|McpError::new("internal_error"))?;}
            Ok(json!({"removed":ids.len(),"protected":current["protected"],"businessRowsRemoved":0,"receiptsRemoved":0,"formalExportsRemoved":0,"oldQueriesRequireRefresh":!ids.is_empty()}))
        })
    }
    pub(crate) fn mcp_maintenance_root(&self) -> &Path {
        self.backup_dir.parent().unwrap_or(&self.backup_dir)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_component_is_protected_and_changed_preview_cannot_clear_receipts() {
        let root = std::env::temp_dir().join(format!(
            "inline-p7-audit-{}",
            crate::mcp::platform::random_secret().unwrap()
        ));
        let db = Database::open_root(root).unwrap();
        let security = crate::mcp::security::Security::open_at(std::env::temp_dir().join(format!(
            "inline-p7-purged-{}",
            crate::mcp::platform::random_secret().unwrap()
        )))
        .unwrap();
        let issued = security
            .grant(crate::mcp::security::Grant {
                name: "synthetic".into(),
                permissions: crate::mcp::security::Permissions::default(),
                scope: crate::mcp::scope::Scope::default(),
            })
            .unwrap();
        let credentials = crate::mcp::contract::Credentials {
            client_id: issued.client_id,
            token: issued.token,
        };
        let old =
            crate::mcp::service::execute(&security, &db, &credentials, "query_tasks", json!({}))
                .unwrap();
        db.with_conn(|c|c.execute_batch("INSERT INTO mcp_receipts VALUES('a','original-key','hash',NULL,3,'{\"commitStatus\":\"committed\"}'); INSERT INTO mcp_ai_audit VALUES(1,'a','patch',NULL,'reason','intent','{}','{}','{}','2026-10-12',NULL),(2,'a','undo',NULL,'reason','intent','{}','{}','{}','2026-10-12',1),(3,'a','patch',NULL,'reason','intent','{}','{}','{}','2026-10-12',NULL); INSERT INTO mcp_undo_requests(audit_id,client_id,reason,created_at) VALUES(2,'a','undo','2026-10-12');").map_err(display_error)).unwrap();
        let p = db.mcp_audit_inventory().unwrap();
        assert_eq!(p["protected"], 2);
        let basis = db.mcp_basis().unwrap();
        let receipt = db
            .with_conn(|c| {
                c.query_row(
                    "SELECT result_json FROM mcp_receipts WHERE request_key='original-key'",
                    [],
                    |r| r.get::<_, String>(0),
                )
                .map_err(display_error)
            })
            .unwrap();
        let result = db.mcp_clear_audit_confirmed(&p).unwrap();
        assert_eq!(result["removed"], 1);
        assert_eq!(db.mcp_audit_inventory().unwrap()["count"], 2);
        let next = db.mcp_basis().unwrap();
        assert_eq!(
            receipt,
            db.with_conn(|c| c
                .query_row(
                    "SELECT result_json FROM mcp_receipts WHERE request_key='original-key'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .map_err(display_error))
                .unwrap()
        );
        assert_eq!(db.mcp_quality_health().unwrap()["healthy"], true);
        assert_eq!(next.database_uuid, basis.database_uuid);
        assert_eq!(next.data_generation, basis.data_generation + 1);
        let error = crate::mcp::service::execute(
            &security,
            &db,
            &credentials,
            "query_tasks",
            json!({"snapshot":old["meta"]["snapshotId"]}),
        )
        .unwrap_err();
        assert_eq!(error.code, "snapshot_invalid");
        assert_eq!(
            db.mcp_clear_audit_confirmed(&p).unwrap_err().code,
            "conflict"
        );
        assert_eq!(
            db.mcp_clear_audit_confirmed(&db.mcp_audit_inventory().unwrap())
                .unwrap()["removed"],
            0
        );
    }
}
