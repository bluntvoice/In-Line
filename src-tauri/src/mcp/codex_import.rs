//! Codex user configuration adapter. No shell, global environment edits, or desktop restart.
use super::{
    contract::{McpError, API_VERSION, TOOLS},
    onboarding::{self, Ticket},
    platform,
    security::{Authorization, Security},
};
use rmcp::{model::CallToolRequestParams, transport::TokioChildProcess, ServiceExt};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use toml_edit::{value, Array, DocumentMut, Item, Table};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub status: &'static str,
    pub client_id: String,
    pub configuration_verified: bool,
    pub current_session_verified: bool,
    pub permissions: super::security::Permissions,
    pub scope: super::security::Scope,
    pub authorization_revision: u64,
}
pub fn codex_home() -> Result<PathBuf, McpError> {
    #[cfg(debug_assertions)]
    if let Some(home) = std::env::var_os("IN_LINE_MCP_TEST_CODEX_HOME") {
        return Ok(home.into());
    }
    Ok(std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or(
        dirs::home_dir()
            .ok_or_else(|| McpError::new("import_invalid"))?
            .join(".codex"),
    ))
}
fn same_command(a: &str, b: &Path) -> bool {
    let a = Path::new(a);
    if !a.is_absolute() || platform::check_path(a).is_err() {
        return false;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a
            .to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy()),
        _ => false,
    }
}
fn merge(original: &[u8], ticket: &Ticket) -> Result<Vec<u8>, McpError> {
    let invalid = || McpError::new("import_conflict");
    let text = std::str::from_utf8(original).map_err(|_| invalid())?;
    let mut doc = text.parse::<DocumentMut>().map_err(|_| invalid())?;
    if doc.contains_key("mcp_servers") && !doc["mcp_servers"].is_table() {
        return Err(invalid());
    }
    if !doc.contains_key("mcp_servers") {
        doc["mcp_servers"] = Item::Table(Table::new());
    }
    if let Some(existing) = doc["mcp_servers"].as_table().and_then(|t| t.get("in_line")) {
        let entry = existing.as_table().ok_or_else(invalid)?;
        if entry
            .iter()
            .any(|(key, _)| !["command", "args", "env", "enabled"].contains(&key))
            || entry
                .get("command")
                .and_then(Item::as_str)
                .is_none_or(|command| !same_command(command, &ticket.command))
            || entry
                .get("args")
                .is_some_and(|item| item.as_array().is_none_or(|args| !args.is_empty()))
            || entry
                .get("enabled")
                .is_some_and(|item| item.as_bool() != Some(true))
        {
            return Err(invalid());
        }
        if let Some(env) = entry.get("env") {
            let allowed = |key: &str| ["IN_LINE_MCP_CLIENT_ID", "IN_LINE_MCP_TOKEN"].contains(&key);
            let safe = if let Some(table) = env.as_table() {
                table.iter().all(|(k, v)| allowed(k) && v.is_str())
            } else if let Some(table) = env.as_inline_table() {
                table.iter().all(|(k, v)| allowed(k) && v.is_str())
            } else {
                false
            };
            if !safe {
                return Err(invalid());
            }
        }
    }
    let mut server = Table::new();
    server["command"] = value(ticket.command.to_str().ok_or_else(invalid)?);
    server["args"] = value(Array::new());
    let mut env = Table::new();
    env["IN_LINE_MCP_CLIENT_ID"] = value(&ticket.credentials.client_id);
    env["IN_LINE_MCP_TOKEN"] = value(&ticket.credentials.token);
    server["env"] = Item::Table(env);
    doc["mcp_servers"]["in_line"] = Item::Table(server);
    let result = doc.to_string().into_bytes();
    if result.len() > 1024 * 1024 {
        return Err(invalid());
    }
    // Parse the actual bytes that will be written; never use textual substitution.
    let checked = std::str::from_utf8(&result)
        .map_err(|_| invalid())?
        .parse::<DocumentMut>()
        .map_err(|_| invalid())?;
    if checked["mcp_servers"]["in_line"]["env"]["IN_LINE_MCP_TOKEN"].as_str()
        != Some(ticket.credentials.token.as_str())
    {
        return Err(invalid());
    }
    Ok(result)
}
fn lock(home: &Path) -> Result<std::fs::File, McpError> {
    if !home.is_absolute() || !home.is_dir() {
        return Err(McpError::new("import_invalid"));
    }
    platform::check_path(home)?;
    // An exclusive file handle survives async thread changes, and the OS releases
    // it after a crash. The empty private lock file is reusable, not a stale flag.
    use std::os::windows::fs::OpenOptionsExt;
    let path = home.join(".in-line-import.lock");
    platform::check_path(&path)?;
    if path.exists() {
        platform::Descriptor::new(false)?
            .verify(&path)
            .map_err(|_| McpError::new("import_conflict"))?;
    } else {
        onboarding::write_private(&path, b"")?;
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(&path)
        .map_err(|_| McpError::new("import_conflict"))?;
    if file
        .metadata()
        .map_err(|_| McpError::new("import_conflict"))?
        .len()
        != 0
    {
        return Err(McpError::new("import_conflict"));
    }
    Ok(file)
}
fn read_config(path: &Path) -> Result<Option<Vec<u8>>, McpError> {
    platform::check_path(path)?;
    match std::fs::metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Ok(m) if m.is_file() && m.len() <= 1024 * 1024 => {
            let bytes = std::fs::read(path).map_err(|_| McpError::new("import_conflict"))?;
            if bytes.len() > 1024 * 1024 {
                return Err(McpError::new("import_conflict"));
            }
            Ok(Some(bytes))
        }
        _ => Err(McpError::new("import_conflict")),
    }
}
fn install(path: &Path, bytes: &[u8], original: &Option<Vec<u8>>) -> Result<(), McpError> {
    let temp = path.with_file_name(format!(".in-line-{}.tmp", platform::random_secret()?));
    let result = (|| {
        onboarding::write_private(&temp, bytes)?;
        if read_config(path)? != *original {
            return Err(McpError::new("import_conflict"));
        }
        if original.is_some() {
            platform::atomic_replace(&temp, path)?;
        } else {
            // rename is non-overwriting on Windows: another creator cannot be overwritten.
            std::fs::rename(&temp, path).map_err(|_| McpError::new("import_conflict"))?;
        }
        if read_config(path)?.as_deref() != Some(bytes) {
            return Err(McpError::new("import_in_doubt"));
        }
        Ok(())
    })();
    let _ = std::fs::remove_file(temp);
    result
}
#[derive(Serialize, Deserialize)]
struct Backup {
    original: Option<String>,
}
fn authorize(root: &Path, ticket: &Ticket) -> Result<Authorization, McpError> {
    let store = Security::read_at(root)?;
    if !store.clients.iter().any(|client| {
        client.id == ticket.credentials.client_id && client.kind.as_deref() == Some("codex")
    }) {
        return Err(McpError::new("import_invalid"));
    }
    store.authorize(&ticket.credentials)
}
fn validate_capability(
    data: &serde_json::Value,
    ticket: &Ticket,
    auth: &Authorization,
) -> Result<(), McpError> {
    let permissions =
        serde_json::to_value(&auth.permissions).map_err(|_| McpError::new("import_failed"))?;
    let scope = serde_json::to_value(&auth.scope).map_err(|_| McpError::new("import_failed"))?;
    if data["clientId"] != ticket.credentials.client_id
        || data["permissions"] != permissions
        || data["scope"] != scope
        || data["authorizationRevision"] != auth.revision
        || data["mcpVersion"] != API_VERSION
        || data["softwareVersion"] != env!("CARGO_PKG_VERSION")
        || data["transport"] != "stdio"
        || data["tools"] != serde_json::json!(TOOLS)
    {
        return Err(McpError::new("import_failed"));
    }
    Ok(())
}
async fn verify_stdio(
    root: &Path,
    candidate: &[u8],
    ticket: &Ticket,
    auth: &Authorization,
) -> Result<(), McpError> {
    let doc = std::str::from_utf8(candidate)
        .map_err(|_| McpError::new("import_invalid"))?
        .parse::<DocumentMut>()
        .map_err(|_| McpError::new("import_invalid"))?;
    let env = &doc["mcp_servers"]["in_line"]["env"];
    let mut command = tokio::process::Command::new(&ticket.command);
    command
        .env(
            "IN_LINE_MCP_CLIENT_ID",
            env["IN_LINE_MCP_CLIENT_ID"]
                .as_str()
                .ok_or_else(|| McpError::new("import_invalid"))?,
        )
        .env(
            "IN_LINE_MCP_TOKEN",
            env["IN_LINE_MCP_TOKEN"]
                .as_str()
                .ok_or_else(|| McpError::new("import_invalid"))?,
        )
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    #[cfg(debug_assertions)]
    {
        command.env("IN_LINE_MCP_TEST_SECURITY_ROOT", root);
        if let Some(host) = std::env::var_os("IN_LINE_MCP_TEST_HOST_EXE") {
            command.env("IN_LINE_MCP_TEST_HOST_EXE", host);
        }
    }
    let _ = root;
    let (transport, _) = TokioChildProcess::builder(command)
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| McpError::new("import_failed"))?;
    let client = ().serve(transport).await.map_err(|_| McpError::new("import_failed"))?;
    let call = client
        .call_tool(
            CallToolRequestParams::new("get_capabilities").with_arguments(serde_json::Map::new()),
        )
        .await
        .map_err(|_| McpError::new("import_failed"))?;
    let result = call
        .structured_content
        .ok_or_else(|| McpError::new("import_failed"))?;
    let checked = if result["status"] == "ok" && call.is_error != Some(true) {
        validate_capability(&result["data"], ticket, auth)
    } else {
        let code = result["error"]["code"].as_str().unwrap_or("import_failed");
        let safe = [
            "paused",
            "revoked",
            "unauthenticated",
            "authorization_changed",
            "host_unavailable",
            "rate_limited",
        ]
        .contains(&code);
        Err(McpError::new(if safe { code } else { "import_failed" }))
    };
    let _ = client.cancel().await;
    checked
}
fn complete(dir: &Path, value: serde_json::Value) -> Result<(), McpError> {
    onboarding::write_private(
        &dir.join("result.json"),
        &serde_json::to_vec(&value).map_err(|_| McpError::new("import_failed"))?,
    )?;
    // The saved result contains no credential or configuration payload.
    let _ = std::fs::remove_file(dir.join("ticket.dpapi"));
    Ok(())
}
/// Only isolated tests may call the core before the UI/client support gate passes.
pub async fn import_at(
    root: &Path,
    home: &Path,
    id: &str,
    now: i64,
) -> Result<ImportReport, McpError> {
    let started = std::time::Instant::now();
    let (ticket, dir) = onboarding::load_ticket(root, id, now)?;
    let auth = authorize(root, &ticket)?;
    let _lock = lock(home)?;
    let path = home.join("config.toml");
    let original = read_config(&path)?;
    let original_acl = if original.is_some() {
        Some(platform::Descriptor::capture(&path)?)
    } else {
        None
    };
    let candidate = merge(original.as_deref().unwrap_or_default(), &ticket)?;
    onboarding::write_private(
        &dir.join("attempt.lock"),
        b"One import attempt; interruption requires inspection",
    )?;
    let mut committed = false;
    let result = async {
        let bytes = serde_json::to_vec(&Backup {
            original: original
                .as_ref()
                .map(|bytes| {
                    String::from_utf8(bytes.clone()).map_err(|_| McpError::new("import_invalid"))
                })
                .transpose()?,
        })
        .map_err(|_| McpError::new("import_failed"))?;
        onboarding::write_private(
            &dir.join("config-backup.dpapi"),
            &platform::seal(&bytes, false)?,
        )?;
        if authorize(root, &ticket)?.revision != auth.revision {
            return Err(McpError::new("authorization_changed"));
        }
        if now.saturating_add(started.elapsed().as_secs() as i64) >= ticket.expires_at {
            return Err(McpError::new("import_expired"));
        }
        install(&path, &candidate, &original)?;
        committed = true;
        let written = read_config(&path)?.ok_or_else(|| McpError::new("import_in_doubt"))?;
        tokio::time::timeout(
            std::time::Duration::from_secs(20),
            verify_stdio(root, &written, &ticket, &auth),
        )
        .await
        .map_err(|_| McpError::new("import_failed"))??;
        if authorize(root, &ticket)?.revision != auth.revision {
            return Err(McpError::new("authorization_changed"));
        }
        if now.saturating_add(started.elapsed().as_secs() as i64) >= ticket.expires_at {
            return Err(McpError::new("import_expired"));
        }
        if read_config(&path)?.as_deref() != Some(candidate.as_slice()) {
            return Err(McpError::new("import_in_doubt"));
        }
        Ok(ImportReport {
            status: "configuration_verified_current_session_pending",
            client_id: ticket.credentials.client_id.clone(),
            configuration_verified: true,
            current_session_verified: false,
            permissions: auth.permissions,
            scope: auth.scope,
            authorization_revision: auth.revision,
        })
    }
    .await;
    match result {
        Ok(report) => {
            complete(
                &dir,
                serde_json::to_value(&report).map_err(|_| McpError::new("import_failed"))?,
            )?;
            Ok(report)
        }
        Err(error) => {
            // Never restore over someone else's newer content. An interrupted/uncertain write is terminal.
            let current = read_config(&path)?;
            let rollback = if !committed && error.code == "import_conflict" {
                Ok(())
            } else if current == original {
                Ok(())
            } else if current.as_deref() == Some(candidate.as_slice()) {
                if let Some(bytes) = &original {
                    install(&path, bytes, &current).and_then(|_| {
                        original_acl
                            .as_ref()
                            .ok_or_else(|| McpError::new("import_in_doubt"))?
                            .apply(&path)
                    })
                } else {
                    std::fs::remove_file(&path).map_err(|_| McpError::new("import_in_doubt"))
                }
            } else {
                Err(McpError::new("import_in_doubt"))
            };
            let error = if rollback.is_ok() {
                error
            } else {
                McpError::new("import_in_doubt")
            };
            complete(
                &dir,
                serde_json::json!({"status":"error","error":error,"configurationVerified":false,"currentSessionVerified":false,"rolledBack":rollback.is_ok()}),
            )?;
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::onboarding::tests::Fixture;
    use super::*;
    #[test]
    fn merge_preserves_other_servers_settings_comments_and_uses_env_only() {
        let f = Fixture::new();
        let r = f.prepare();
        let (ticket, _) = onboarding::load_ticket(&f.security_root(), &r.package_id, 1000).unwrap();
        let source=b"# keep this comment\nmodel = 'chosen-model'\n[mcp_servers.other]\ncommand = 'another-program'\nargs = ['--safe']\n";
        let result = merge(source, &ticket).unwrap();
        let s = String::from_utf8(result).unwrap();
        let doc = s.parse::<DocumentMut>().unwrap();
        assert!(s.contains("# keep this comment"));
        assert_eq!(doc["model"].as_str(), Some("chosen-model"));
        assert_eq!(
            doc["mcp_servers"]["other"]["command"].as_str(),
            Some("another-program")
        );
        assert!(doc["mcp_servers"]["in_line"]["args"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(
            doc["mcp_servers"]["in_line"]["env"]["IN_LINE_MCP_TOKEN"].as_str(),
            Some(ticket.credentials.token.as_str())
        );
        assert_eq!(
            doc["mcp_servers"]["in_line"]["command"].as_str(),
            ticket.command.to_str()
        );
        assert!(merge(s.as_bytes(), &ticket).is_ok());
    }
    #[test]
    fn refuses_invalid_foreign_disabled_and_argument_bearing_same_name() {
        let f = Fixture::new();
        let r = f.prepare();
        let (ticket, _) = onboarding::load_ticket(&f.security_root(), &r.package_id, 1000).unwrap();
        for source in [
            "not toml = [",
            "mcp_servers = 1",
            "[mcp_servers.in_line]\ncommand='other.exe'",
            "[mcp_servers.in_line]\nurl='https://example.invalid'",
        ] {
            assert!(merge(source.as_bytes(), &ticket).is_err());
        }
        let base = String::from_utf8(merge(b"", &ticket).unwrap()).unwrap();
        for addition in [
            "enabled = false\n",
            "cwd = 'other'\n",
            "url = 'https://example.invalid'\n",
        ] {
            let text = base.replace(
                "[mcp_servers.in_line]\n",
                &format!("[mcp_servers.in_line]\n{addition}"),
            );
            assert!(merge(text.as_bytes(), &ticket).is_err());
        }
        assert!(merge(
            base.replace("args = []", "args = ['--unexpected']")
                .as_bytes(),
            &ticket
        )
        .is_err());
    }
    #[test]
    fn detects_concurrent_config_change_and_lock_is_reusable_after_release() {
        let f = Fixture::new();
        let path = f.home.join("config.toml");
        let original = Some(b"model='old'\n".to_vec());
        std::fs::write(&path, original.as_ref().unwrap()).unwrap();
        std::fs::write(&path, b"model='external-new'\n").unwrap();
        assert_eq!(
            install(&path, b"model='ours'\n", &original)
                .err()
                .unwrap()
                .code,
            "import_conflict"
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"model='external-new'\n");
        let held = lock(&f.home).unwrap();
        assert!(lock(&f.home).is_err());
        drop(held);
        assert!(lock(&f.home).is_ok());
    }
    #[tokio::test]
    async fn stdio_failure_restores_bytes_and_acl_consumes_ticket_and_leaks_no_credential() {
        let f = Fixture::new();
        let r = f.prepare();
        let (ticket, dir) =
            onboarding::load_ticket(&f.security_root(), &r.package_id, 1000).unwrap();
        let path = f.home.join("config.toml");
        let original = b"# original\nmodel='keep'\n";
        onboarding::write_private(&path, original).unwrap();
        let e = import_at(&f.security_root(), &f.home, &r.package_id, 1000)
            .await
            .err()
            .unwrap();
        assert_eq!(e.code, "import_failed");
        assert_eq!(std::fs::read(&path).unwrap(), original);
        platform::Descriptor::new(false)
            .unwrap()
            .verify(&path)
            .unwrap();
        let result = std::fs::read_to_string(dir.join("result.json")).unwrap();
        assert!(!result.contains(&ticket.credentials.token));
        assert!(result.contains("rolledBack"));
        assert!(!dir.join("ticket.dpapi").exists());
        assert_eq!(
            import_at(&f.security_root(), &f.home, &r.package_id, 1000)
                .await
                .err()
                .unwrap()
                .code,
            "import_used"
        );
        let backup: Backup = serde_json::from_slice(
            &platform::seal(
                &onboarding::read_private(&dir.join("config-backup.dpapi")).unwrap(),
                true,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(backup.original.unwrap().into_bytes(), original);
    }
    #[tokio::test]
    async fn revoked_rotated_and_paused_credentials_do_not_write_configuration() {
        for mode in 0..3 {
            let f = Fixture::new();
            let r = f.prepare();
            match mode {
                0 => f.security.revoke(&r.client_id).unwrap(),
                1 => {
                    f.security.rotate(&r.client_id).unwrap();
                }
                _ => f.security.pause(true).unwrap(),
            };
            let e = import_at(&f.security_root(), &f.home, &r.package_id, 1000)
                .await
                .err()
                .unwrap();
            assert_eq!(e.code, ["revoked", "unauthenticated", "paused"][mode]);
            assert!(!f.home.join("config.toml").exists());
            assert!(!f.home.join(".in-line-import.lock").exists());
        }
    }
    #[test]
    fn capability_requires_matching_identity_permissions_scope_and_version() {
        let f = Fixture::new();
        let r = f.prepare();
        let (ticket, _) = onboarding::load_ticket(&f.security_root(), &r.package_id, 1000).unwrap();
        let auth = authorize(&f.security_root(), &ticket).unwrap();
        let good = serde_json::json!({"clientId":ticket.credentials.client_id,"permissions":auth.permissions,"scope":auth.scope,"authorizationRevision":auth.revision,"mcpVersion":API_VERSION,"softwareVersion":env!("CARGO_PKG_VERSION"),"transport":"stdio","tools":TOOLS});
        assert!(validate_capability(&good, &ticket, &auth).is_ok());
        for key in [
            "clientId",
            "permissions",
            "scope",
            "authorizationRevision",
            "mcpVersion",
            "softwareVersion",
            "transport",
            "tools",
        ] {
            let mut wrong = good.clone();
            wrong[key] = serde_json::json!("wrong");
            assert!(validate_capability(&wrong, &ticket, &auth).is_err());
        }
    }
}
