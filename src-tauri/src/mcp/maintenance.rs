//! Exact preview tickets held in RAM. A caller supplies a nonce and destination, never file content.
use super::{contract::McpError, platform, security::Security};
use crate::database::Database;
use serde_json::{json, Value};
use std::{collections::HashMap, io::Write, path::Path, sync::Mutex};
struct Ticket {
    at: i64,
    source: String,
    action: String,
    format: String,
    bytes: Vec<u8>,
    inventory: Value,
}
#[derive(Default)]
pub struct Previews {
    tickets: Mutex<HashMap<String, Ticket>>,
}
impl Previews {
    pub fn preview(
        &self,
        security: &Security,
        db: &Database,
        source: &str,
        action: &str,
        format: &str,
        filter: &str,
        now: i64,
    ) -> Result<Value, McpError> {
        if !["faults", "audit"].contains(&source)
            || !["export", "clear"].contains(&action)
            || !["json", "csv"].contains(&format)
            || source == "faults" && action == "clear"
        {
            return Err(McpError::new("invalid_arguments"));
        }
        let data = if source == "faults" {
            security.diagnostics.local(filter, now)?
        } else {
            db.mcp_audit_inventory().map_err(McpError::from)?
        };
        let inventory = if source == "audit" {
            data.clone()
        } else {
            Value::Null
        };
        let build = super::diagnostics::safe_build(&super::build_identity::current());
        let content = if action == "clear" {
            json!({"action":"clearAudit","count":data["count"].as_u64().unwrap_or(0).saturating_sub(data["protected"].as_u64().unwrap_or(0)),"protected":data["protected"],"pendingUndoAndEvidencePreserved":true,"receiptsAndBusinessRowsPreserved":true,"oldQueriesRequireRefresh":true})
        } else if source == "faults" {
            json!({"source":"technical_faults","exportedAt":now,"exportBuild":build,"items":data["items"],"facts":"记录的技术结果与独立轮次；未证明根因。","hypotheses":"待在隔离数据中验证；不自动修复业务或修改权限。","storageComplete":data["storageError"]!=true,"businessBodiesIncluded":false,"rawLogsIncluded":false})
        } else {
            json!({"source":"local_business_audit","exportedAt":now,"exportBuild":build,"items":data["items"],"containsBusinessData":true})
        };
        let text = if format == "json" || action == "clear" {
            serde_json::to_string_pretty(&content).map_err(|_| McpError::new("internal_error"))?
        } else {
            csv_export(&content, source)?
        };
        if text.len() > 16 * 1024 * 1024 {
            return Err(McpError::new("resource_limit"));
        }
        let nonce = platform::random_secret()?;
        let mut tickets = self
            .tickets
            .lock()
            .map_err(|_| McpError::new("internal_error"))?;
        tickets.retain(|_, t| now >= t.at && now - t.at < 600);
        if tickets.len() >= 2 {
            return Err(McpError::new("resource_limit"));
        }
        tickets.insert(
            nonce.clone(),
            Ticket {
                at: now,
                source: source.into(),
                action: action.into(),
                format: format.into(),
                bytes: text.as_bytes().to_vec(),
                inventory,
            },
        );
        Ok(
            json!({"nonce":nonce,"content":text,"source":source,"action":action,"format":format,"expiresAt":now+600,"suggestedName":format!("in-line-{source}-{now}.{format}"),"prompt":if source=="faults"{"请检查我确认导出的In Line脱敏故障摘要。区分事实和假设，先核实故障版本和当前版本，在独立worktree与隔离数据库复现，提出最小修复和回归方案。不得读取Token、扩大权限、自动修复业务、盲目重放写入或自行发布。"}else{""}}),
        )
    }
    pub fn cancel(&self, nonce: &str) {
        if let Ok(mut t) = self.tickets.lock() {
            t.remove(nonce);
        }
    }
    pub fn confirm(
        &self,
        security: &Security,
        db: &Database,
        nonce: &str,
        path: Option<&str>,
        now: i64,
    ) -> Result<Value, McpError> {
        let mut tickets = self
            .tickets
            .lock()
            .map_err(|_| McpError::new("internal_error"))?;
        let t = tickets
            .get(nonce)
            .ok_or_else(|| McpError::new("not_found"))?;
        if now < t.at || now - t.at >= 600 {
            tickets.remove(nonce);
            return Err(McpError::new("snapshot_expired"));
        }
        let result = if t.action == "clear" {
            security.jobs.initialize(db, None)?;
            security
                .jobs
                .with_no_active(|| db.mcp_clear_audit_confirmed(&t.inventory))?
        } else {
            let path = Path::new(path.ok_or_else(|| McpError::new("invalid_arguments"))?);
            if !path.is_absolute()
                || path.extension().and_then(|p| p.to_str()) != Some(t.format.as_str())
            {
                return Err(McpError::new("invalid_arguments"));
            }
            platform::check_path(path)?;
            let parent = path
                .parent()
                .ok_or_else(|| McpError::new("invalid_arguments"))?
                .canonicalize()
                .map_err(|_| McpError::new("invalid_arguments"))?;
            for protected in [security.private_root(), db.mcp_maintenance_root()] {
                if let Ok(p) = protected.canonicalize() {
                    if parent.starts_with(p) {
                        return Err(McpError::new("forbidden"));
                    }
                }
            }
            use std::os::windows::fs::OpenOptionsExt;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .read(true)
                .create_new(true)
                .share_mode(windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ)
                .open(path)
                .map_err(|_| McpError::new("conflict"))?;
            let written = (|| {
                platform::Descriptor::new(false)?.apply(path)?;
                f.write_all(&t.bytes)
                    .and_then(|_| f.sync_all())
                    .map_err(|_| McpError::new("internal_error"))?;
                use std::io::{Read, Seek};
                f.rewind().map_err(|_| McpError::new("internal_error"))?;
                let mut actual = Vec::new();
                f.take((t.bytes.len() + 1) as u64)
                    .read_to_end(&mut actual)
                    .map_err(|_| McpError::new("internal_error"))?;
                if actual != t.bytes {
                    return Err(McpError::new("verification_failed"));
                }
                Ok(())
            })();
            if let Err(e) = written {
                return Err(e);
            }
            json!({"saved":true,"bytes":t.bytes.len(),"readbackMatchesPreview":true,"source":t.source,"hashManifestCreated":false})
        };
        tickets.remove(nonce);
        Ok(result)
    }
}
fn csv(value: &str) -> String {
    let mut s = value.to_owned();
    if s.trim_start().starts_with(['=', '+', '-', '@']) || s.starts_with(['\t', '\r', '\n']) {
        s.insert(0, '\'');
    }
    format!("\"{}\"", s.replace('"', "\"\""))
}
fn csv_export(content: &Value, source: &str) -> Result<String, McpError> {
    let mut out = String::from("\u{feff}record,metadata,summary\r\n");
    out += &format!(
        "{}, {}, {}\r\n",
        csv("export"),
        csv(&json!({"exportedAt":content["exportedAt"],"exportBuild":content["exportBuild"],"facts":content["facts"],"hypotheses":content["hypotheses"],"storageComplete":content["storageComplete"]}).to_string()),
        csv("技术结果不单独证明根因；业务审计仅限用户主动本地导出")
    );
    for item in content["items"]
        .as_array()
        .ok_or_else(|| McpError::new("internal_error"))?
    {
        if source == "faults" {
            for episode in item["episodes"]
                .as_array()
                .ok_or_else(|| McpError::new("internal_error"))?
            {
                out+=&format!("{},{},{}\r\n",csv(&item["id"].to_string()),csv(&json!({"client":item["client"],"tool":item["tool"],"module":item["module"],"code":item["code"],"severity":item["severity"],"attention":item["attention"]}).to_string()),csv(&episode.to_string()));
            }
        } else {
            out += &format!(
                "{},{},{}\r\n",
                csv(&item["id"].to_string()),
                csv(&item["createdAt"].to_string()),
                csv(&item.to_string())
            );
        }
    }
    Ok(out)
}
