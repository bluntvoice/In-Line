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
    pub configuration_changed: bool,
    pub current_session_verified: bool,
    pub session_credential_refresh_prepared: bool,
    pub session_credential_refresh_error: Option<String>,
    pub permissions: super::security::Permissions,
    pub scope: super::security::Scope,
    pub authorization_revision: u64,
}
pub fn codex_home() -> Result<PathBuf, McpError> {
    #[cfg(debug_assertions)]
    if let Some(home) = std::env::var_os("IN_LINE_MCP_TEST_CODEX_HOME") {
        return Ok(home.into());
    }
    let home = std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or(
        dirs::home_dir()
            .ok_or_else(|| McpError::new("import_invalid"))?
            .join(".codex"),
    );
    // A selected user profile may itself be a junction after a drive migration.
    // Verify the resolved directory, keeping reparse checks on all files below it.
    std::fs::canonicalize(home).map_err(|_| McpError::new("import_invalid"))
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
fn configuration_conflict(reason: &str) -> McpError {
    let mut error = McpError::new("import_conflict");
    // Reasons are static categories, never a user-controlled key, value or path.
    let detail = match reason {
        "command_mismatch" => "In-Line 启动路径与本次接入不一致",
        "unsupported_option" => "In-Line 服务项含尚未支持的配置字段，需先核对兼容性",
        "unexpected_arguments" => "启动参数非空或类型无效",
        "server_disabled_or_invalid" => "服务已禁用或 enabled 类型无效",
        "unsafe_environment" => "环境设置含未允许的变量或非字符串值",
        "invalid_tool_policy" => "工具审批或输出设置的类型或取值无效",
        "concurrent_change" => "读取期间配置发生变化",
        _ => "超时或工具选项的类型或取值无效",
    };
    error.message.push_str(&format!("（{detail}）"));
    error
}
fn approval_mode(item: &Item) -> bool {
    matches!(
        item.as_str(),
        Some("auto" | "prompt" | "writes" | "approve")
    )
}
fn valid_seconds(item: &Item) -> bool {
    item.as_integer().is_some_and(|v| v >= 0)
        || item
            .as_float()
            .is_some_and(|v| std::time::Duration::try_from_secs_f64(v).is_ok())
}
fn tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 160
        && name.trim() == name
        && !name.chars().any(char::is_control)
}
fn validate_entry(entry: &dyn toml_edit::TableLike, command: &Path) -> Result<(), McpError> {
    if entry
        .get("command")
        .and_then(Item::as_str)
        .is_none_or(|v| !same_command(v, command))
    {
        return Err(configuration_conflict("command_mismatch"));
    }
    for (key, item) in entry.iter() {
        let safe = match key {
            "command" => true,
            "args" => item.as_array().is_some_and(|v| v.is_empty()),
            "enabled" => item.as_bool() == Some(true),
            "required" => item.is_bool(),
            "startup_timeout_sec" | "tool_timeout_sec" => valid_seconds(item),
            "startup_timeout_ms" => item.as_integer().is_some_and(|v| v >= 0),
            "default_tools_approval_mode" => approval_mode(item),
            "enabled_tools" | "disabled_tools" => item
                .as_array()
                .is_some_and(|a| a.iter().all(|v| v.as_str().is_some_and(tool_name))),
            "env" => item.as_table_like().is_some_and(|env| {
                env.iter().all(|(k, v)| {
                    ["IN_LINE_MCP_CLIENT_ID", "IN_LINE_MCP_TOKEN"].contains(&k) && v.is_str()
                })
            }),
            "tools" => item.as_table_like().is_some_and(|tools| {
                tools.iter().all(|(name, options)| {
                    tool_name(name)
                        && options.as_table_like().is_some_and(|options| {
                            options.iter().all(|(key, value)| match key {
                                "approval_mode" => approval_mode(value),
                                "output_token_limit" => value.as_integer().is_some_and(|v| v > 0),
                                _ => false,
                            })
                        })
                })
            }),
            _ => return Err(configuration_conflict("unsupported_option")),
        };
        if !safe {
            return Err(configuration_conflict(match key {
                "args" => "unexpected_arguments",
                "enabled" => "server_disabled_or_invalid",
                "env" => "unsafe_environment",
                "tools" | "default_tools_approval_mode" => "invalid_tool_policy",
                _ => "invalid_host_option",
            }));
        }
    }
    Ok(())
}
fn set_credential(table: &mut dyn toml_edit::TableLike, key: &str, text: &str) {
    if table.get(key).and_then(Item::as_str) == Some(text) {
        return;
    }
    let mut replacement = toml_edit::Value::from(text);
    if let Some(old) = table.get(key).and_then(Item::as_value) {
        // Credential strings are hex; retain a single-quoted user's item too.
        if old.to_string().trim_start().starts_with('\'') && !text.contains(['\'', '\r', '\n']) {
            replacement = format!("'{text}'")
                .parse()
                .expect("validated literal string");
        }
        *replacement.decor_mut() = old.decor().clone();
    }
    if let Some(slot) = table.get_mut(key) {
        *slot = Item::Value(replacement);
    } else {
        table.insert(key, Item::Value(replacement));
    }
}
fn table_block(item: &Item, text: &str) -> Result<std::ops::Range<usize>, McpError> {
    let invalid = || configuration_conflict("invalid_host_option");
    let mut range = item.span().ok_or_else(invalid)?;
    if let Some(table) = item.as_table() {
        // Table::span covers its header; values retain their own parser spans.
        for (_, value) in table.iter() {
            range.end = range.end.max(value.span().ok_or_else(invalid)?.end);
        }
        let tail = text.get(range.end..).ok_or_else(invalid)?;
        range.end += tail.find('\n').map_or(tail.len(), |n| n + 1);
    }
    Ok(range)
}
fn preserve_document_bytes(original: &str, rendered: &str) -> Result<Vec<u8>, McpError> {
    let invalid = || configuration_conflict("invalid_host_option");
    let old = toml_edit::Document::parse(original).map_err(|_| invalid())?;
    let new = toml_edit::Document::parse(rendered).map_err(|_| invalid())?;
    let server = |doc: &toml_edit::Document<&str>| {
        doc.as_table()
            .get("mcp_servers")
            .and_then(|s| s.get("in_line"))
            .cloned()
    };
    let old_server = server(&old);
    let new_server = server(&new).ok_or_else(invalid)?;
    let new_env = new_server.get("env").ok_or_else(invalid)?;
    let fragment = |range: std::ops::Range<usize>| -> Result<String, McpError> {
        let newline = if original
            .find('\n')
            .is_some_and(|n| n > 0 && original.as_bytes()[n - 1] == b'\r')
        {
            "\r\n"
        } else {
            "\n"
        };
        Ok(rendered
            .get(range)
            .ok_or_else(invalid)?
            .replace("\r\n", "\n")
            .replace('\n', newline))
    };
    let mut result = original.to_owned();
    if let Some(old_env) = old_server.as_ref().and_then(|s| s.get("env")) {
        let keys = ["IN_LINE_MCP_CLIENT_ID", "IN_LINE_MCP_TOKEN"];
        if keys.iter().all(|key| old_env.get(key).is_some()) {
            // Parser-derived value spans, never a textual search for secrets.
            // Keep every other byte, including mixed line endings and comments.
            let mut edits = Vec::new();
            for key in keys {
                let old_range = old_env.get(key).and_then(Item::span).ok_or_else(invalid)?;
                let new_range = new_env.get(key).and_then(Item::span).ok_or_else(invalid)?;
                if result.get(old_range.clone()).is_none() {
                    return Err(invalid());
                }
                edits.push((
                    old_range,
                    rendered.get(new_range).ok_or_else(invalid)?.to_owned(),
                ));
            }
            edits.sort_by_key(|(range, _)| range.start);
            if edits[0].0.end > edits[1].0.start {
                return Err(invalid());
            }
            for (range, value) in edits.into_iter().rev() {
                result.replace_range(range, &value);
            }
        } else {
            let range = table_block(old_env, original)?;
            if result.get(range.clone()).is_none() {
                return Err(invalid());
            }
            result.replace_range(range, &fragment(table_block(new_env, rendered)?)?);
        }
    } else if old_server.as_ref().is_some_and(Item::is_inline_table) {
        let range = old_server
            .as_ref()
            .and_then(Item::span)
            .ok_or_else(invalid)?;
        if result.get(range.clone()).is_none() {
            return Err(invalid());
        }
        result.replace_range(range, &fragment(new_server.span().ok_or_else(invalid)?)?);
    } else {
        // Missing managed tables are appended; existing user/server bytes stay
        // untouched, rather than being reserialized by DocumentMut.
        let start = if old_server.is_none() {
            new_server.span().ok_or_else(invalid)?.start
        } else {
            new_env.span().ok_or_else(invalid)?.start
        };
        let end = table_block(new_env, rendered)?.end;
        let newline = if original.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        if !result.is_empty() && !result.ends_with('\n') {
            result.push_str(newline);
        }
        result.push_str(&fragment(start..end)?);
        result.push_str(newline);
    }
    Ok(result.into_bytes())
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
    let unchanged = doc
        .get("mcp_servers")
        .and_then(|s| s.get("in_line"))
        .and_then(|s| s.get("env"))
        .is_some_and(|env| {
            env.get("IN_LINE_MCP_CLIENT_ID").and_then(Item::as_str)
                == Some(ticket.credentials.client_id.as_str())
                && env.get("IN_LINE_MCP_TOKEN").and_then(Item::as_str)
                    == Some(ticket.credentials.token.as_str())
        });
    if let Some(existing) = doc["mcp_servers"].as_table().and_then(|t| t.get("in_line")) {
        validate_entry(
            existing.as_table_like().ok_or_else(invalid)?,
            &ticket.command,
        )?;
    } else {
        let mut server = Table::new();
        server["command"] = value(ticket.command.to_str().ok_or_else(invalid)?);
        server["args"] = value(Array::new());
        doc["mcp_servers"]["in_line"] = Item::Table(server);
    }
    let server = doc["mcp_servers"]["in_line"]
        .as_table_like_mut()
        .ok_or_else(invalid)?;
    if !server.contains_key("env") {
        server.insert("env", Item::Table(Table::new()));
    }
    let env = server
        .get_mut("env")
        .and_then(Item::as_table_like_mut)
        .ok_or_else(invalid)?;
    set_credential(env, "IN_LINE_MCP_CLIENT_ID", &ticket.credentials.client_id);
    set_credential(env, "IN_LINE_MCP_TOKEN", &ticket.credentials.token);
    let result = if unchanged {
        original.to_vec()
    } else {
        preserve_document_bytes(text, &doc.to_string())?
    };
    if result.len() > 1024 * 1024 {
        return Err(invalid());
    }
    // Parse the actual bytes that will be written; never use textual substitution.
    let checked = std::str::from_utf8(&result)
        .map_err(|_| invalid())?
        .parse::<DocumentMut>()
        .map_err(|_| invalid())?;
    let checked_env = checked
        .get("mcp_servers")
        .and_then(|s| s.get("in_line"))
        .and_then(|s| s.get("env"));
    if !checked_env.is_some_and(|env| {
        env.get("IN_LINE_MCP_TOKEN").and_then(Item::as_str)
            == Some(ticket.credentials.token.as_str())
            && env.get("IN_LINE_MCP_CLIENT_ID").and_then(Item::as_str)
                == Some(ticket.credentials.client_id.as_str())
    }) {
        return Err(invalid());
    }
    Ok(result)
}

