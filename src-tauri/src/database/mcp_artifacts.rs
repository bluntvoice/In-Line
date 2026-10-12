//! Explicit, local-only artifact creation. No caller-supplied paths or backup read APIs.
use super::mcp_write::*;
use super::*;
use crate::mcp::{
    contract::McpError,
    platform::{check_path, Descriptor},
    security::Authorization,
    write_types::*,
};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::io::Write;

fn job_request(args: &BusinessDataArgs, auth: &Authorization) -> Json {
    json!({"tool":"manage_business_data","args":args,"permissions":auth.permissions,"scope":auth.scope})
}
impl Database {
    pub(crate) fn mcp_validate_artifact_intent(
        auth: &Authorization,
        args: &BusinessDataArgs,
    ) -> Result<(), McpError> {
        if !auth.permissions.write || !auth.permissions.regular_read {
            return Err(err("forbidden"));
        }
        validate_intent(&args.idempotency_key, &args.intent, &args.reason)
    }
    pub fn mcp_artifact(
        &self,
        client: &str,
        auth: &Authorization,
        args: BusinessDataArgs,
        export: impl FnOnce() -> Result<Vec<u8>, McpError>,
    ) -> Result<Json, McpError> {
        self.mcp_artifact_impl(client, auth, args, true, export)
    }
    pub(crate) fn mcp_artifact_receipt(
        &self,
        client: &str,
        auth: &Authorization,
        args: BusinessDataArgs,
    ) -> Result<Json, McpError> {
        self.mcp_artifact_impl(client, auth, args, false, || Err(err("result_unknown")))
    }
    fn mcp_artifact_impl(
        &self,
        client: &str,
        auth: &Authorization,
        args: BusinessDataArgs,
        allow_fresh: bool,
        export: impl FnOnce() -> Result<Vec<u8>, McpError>,
    ) -> Result<Json, McpError> {
        Self::mcp_validate_artifact_intent(auth, &args)?;
        if !matches!(args.action.as_str(), "backup" | "export") {
            return Err(err("invalid_arguments"));
        }
        if args.action == "backup"
            && (args.format.is_some() || args.filters.is_some() || args.projection.is_some())
        {
            return Err(err("invalid_arguments"));
        }
        let format = if args.action == "backup" {
            "db"
        } else {
            args.format
                .as_deref()
                .ok_or_else(|| err("invalid_arguments"))?
        };
        if (args.action == "backup" && format != "db")
            || (args.action == "export" && !matches!(format, "json" | "csv"))
        {
            return Err(err("invalid_arguments"));
        }
        let request = job_request(&args, auth);
        let hash = fingerprint(&request);
        let directory = self.backup_dir.join("mcp-created");
        check_path(&self.backup_dir)?;
        fs::create_dir_all(&directory).map_err(|_| err("internal_error"))?;
        check_path(&directory)?;
        let acl = Descriptor::new(true)?;
        acl.apply(&directory)?;
        acl.verify(&directory)?;
        let name = format!(
            "{}-mcp-{}.{}",
            if args.action == "backup" {
                "manual"
            } else {
                "export"
            },
            fingerprint(&json!([client, args.idempotency_key])),
            format
        );
        let path = directory.join(&name);
        let fresh = {
            let mut guard = self.connection.lock().map_err(|_| err("internal_error"))?;
            let conn = guard.as_mut().ok_or_else(|| err("host_unavailable"))?;
            let tx = conn.transaction().map_err(sql)?;
            let existing:Option<(String,String,Option<String>)>=tx.query_row("SELECT request_hash,status,result_json FROM mcp_artifact_jobs WHERE client_id=? AND request_key=?",params![client,args.idempotency_key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(sql)?;
            if let Some((h, status, result)) = existing {
                if h != hash {
                    return Err(err("idempotency_conflict"));
                }
                if status == "complete" {
                    let mut r: Json = serde_json::from_str(
                        result.as_deref().ok_or_else(|| err("internal_error"))?,
                    )
                    .map_err(|_| err("internal_error"))?;
                    r["replayed"] = json!(true);
                    return Ok(r);
                }
                if !path.exists() {
                    return Err(err("result_unknown"));
                }
                false
            } else {
                if !allow_fresh {
                    return Err(err("result_unknown"));
                }
                if path.exists() {
                    return Err(err("result_unknown"));
                }
                tx.execute("INSERT INTO mcp_artifact_jobs(client_id,request_key,request_hash,kind,path,created_at) VALUES(?,?,?,?,?,?)",params![client,args.idempotency_key,hash,args.action,path.to_string_lossy(),now()]).map_err(sql)?;
                tx.commit().map_err(|_| err("result_unknown"))?;
                true
            }
        };
        if fresh {
            if args.action == "backup" {
                // Refuse existing destinations; the GUI replacement helper removes them.
                self.with_conn(|conn| {
                    if path.exists() {
                        return Err("备份目标已存在，未覆盖".into());
                    }
                    conn.execute_batch(&format!(
                        "VACUUM INTO '{}'",
                        path.to_string_lossy().replace('\'', "''")
                    ))
                    .map_err(display_error)
                })
                .map_err(|_| err("result_unknown"))?;
            } else {
                let bytes = export()?;
                if bytes.len() > 32 * 1024 * 1024 {
                    return Err(err("resource_limit"));
                }
                // Durable metadata precedes publication; a complete target only appears by rename.
                let expected = json!({"bytes":bytes.len(),"contentHash":format!("{:x}",Sha256::digest(&bytes))});
                self.with_conn(|conn|{conn.execute("UPDATE mcp_artifact_jobs SET result_json=? WHERE client_id=? AND request_key=?",params![expected.to_string(),client,args.idempotency_key]).map_err(display_error)?;Ok(())}).map_err(|_|err("internal_error"))?;
                let temporary = directory.join(format!("{name}.partial"));
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temporary)
                    .map_err(|_| err("result_unknown"))?;
                file.write_all(&bytes)
                    .and_then(|_| file.sync_all())
                    .map_err(|_| err("result_unknown"))?;
                drop(file);
                acl.apply(&temporary)?;
                fs::rename(&temporary, &path).map_err(|_| err("result_unknown"))?;
            }
        }
        check_path(&path)?;
        acl.apply(&path)?;
        acl.verify(&path)?;
        if args.action == "backup" {
            Self::validate_backup_file(&path).map_err(|_| err("result_unknown"))?;
            let backup = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|_| err("result_unknown"))?;
            let bound:bool=backup.query_row("SELECT EXISTS(SELECT 1 FROM mcp_artifact_jobs WHERE client_id=? AND request_key=? AND request_hash=?)",params![client,args.idempotency_key,hash],|r|r.get(0)).map_err(|_|err("result_unknown"))?;
            if !bound {
                return Err(err("result_unknown"));
            }
        } else {
            let expected=self.with_conn(|conn|conn.query_row("SELECT result_json FROM mcp_artifact_jobs WHERE client_id=? AND request_key=?",params![client,args.idempotency_key],|r|r.get::<_,String>(0)).map_err(display_error)).map_err(|_|err("result_unknown"))?;
            let expected: Json =
                serde_json::from_str(&expected).map_err(|_| err("result_unknown"))?;
            let bytes = fs::read(&path).map_err(|_| err("result_unknown"))?;
            if expected["bytes"].as_u64() != Some(bytes.len() as u64)
                || expected["contentHash"] != json!(format!("{:x}", Sha256::digest(&bytes)))
            {
                return Err(err("result_unknown"));
            }
        }
        self.mcp_transaction(|conn|{
            let saved:Option<String>=conn.query_row("SELECT result_json FROM mcp_artifact_jobs WHERE client_id=? AND request_key=? AND status='complete'",params![client,args.idempotency_key],|r|r.get(0)).optional().map_err(sql)?;
            if let Some(saved)=saved{let mut result:Json=serde_json::from_str(&saved).map_err(|_|err("internal_error"))?;result["replayed"]=json!(true);return Ok(result);}
            let result=json!({"commitStatus":"artifact_created","artifactName":name,"location":"软件备份目录/mcp-created","kind":args.action,"format":format,"bytes":fs::metadata(&path).map_err(|_|err("internal_error"))?.len(),"replayed":!fresh});
            let aid=audit(conn,client,&args.action,None,&args.intent,&args.reason,&Json::Null,&result,&request,None)?;
            let mut result=result;result["auditId"]=json!(aid);
            conn.execute("UPDATE mcp_artifact_jobs SET status='complete',result_json=? WHERE client_id=? AND request_key=?",params![result.to_string(),client,args.idempotency_key]).map_err(sql)?;Ok(result)
        })
    }
}
