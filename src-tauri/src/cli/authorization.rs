//! Local GUI issuance and one-use CLI import. No grant endpoint is exposed to CLI.
use super::profile::Profile;
use crate::mcp::{
    contract::*,
    onboarding::{self, PrepareArgs, Receipt},
    platform,
    security::{Grant, Issued, Security},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

const TTL: i64 = 600;
#[cfg(test)]
#[path = "authorization_tests.rs"]
mod tests;
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Ticket {
    version: u32,
    purpose: String,
    package_id: String,
    selected_client: String,
    issued_at: i64,
    expires_at: i64,
    previous_hash: Option<String>,
    profile: Profile,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn is_cli(client: &str) -> bool {
    matches!(client, "codex-cli" | "powershell-cli")
}
pub(crate) fn executable() -> Result<PathBuf, McpError> {
    let path = std::env::current_exe()
        .map_err(|_| McpError::new("host_unavailable"))?
        .with_file_name("in-line-cli.exe");
    checked_executable(&path)
}
fn checked_executable(path: &Path) -> Result<PathBuf, McpError> {
    platform::check_path(path)?;
    if !path.is_absolute()
        || !path.is_file()
        || path.file_name().and_then(|s| s.to_str()) != Some("in-line-cli.exe")
    {
        return Err(McpError::new("onboarding_unavailable"));
    }
    std::fs::canonicalize(path).map_err(|_| McpError::new("onboarding_unavailable"))
}
pub(crate) fn presets() -> Vec<onboarding::ClientPreset> {
    let available = executable().is_ok();
    [
        ("codex-cli", "Codex（CLI，独立授权）"),
        ("powershell-cli", "PowerShell（CLI，独立授权）"),
    ]
    .into_iter()
    .map(|(id, label)| onboarding::ClientPreset {
        id,
        label,
        available: false,
        authorization_available: available,
        status: if available {
            "独立授权，导入后单独验证"
        } else {
            "未安装 CLI 程序"
        },
    })
    .collect()
}
fn lock(root: &Path) -> Result<std::fs::File, McpError> {
    platform::Descriptor::new(true)?.verify(root)?;
    let home = root.join("cli-profiles");
    onboarding::private_directory(&home)?;
    let file = home.join("import.lock");
    platform::check_path(&file)?;
    if !file.exists() {
        onboarding::write_private(&file, b"")?;
    }
    platform::Descriptor::new(false)?.verify(&file)?;
    let handle = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(file)
        .map_err(|_| McpError::new("import_conflict"))?;
    if handle
        .metadata()
        .map_err(|_| McpError::new("import_conflict"))?
        .len()
        != 0
    {
        return Err(McpError::new("import_conflict"));
    }
    Ok(handle)
}
fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, McpError> {
    platform::check_path(path)?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = onboarding::read_private(path)?;
    if bytes.len() > 32768 {
        return Err(McpError::new("import_invalid"));
    }
    Ok(Some(bytes))
}
fn directory(root: &Path, id: &str) -> Result<PathBuf, McpError> {
    if !onboarding::valid_id(id) {
        return Err(McpError::new("import_invalid"));
    }
    let home = root.join("cli-onboarding");
    let dir = home.join(id);
    platform::check_path(&dir)?;
    platform::Descriptor::new(true)?.verify(&home)?;
    platform::Descriptor::new(true)?.verify(&dir)?;
    Ok(dir)
}
fn prune(home: &Path, now: i64) -> Result<(), McpError> {
    let mut count = 0;
    for entry in std::fs::read_dir(home).map_err(|_| McpError::new("security_unavailable"))? {
        let entry = entry.map_err(|_| McpError::new("security_unavailable"))?;
        count += 1;
        if count > 100 {
            return Err(McpError::new("resource_limit"));
        }
        if !onboarding::valid_id(&entry.file_name().to_string_lossy()) {
            return Err(McpError::new("security_unavailable"));
        }
        let dir = entry.path();
        platform::check_path(&dir)?;
        platform::Descriptor::new(true)?.verify(&dir)?;
        let age = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|t| now.saturating_sub(t.as_secs() as i64))
            .unwrap_or(0);
        if age > 7 * 86400 {
            // Only checked direct children and known files; never recursive deletion.
            for name in [
                "ticket.dpapi",
                "attempt.lock",
                "result.json",
                "profile-backup.dpapi",
            ] {
                let path = dir.join(name);
                platform::check_path(&path)?;
                if path.is_file() {
                    platform::Descriptor::new(false)?.verify(&path)?;
                    std::fs::remove_file(path)
                        .map_err(|_| McpError::new("security_unavailable"))?;
                }
            }
            if std::fs::remove_dir(&dir).is_ok() {
                count -= 1;
            }
        }
    }
    if count >= 100 {
        return Err(McpError::new("resource_limit"));
    }
    Ok(())
}
fn issue(
    root: &Path,
    command: &Path,
    client: &str,
    issued: Issued,
    now: i64,
) -> Result<Receipt, McpError> {
    let _guard = lock(root)?;
    let executable = checked_executable(command)?;
    let home = root.join("cli-onboarding");
    onboarding::private_directory(&home)?;
    prune(&home, now)?;
    let name = format!("cli-{}", &issued.client_id[..32]);
    let previous = read_optional(&root.join("cli-profiles").join(format!("{name}.dpapi")))?;
    let id = platform::random_secret()?;
    let dir = home.join(&id);
    onboarding::private_directory(&dir)?;
    let ticket = Ticket {
        version: 1,
        purpose: "cli".into(),
        package_id: id.clone(),
        selected_client: client.into(),
        issued_at: now,
        expires_at: now
            .checked_add(TTL)
            .ok_or_else(|| McpError::new("import_invalid"))?,
        previous_hash: previous.as_ref().map(|v| hash(v)),
        profile: Profile {
            version: 1,
            purpose: "cli".into(),
            name: name.clone(),
            machine: platform::machine_id()?,
            sid: platform::user_sid()?,
            binary_hash: hash(
                &std::fs::read(&executable).map_err(|_| McpError::new("import_invalid"))?,
            ),
            executable,
            software_version: env!("CARGO_PKG_VERSION").into(),
            credentials: Credentials {
                client_id: issued.client_id.clone(),
                token: issued.token,
            },
        },
    };
    let sealed = platform::seal(
        &serde_json::to_vec(&ticket).map_err(|_| McpError::new("import_invalid"))?,
        false,
    )?;
    if let Err(e) = onboarding::write_private(&dir.join("ticket.dpapi"), &sealed) {
        let _ = std::fs::remove_file(dir.join("ticket.dpapi"));
        let _ = std::fs::remove_dir(dir);
        return Err(e);
    }
    let exe = command.to_string_lossy().replace('\'', "''");
    let prompt = format!("我已在 In-Line 中选择 {client} 并按所选权限独立授权。请在本机 PowerShell 10分钟内仅执行一次：\n& '{exe}' client import --package {id} --format json\n凭证仅由本机程序处理，不打开、复制、输出或上传授权包、profile、备份或配置，不把凭证放入环境变量、启动参数或聊天。成功后读取返回的 profileName，后续命令显式使用 --profile {name} --format json；先调用 capabilities 核对 clientId、权限和范围。失败时报告脱敏错误，不重复导入、不切换到其他身份、不增权、不修改数据。CLI连接验证与当前聊天MCP会话认证分别记录；不得以CLI验证冒充当前聊天MCP接入成功。写入必须另有我的明确指令、独立写权限、--execute、原因、意图、版本及原幂等键。实际办理必须由我确认，不能猜联系人、截止时间或新义务；创建前检索候选，多个候选先消歧。任何不明结果都只能先用原键执行receipt get，不盲目重放；历史撤销只能申请，由我在软件内批准。CLI客户端名称只用于独立授权分类，不表示Codex配置已自动重载。");
    Ok(Receipt {
        package_id: id,
        client_id: issued.client_id,
        expires_at: ticket.expires_at,
        prompt,
    })
}
pub(crate) fn prepare_at(
    security: &Security,
    root: &Path,
    command: &Path,
    args: PrepareArgs,
    now: i64,
) -> Result<Receipt, McpError> {
    if security.private_root() != root {
        return Err(McpError::new("security_unavailable"));
    }
    if !is_cli(&args.client) {
        return Err(McpError::new("onboarding_unavailable"));
    }
    checked_executable(command)?;
    let mut created = None;
    let result = security.grant_with(
        Grant {
            name: match args.client.as_str() {
                "codex-cli" => "Codex CLI",
                _ => "PowerShell CLI",
            }
            .into(),
            permissions: args.permissions,
            scope: args.scope,
        },
        Some("cli"),
        |issued| {
            let receipt = issue(root, command, &args.client, issued, now)?;
            created = Some(receipt.package_id.clone());
            Ok(receipt)
        },
    );
    if result.is_err() {
        discard(root, created.as_deref());
    }
    result
}
fn discard(root: &Path, id: Option<&str>) {
    if let Some(id) = id {
        if let Ok(dir) = directory(root, id) {
            let _ = std::fs::remove_file(dir.join("ticket.dpapi"));
            let _ = std::fs::remove_dir(dir);
        }
    }
}
pub(crate) fn rotate_at(
    security: &Security,
    root: &Path,
    command: &Path,
    id: &str,
    now: i64,
) -> Result<Receipt, McpError> {
    if security.private_root() != root {
        return Err(McpError::new("security_unavailable"));
    }
    let view = security.view()?;
    let client = view
        .clients
        .iter()
        .find(|c| c.id == id && c.kind.as_deref() == Some("cli") && !c.revoked)
        .ok_or_else(|| McpError::new("onboarding_unavailable"))?;
    let selected = if client.name == "Codex CLI" {
        "codex-cli"
    } else if client.name == "PowerShell CLI" {
        "powershell-cli"
    } else {
        return Err(McpError::new("onboarding_unavailable"));
    };
    let mut created = None;
    let result = security.rotate_with(id, |issued| {
        let r = issue(root, command, selected, issued, now)?;
        created = Some(r.package_id.clone());
        Ok(r)
    });
    if result.is_err() {
        discard(root, created.as_deref());
    }
    result
}
fn load(root: &Path, id: &str, exe: &Path, now: i64) -> Result<(Ticket, PathBuf), McpError> {
    let dir = directory(root, id)?;
    if dir.join("result.json").exists() {
        return Err(McpError::new("import_used"));
    }
    if dir.join("attempt.lock").exists() {
        return Err(McpError::new("import_in_doubt"));
    }
    let sealed = onboarding::read_private(&dir.join("ticket.dpapi"))?;
    if sealed.len() > 32768 {
        return Err(McpError::new("import_invalid"));
    }
    let t: Ticket = serde_json::from_slice(&platform::seal(&sealed, true)?)
        .map_err(|_| McpError::new("import_invalid"))?;
    let p = &t.profile;
    if t.version != 1
        || t.purpose != "cli"
        || t.package_id != id
        || !is_cli(&t.selected_client)
        || p.version != 1
        || p.purpose != "cli"
        || !super::parse::valid_profile(&p.name)
        || !onboarding::valid_id(&p.credentials.client_id)
        || !onboarding::valid_id(&p.credentials.token)
        || p.name != format!("cli-{}", &p.credentials.client_id[..32])
        || p.machine != platform::machine_id()?
        || p.sid != platform::user_sid()?
        || p.executable != checked_executable(exe)?
        || p.binary_hash != hash(&std::fs::read(exe).map_err(|_| McpError::new("import_invalid"))?)
        || p.software_version != env!("CARGO_PKG_VERSION")
    {
        return Err(McpError::new("import_invalid"));
    }
    check_time(&t, now)?;
    let store = Security::read_at(root)?;
    store.authorize(&p.credentials)?;
    let expected_name = if t.selected_client == "codex-cli" {
        "Codex CLI"
    } else {
        "PowerShell CLI"
    };
    if !store.clients.iter().any(|c| {
        c.id == p.credentials.client_id
            && c.kind.as_deref() == Some("cli")
            && c.name == expected_name
    }) {
        return Err(McpError::new("forbidden"));
    }
    Ok((t, dir))
}
fn check_time(t: &Ticket, now: i64) -> Result<(), McpError> {
    if t.issued_at < 0
        || t.issued_at > now
        || t.issued_at.checked_add(TTL) != Some(t.expires_at)
        || now >= t.expires_at
    {
        return Err(McpError::new("import_expired"));
    }
    Ok(())
}
fn install(path: &Path, bytes: &[u8], original: &Option<Vec<u8>>) -> Result<(), McpError> {
    let temp = path.with_file_name(format!("{}.tmp", platform::random_secret()?));
    let result = (|| {
        onboarding::write_private(&temp, bytes)?;
        if read_optional(path)? != *original {
            return Err(McpError::new("import_conflict"));
        }
        if original.is_some() {
            platform::atomic_replace(&temp, path)?;
        } else {
            std::fs::rename(&temp, path).map_err(|_| McpError::new("import_conflict"))?;
        }
        if read_optional(path)?.as_deref() != Some(bytes) {
            return Err(McpError::new("import_in_doubt"));
        }
        Ok(())
    })();
    if temp.exists() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
pub(super) async fn import(id: &str, request_id: String) -> Result<Value, McpError> {
    let root = platform::root()?;
    let exe = std::env::current_exe().map_err(|_| McpError::new("host_unavailable"))?;
    import_at(&root, id, &exe, request_id).await
}
async fn import_at(
    root: &Path,
    id: &str,
    exe: &Path,
    request_id: String,
) -> Result<Value, McpError> {
    let _guard = lock(root)?;
    let (ticket, dir) = load(root, id, exe, chrono::Utc::now().timestamp())?;
    let path = root
        .join("cli-profiles")
        .join(format!("{}.dpapi", ticket.profile.name));
    let original = read_optional(&path)?;
    if original.as_ref().map(|v| hash(v)) != ticket.previous_hash {
        return Err(McpError::new("import_conflict"));
    }
    let auth = Security::read_at(root)?.authorize(&ticket.profile.credentials)?;
    let candidate = platform::seal(
        &serde_json::to_vec(&ticket.profile).map_err(|_| McpError::new("import_invalid"))?,
        false,
    )?;
    if let Some(bytes) = &original {
        onboarding::write_private(&dir.join("profile-backup.dpapi"), bytes)?;
    }
    onboarding::write_private(&dir.join("attempt.lock"), b"")?;
    let outcome = async {
        install(&path, &candidate, &original)?;
        let credentials = super::profile::load_at(root, &ticket.profile.name, exe)?;
        let data = crate::mcp::ipc::call_at_with_id(root.into(), credentials.clone(), "get_capabilities", json!({}), request_id).await?;
        let verified = super::validate_status(data, &credentials.client_id)?;
        let current = Security::read_at(root)?.authorize(&credentials)?;
        if current.revision != auth.revision { return Err(McpError::new("authorization_changed")); }
        let capabilities = &verified["capabilities"];
        if capabilities["permissions"] != serde_json::to_value(&current.permissions).map_err(|_| McpError::new("import_failed"))?
            || capabilities["scope"] != serde_json::to_value(&current.scope).map_err(|_| McpError::new("import_failed"))?
            || capabilities["authorizationRevision"] != current.revision { return Err(McpError::new("import_failed")); }
        check_time(&ticket, chrono::Utc::now().timestamp())?;
        if read_optional(&path)?.as_ref() != Some(&candidate) { return Err(McpError::new("import_in_doubt")); }
        Ok(json!({"profileName":ticket.profile.name,"clientId":credentials.client_id,
            "selectedClient":ticket.selected_client,"configurationVerified":true,"cliConnectionVerified":true,
            "currentMcpSessionVerified":false,"connection":verified}))
    }.await;
    match outcome {
        Ok(value) => {
            if onboarding::write_private(
                &dir.join("result.json"),
                &serde_json::to_vec(&value).map_err(|_| McpError::new("import_in_doubt"))?,
            )
            .is_err()
            {
                return Err(McpError::new("import_in_doubt"));
            }
            let _ = std::fs::remove_file(dir.join("ticket.dpapi"));
            Ok(value)
        }
        Err(error) => {
            // Restore only the candidate written by this attempt. Preserve concurrent edits.
            let rolled_back = match read_optional(&path) {
                Ok(current) if current.as_ref() == Some(&candidate) => match &original {
                    Some(bytes) => install(&path, bytes, &Some(candidate)).is_ok(),
                    None => std::fs::remove_file(&path).is_ok(),
                },
                Ok(current) if current == original => true,
                _ => false,
            };
            let terminal = json!({"configurationVerified":false,"cliConnectionVerified":false,"currentMcpSessionVerified":false,"rolledBack":rolled_back,"errorCode":error.code});
            if onboarding::write_private(
                &dir.join("result.json"),
                &serde_json::to_vec(&terminal).unwrap_or_default(),
            )
            .is_err()
                || !rolled_back
            {
                return Err(McpError::new("import_in_doubt"));
            }
            Err(error)
        }
    }
}