/// GUI-owned reuse reads only the selected, private in_line entry. It does not
/// scan tickets/bindings, guess a host identity, or change an authorization.
pub(crate) fn existing_credentials(
    home: &Path,
    command: &Path,
    id: &str,
) -> Result<super::contract::Credentials, McpError> {
    let unavailable = || McpError::new("onboarding_credentials_unavailable");
    platform::check_path(home)?;
    let path = home.join("config.toml");
    if !path.exists() {
        return Err(unavailable());
    }
    platform::Descriptor::new(false)?
        .verify(&path)
        .map_err(|_| unavailable())?;
    let original = read_config(&path)?.ok_or_else(unavailable)?;
    let doc = std::str::from_utf8(&original)
        .map_err(|_| unavailable())?
        .parse::<DocumentMut>()
        .map_err(|_| unavailable())?;
    let entry = doc
        .get("mcp_servers")
        .and_then(|v| v.get("in_line"))
        .and_then(Item::as_table_like)
        .ok_or_else(unavailable)?;
    validate_entry(entry, command)?;
    let env = entry
        .get("env")
        .and_then(Item::as_table_like)
        .ok_or_else(unavailable)?;
    let cid = env
        .get("IN_LINE_MCP_CLIENT_ID")
        .and_then(Item::as_str)
        .ok_or_else(unavailable)?;
    let token = env
        .get("IN_LINE_MCP_TOKEN")
        .and_then(Item::as_str)
        .ok_or_else(unavailable)?;
    if cid != id || !onboarding::valid_id(cid) || !onboarding::valid_id(token) {
        return Err(unavailable());
    }
    if read_config(&path)?.as_deref() != Some(original.as_slice()) {
        return Err(configuration_conflict("concurrent_change"));
    }
    Ok(super::contract::Credentials {
        client_id: cid.into(),
        token: token.into(),
    })
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
        return Err(McpError::new("import_authorization_missing"));
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
    if original.as_deref() == Some(candidate.as_slice()) {
        // A no-op must not preserve an already weakened credential-file ACL.
        // Reject before consuming the ticket; do not silently change the ACL.
        platform::Descriptor::new(false)?.verify(&path)?;
    }
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
        if original.as_deref() != Some(candidate.as_slice()) {
            install(&path, &candidate, &original)?;
            committed = true;
        }
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
        platform::Descriptor::new(false)?.verify(&path)?;
        let refreshed = super::runtime_credentials::publish(root, original.as_deref(), &ticket);
        let (session_credential_refresh_prepared, session_credential_refresh_error) =
            match refreshed {
                Ok(true) => (true, None),
                Ok(false) => (
                    false,
                    Some("client_instance_not_identified_or_ambiguous".into()),
                ),
                Err(error) => (false, Some(error.code)),
            };
        Ok(ImportReport {
            status: "configuration_verified_current_session_pending",
            client_id: ticket.credentials.client_id.clone(),
            configuration_verified: true,
            configuration_changed: committed,
            current_session_verified: false,
            session_credential_refresh_prepared,
            session_credential_refresh_error,
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
    fn merge_keeps_safe_host_policy_and_unchanged_bytes_including_inline_env() {
        let f = Fixture::new();
        let receipt = f.prepare();
        let (ticket, _) =
            onboarding::load_ticket(&f.security_root(), &receipt.package_id, 1000).unwrap();
        let source = format!(
            "# untouched\nmodel = 'chosen'\n[mcp_servers.other]\ncommand = 'other' # keep\n[mcp_servers.in_line] # server comment\ncommand = {} # command comment\nargs = [] # args comment\nenabled = true # enabled comment\nstartup_timeout_sec = 12.5 # timeout\ntool_timeout_sec = 45\nrequired = false\nenabled_tools = ['get_capabilities', 'query_tasks']\ndisabled_tools = ['mutate_task']\ndefault_tools_approval_mode = 'prompt' # policy\nenv = {{ IN_LINE_MCP_CLIENT_ID = '{}', IN_LINE_MCP_TOKEN = '{}' }} # env comment\n[mcp_servers.in_line.tools.query_tasks]\napproval_mode = 'approve' # per tool\noutput_token_limit = 3000\n",
            value(ticket.command.to_str().unwrap()), ticket.credentials.client_id, ticket.credentials.token
        );
        assert!(
            merge(source.as_bytes(), &ticket).is_ok(),
            "safe host policy rejected"
        );
        assert_eq!(
            merge(source.as_bytes(), &ticket).unwrap(),
            source.as_bytes()
        );
        let old = source.replace(&ticket.credentials.token, "synthetic-old-token");
        let merged = String::from_utf8(merge(old.as_bytes(), &ticket).unwrap()).unwrap();
        assert_eq!(merged, source);
        let crlf = source.replace('\n', "\r\n");
        assert_eq!(merge(crlf.as_bytes(), &ticket).unwrap(), crlf.as_bytes());
        let changed_crlf = old.replace('\n', "\r\n");
        assert_eq!(
            merge(changed_crlf.as_bytes(), &ticket).unwrap(),
            crlf.as_bytes()
        );
        for zero in [
            source.replace("12.5 # timeout", "0 # timeout"),
            source.replace("startup_timeout_sec = 12.5", "startup_timeout_ms = 0"),
            source.replace("tool_timeout_sec = 45", "tool_timeout_sec = 0.0"),
        ] {
            assert_eq!(merge(zero.as_bytes(), &ticket).unwrap(), zero.as_bytes());
        }
    }
    #[test]
    fn merge_adds_only_managed_sections_and_preserves_partial_env_comments() {
        let f = Fixture::new();
        let receipt = f.prepare();
        let (ticket, _) =
            onboarding::load_ticket(&f.security_root(), &receipt.package_id, 1000).unwrap();
        let command = value(ticket.command.to_str().unwrap()).to_string();
        let other = "[mcp_servers.other] # 保留\r\ncommand='other' # unchanged\r\n";
        for source in [
            other.to_owned(),
            format!("{other}[mcp_servers.in_line] # parent\r\ncommand={command} # command\r\n"),
        ] {
            let output = merge(source.as_bytes(), &ticket).unwrap();
            assert!(output.starts_with(source.as_bytes()));
        }
        for source in [
            format!("[mcp_servers.in_line]\r\ncommand={command}\r\n[mcp_servers.in_line.env]\r\nIN_LINE_MCP_TOKEN='old' # preserve exactly once\r\n{other}"),
            format!("[mcp_servers]\r\nin_line={{command={command}}} # preserve exactly once\r\nother={{command='other'}} # unchanged\r\n"),
        ] {
            let output = String::from_utf8(merge(source.as_bytes(), &ticket).unwrap()).unwrap();
            assert_eq!(output.matches("# preserve exactly once").count(), 1);
            let parsed = output.parse::<DocumentMut>().unwrap();
            assert_eq!(parsed["mcp_servers"]["other"]["command"].as_str(), Some("other"));
            assert!(output.contains("# unchanged\r\n"));
            assert_eq!(merge(output.as_bytes(), &ticket).unwrap(), output.as_bytes());
        }
    }
    #[test]
    fn merge_rejects_invalid_host_options_with_static_redacted_reasons() {
        let f = Fixture::new();
        let receipt = f.prepare();
        let (ticket, _) =
            onboarding::load_ticket(&f.security_root(), &receipt.package_id, 1000).unwrap();
        let base = String::from_utf8(merge(b"", &ticket).unwrap()).unwrap();
        for policy in [
            "startup_timeout_sec = -1",
            "tool_timeout_sec = nan",
            "startup_timeout_ms = -1",
            "startup_timeout_ms = 0.5",
            "required = 'yes'",
            "enabled_tools = [1]",
            "disabled_tools = 'query_tasks'",
            "tools = { query_tasks = {approval_mode = 'PROMPT'} }",
            "tools = { query_tasks = {output_token_limit = 0} }",
            "tools = { query_tasks = {unknown = true} }",
            "'synthetic-secret-key' = 'synthetic-secret-value'",
            "env_vars = ['IN_LINE_MCP_TEST_SECURITY_ROOT']",
        ] {
            let input = base.replace(
                "[mcp_servers.in_line]\n",
                &format!("[mcp_servers.in_line]\n{policy}\n"),
            );
            let error = merge(input.as_bytes(), &ticket).unwrap_err();
            assert_eq!(error.code, "import_conflict");
            assert!(!error.message.contains("synthetic-secret"));
            assert!(!error.message.contains(&ticket.credentials.token));
        }
    }
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
    fn merge_preserves_each_approval_policy_and_its_decoration() {
        let f = Fixture::new();
        let r = f.prepare();
        let (ticket, _) = onboarding::load_ticket(&f.security_root(), &r.package_id, 1000).unwrap();
        let base = String::from_utf8(
            merge(
                b"# keep\nmodel='chosen'\n[mcp_servers.other]\ncommand='other'\n",
                &ticket,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(!base.contains("default_tools_approval_mode"));
        for mode in ["auto", "prompt", "writes", "approve"] {
            for quote in ["'", "\""] {
                let policy = format!("# approval comment\ndefault_tools_approval_mode = {quote}{mode}{quote} # preserve policy\n");
                let source = base.replace(
                    "[mcp_servers.in_line]\n",
                    &format!("[mcp_servers.in_line]\n{policy}"),
                );
                let result = String::from_utf8(merge(source.as_bytes(), &ticket).unwrap()).unwrap();
                assert!(result.contains(&policy));
                let doc = result.parse::<DocumentMut>().unwrap();
                assert_eq!(
                    doc["mcp_servers"]["in_line"]["default_tools_approval_mode"].as_str(),
                    Some(mode)
                );
                assert_eq!(
                    doc["mcp_servers"]["other"]["command"].as_str(),
                    Some("other")
                );
                assert_eq!(doc["model"].as_str(), Some("chosen"));
                assert!(result.starts_with("# keep\n"));
                assert_eq!(
                    merge(result.as_bytes(), &ticket).unwrap(),
                    result.as_bytes()
                );
            }
        }
    }
    #[test]
    fn merge_rejects_invalid_approval_policy_types_values_and_unsafe_settings() {
        let f = Fixture::new();
        let r = f.prepare();
        let (ticket, _) = onboarding::load_ticket(&f.security_root(), &r.package_id, 1000).unwrap();
        let base = String::from_utf8(merge(b"", &ticket).unwrap()).unwrap();
        for invalid in [
            "true",
            "false",
            "1",
            "[]",
            "{}",
            "['prompt']",
            "''",
            "'PROMPT'",
            "'prompt '",
            "'unknown'",
        ] {
            let source = base.replace(
                "[mcp_servers.in_line]\n",
                &format!("[mcp_servers.in_line]\ndefault_tools_approval_mode = {invalid}\n"),
            );
            assert_eq!(
                merge(source.as_bytes(), &ticket).unwrap_err().code,
                "import_conflict"
            );
        }
        let safe = base.replace(
            "[mcp_servers.in_line]\n",
            "[mcp_servers.in_line]\ndefault_tools_approval_mode = 'prompt'\n",
        );
        for source in [
            safe.replace("args = []", "args = ['--unexpected']"),
            safe.replace(
                "[mcp_servers.in_line]\n",
                "[mcp_servers.in_line]\nenabled = false\n",
            ),
            safe.replace(
                "[mcp_servers.in_line]\n",
                "[mcp_servers.in_line]\nunknown_field = true\n",
            ),
            safe.replace(
                "[mcp_servers.in_line.env]\n",
                "[mcp_servers.in_line.env]\nPATH = 'unsafe'\n",
            ),
            safe.replace(
                "[mcp_servers.in_line.env]\n",
                "[mcp_servers.in_line.env]\nIN_LINE_MCP_CLIENT_ID_EXTRA = 'unsafe'\n",
            ),
            safe.replace(
                &format!("command = {}", value(ticket.command.to_str().unwrap())),
                "command = 'foreign.exe'",
            ),
            safe.replace(
                &format!("IN_LINE_MCP_TOKEN = {}", value(&ticket.credentials.token)),
                "IN_LINE_MCP_TOKEN = 1",
            ),
        ] {
            assert_ne!(source, safe);
            assert_eq!(
                merge(source.as_bytes(), &ticket).unwrap_err().code,
                "import_conflict"
            );
        }
        // An inline env table has the same strict boundary as a standard table.
        let command = value(ticket.command.to_str().unwrap()).to_string();
        for env in ["{ PATH = 'unsafe' }", "{ IN_LINE_MCP_TOKEN = false }"] {
            let source = format!("[mcp_servers.in_line]\ncommand = {command}\ndefault_tools_approval_mode = 'prompt'\nenv = {env}\n");
            assert_eq!(
                merge(source.as_bytes(), &ticket).unwrap_err().code,
                "import_conflict"
            );
        }
        let source = format!("[mcp_servers.in_line]\ncommand = {command}\ndefault_tools_approval_mode = 'writes'\nenv = {{ IN_LINE_MCP_TOKEN = 'synthetic-old-token' }}\n");
        assert!(merge(source.as_bytes(), &ticket).is_ok());
    }
    #[tokio::test]
    async fn stdio_failure_restores_existing_approval_policy_bytes() {
        let f = Fixture::new();
        let r = f.prepare();
        let (ticket, _) = onboarding::load_ticket(&f.security_root(), &r.package_id, 1000).unwrap();
        let base =
            String::from_utf8(merge(b"# synthetic rollback fixture\n", &ticket).unwrap()).unwrap();
        let original = base.replace("[mcp_servers.in_line]\n", "[mcp_servers.in_line]\n# preserve approval\ndefault_tools_approval_mode = 'approve' # unchanged\n").replace(&ticket.credentials.token, "synthetic-prior-token");
        let path = f.home.join("config.toml");
        onboarding::write_private(&path, original.as_bytes()).unwrap();
        let error = import_at(&f.security_root(), &f.home, &r.package_id, 1000)
            .await
            .err()
            .unwrap();
        assert_eq!(error.code, "import_failed");
        assert_eq!(std::fs::read(&path).unwrap(), original.as_bytes());
        platform::Descriptor::new(false)
            .unwrap()
            .verify(&path)
            .unwrap();
    }
    #[test]
    fn unchanged_import_rejects_nonprivate_acl_before_consuming_ticket() {
        let f = Fixture::new();
        let receipt = f.prepare();
        let (ticket, _) =
            onboarding::load_ticket(&f.security_root(), &receipt.package_id, 1000).unwrap();
        let config = merge(b"", &ticket).unwrap();
        let path = f.home.join("config.toml");
        std::fs::write(&path, &config).unwrap();
        assert!(platform::Descriptor::new(false)
            .unwrap()
            .verify(&path)
            .is_err());
        let identity = platform::file_location(&path).unwrap().1;
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let error = runtime
            .block_on(import_at(
                &f.security_root(),
                &f.home,
                &receipt.package_id,
                1000,
            ))
            .err()
            .unwrap();
        assert_eq!(error.code, "security_unavailable");
        assert!(!error.message.contains(&ticket.credentials.token));
        assert_eq!(platform::file_location(&path).unwrap().1, identity);
        assert_eq!(std::fs::read(&path).unwrap(), config);
        assert!(onboarding::load_ticket(&f.security_root(), &receipt.package_id, 1000).is_ok());
    }
    #[test]
    fn unchanged_configuration_keeps_private_file_identity_when_stdio_fails() {
        let f = Fixture::new();
        let receipt = f.prepare();
        let (ticket, _) =
            onboarding::load_ticket(&f.security_root(), &receipt.package_id, 1000).unwrap();
        let config = merge(b"", &ticket).unwrap();
        let path = f.home.join("config.toml");
        onboarding::write_private(&path, &config).unwrap();
        let identity = platform::file_location(&path).unwrap().1;
        let runtime = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(
            runtime
                .block_on(import_at(
                    &f.security_root(),
                    &f.home,
                    &receipt.package_id,
                    1000
                ))
                .err()
                .unwrap()
                .code,
            "import_failed"
        );
        assert_eq!(platform::file_location(&path).unwrap().1, identity);
        assert_eq!(std::fs::read(&path).unwrap(), config);
        platform::Descriptor::new(false)
            .unwrap()
            .verify(&path)
            .unwrap();
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
    async fn missing_persisted_grant_does_not_consume_ticket_or_touch_configuration() {
        let f = Fixture::new();
        let r = f.prepare();
        let (_, dir) = onboarding::load_ticket(&f.security_root(), &r.package_id, 1000).unwrap();
        f.security
            .change(|store| {
                store.clients.clear();
                Ok(())
            })
            .unwrap();
        let path = f.home.join("config.toml");
        let original = b"# preserved\nmodel='keep'\n";
        onboarding::write_private(&path, original).unwrap();
        let error = import_at(&f.security_root(), &f.home, &r.package_id, 1000)
            .await
            .err()
            .unwrap();
        assert_eq!(error.code, "import_authorization_missing");
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(dir.join("ticket.dpapi").exists());
        for name in ["attempt.lock", "result.json", "config-backup.dpapi"] {
            assert!(!dir.join(name).exists());
        }
        assert!(!f.home.join(".in-line-import.lock").exists());
        assert!(Security::read_at(&f.security_root())
            .unwrap()
            .clients
            .is_empty());
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
