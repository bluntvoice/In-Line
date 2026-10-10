//! Local, short-lived handoff. Long-lived credentials never cross the UI boundary.
use super::{
    contract::{Credentials, McpError},
    platform,
    security::{Grant, Issued, Permissions, Scope, Security},
};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

pub const TICKET_SECONDS: i64 = 600;
pub const CODEX_CURRENT_SESSION_VERIFIED: bool = false;
const MAX_BYTES: u64 = 3 * 1024 * 1024;
const RETENTION_SECONDS: i64 = 7 * 24 * 60 * 60;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub available: bool,
    pub status: &'static str,
}
pub fn presets() -> Vec<ClientPreset> {
    vec![ClientPreset {
        id: "codex",
        label: "Codex",
        available: CODEX_CURRENT_SESSION_VERIFIED,
        status: "自动接入验证中",
    }]
}
pub fn require_available(client: &str) -> Result<(), McpError> {
    if client != "codex" || !CODEX_CURRENT_SESSION_VERIFIED {
        return Err(McpError::new("onboarding_unavailable"));
    }
    Ok(())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareArgs {
    pub client: String,
    pub permissions: Permissions,
    pub scope: Scope,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub package_id: String,
    pub client_id: String,
    pub expires_at: i64,
    pub prompt: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Ticket {
    pub version: u32,
    pub package_id: String,
    pub client: String,
    pub machine: String,
    pub sid: String,
    pub issued_at: i64,
    pub expires_at: i64,
    pub command: PathBuf,
    pub credentials: Credentials,
}
pub(crate) fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
pub(crate) fn private_directory(path: &Path) -> Result<(), McpError> {
    if !path.is_absolute() {
        return Err(McpError::new("import_invalid"));
    }
    platform::check_path(path)?;
    if path.exists() {
        platform::Descriptor::new(true)?.verify(path)?;
    } else {
        std::fs::create_dir_all(path).map_err(|_| McpError::new("security_unavailable"))?;
        platform::Descriptor::new(true)?.apply(path)?;
    }
    Ok(())
}
pub(crate) fn write_private(path: &Path, bytes: &[u8]) -> Result<(), McpError> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err(McpError::new("resource_limit"));
    }
    platform::check_path(path)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| McpError::new("import_conflict"))?;
    platform::Descriptor::new(false)?.apply(path)?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| McpError::new("security_unavailable"))
}
pub(crate) fn read_private(path: &Path) -> Result<Vec<u8>, McpError> {
    platform::check_path(path)?;
    platform::Descriptor::new(false)?.verify(path)?;
    if std::fs::metadata(path)
        .map_err(|_| McpError::new("import_invalid"))?
        .len()
        > MAX_BYTES
    {
        return Err(McpError::new("import_invalid"));
    }
    let data = std::fs::read(path).map_err(|_| McpError::new("import_invalid"))?;
    if data.len() as u64 > MAX_BYTES {
        return Err(McpError::new("import_invalid"));
    }
    Ok(data)
}
pub(crate) fn ticket_directory(root: &Path, id: &str) -> Result<PathBuf, McpError> {
    if !valid_id(id) {
        return Err(McpError::new("import_invalid"));
    }
    let dir = root.join("onboarding").join(id);
    platform::check_path(&dir)?;
    platform::Descriptor::new(true)?.verify(&root.join("onboarding"))?;
    platform::Descriptor::new(true)?.verify(&dir)?;
    Ok(dir)
}
fn prune(root: &Path, now: i64) -> Result<(), McpError> {
    let mut count = 0;
    for entry in std::fs::read_dir(root).map_err(|_| McpError::new("security_unavailable"))? {
        let entry = entry.map_err(|_| McpError::new("security_unavailable"))?;
        count += 1;
        if count > 100 {
            return Err(McpError::new("resource_limit"));
        }
        let id = entry.file_name().to_string_lossy().to_string();
        if !valid_id(&id) {
            return Err(McpError::new("security_unavailable"));
        }
        let dir = entry.path();
        platform::check_path(&dir)?;
        platform::Descriptor::new(true)?.verify(&dir)?;
        let ticket_path = dir.join("ticket.dpapi");
        if ticket_path.is_file() {
            let bytes = platform::seal(&read_private(&ticket_path)?, true)?;
            let ticket: Ticket = serde_json::from_slice(&bytes)
                .map_err(|_| McpError::new("security_unavailable"))?;
            if ticket.package_id != id
                || ticket.machine != platform::machine_id()?
                || ticket.sid != platform::user_sid()?
            {
                return Err(McpError::new("security_unavailable"));
            }
            if now >= ticket.expires_at {
                std::fs::remove_file(&ticket_path)
                    .map_err(|_| McpError::new("security_unavailable"))?;
                if std::fs::remove_dir(&dir).is_ok() {
                    count -= 1;
                    continue;
                }
            }
        }
        let age = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|t| now.saturating_sub(t.as_secs() as i64))
            .unwrap_or(0);
        // Delete only our known files and a checked direct child, never a recursive tree.
        if age > RETENTION_SECONDS {
            for name in [
                "ticket.dpapi",
                "attempt.lock",
                "result.json",
                "config-backup.dpapi",
            ] {
                let file = dir.join(name);
                platform::check_path(&file)?;
                if file.is_file() {
                    let _ = std::fs::remove_file(file);
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
fn issue(root: &Path, command: &Path, issued: Issued, now: i64) -> Result<Receipt, McpError> {
    validate_command(command)?;
    let home = root.join("onboarding");
    private_directory(&home)?;
    prune(&home, now)?;
    let id = platform::random_secret()?;
    let dir = home.join(&id);
    private_directory(&dir)?;
    let ticket = Ticket {
        version: 1,
        package_id: id.clone(),
        client: "codex".into(),
        machine: platform::machine_id()?,
        sid: platform::user_sid()?,
        issued_at: now,
        expires_at: now
            .checked_add(TICKET_SECONDS)
            .ok_or_else(|| McpError::new("import_invalid"))?,
        command: command.into(),
        credentials: Credentials {
            client_id: issued.client_id.clone(),
            token: issued.token,
        },
    };
    let bytes = serde_json::to_vec(&ticket).map_err(|_| McpError::new("security_unavailable"))?;
    let sealed = platform::seal(&bytes, false)?;
    if let Err(error) = write_private(&dir.join("ticket.dpapi"), &sealed) {
        let _ = std::fs::remove_file(dir.join("ticket.dpapi"));
        let _ = std::fs::remove_dir(dir);
        return Err(error);
    }
    let invocation = format!(
        "& '{}' --import-client {}",
        command.to_string_lossy().replace('\'', "''"),
        id
    );
    let prompt = format!("我已在 In-Line 中选择 Codex 并明确授予本次客户端权限。请在本机 PowerShell 仅执行以下接入命令（10分钟内有效、只执行一次）：\n{invocation}\n凭证由程序在本机处理。不要打开、复制、输出或上传接入包、配置或备份，不要将凭证放入启动参数或系统环境变量。不要改变授权、暂停设置或其他客户端配置。仅报告程序的脱敏 JSON 结果；configurationVerified 与 currentSessionVerified 必须分别判断。只有当前 Codex 会话实际调用 get_capabilities 并核对结果中的 clientId、权限及范围后，才可报告当前会话可用；不得用另起实例代替。若当前客户端无法自动重载，应如实报告尚未验证，不要求我手填或重连，不重复执行导入命令。");
    Ok(Receipt {
        package_id: id,
        client_id: issued.client_id,
        expires_at: ticket.expires_at,
        prompt,
    })
}
pub(crate) fn validate_command(command: &Path) -> Result<(), McpError> {
    platform::check_path(command)?;
    if !command.is_absolute()
        || !command.is_file()
        || command.file_name().and_then(|n| n.to_str()) != Some("in-line-mcp.exe")
    {
        return Err(McpError::new("import_invalid"));
    }
    Ok(())
}
/// The UI command enforces availability; this core is exercised in isolated tests now.
pub fn prepare_at(
    security: &Security,
    root: &Path,
    command: &Path,
    args: PrepareArgs,
    now: i64,
) -> Result<Receipt, McpError> {
    if args.client != "codex" {
        return Err(McpError::new("onboarding_unavailable"));
    }
    validate_command(command)?;
    let mut created = None;
    let result = security.grant_with(
        Grant {
            name: "Codex".into(),
            permissions: args.permissions,
            scope: args.scope,
        },
        Some("codex"),
        |issued| {
            let receipt = issue(root, command, issued, now)?;
            created = Some(receipt.package_id.clone());
            Ok(receipt)
        },
    );
    if result.is_err() {
        discard_failed(root, created.as_deref());
    }
    result
}
fn discard_failed(root: &Path, id: Option<&str>) {
    if let Some(id) = id {
        if let Ok(dir) = ticket_directory(root, id) {
            let _ = std::fs::remove_file(dir.join("ticket.dpapi"));
            let _ = std::fs::remove_dir(dir);
        }
    }
}
pub fn rotate_at(
    security: &Security,
    root: &Path,
    command: &Path,
    id: &str,
    now: i64,
) -> Result<Receipt, McpError> {
    if !security
        .view()?
        .clients
        .iter()
        .any(|client| client.id == id && client.kind.as_deref() == Some("codex") && !client.revoked)
    {
        return Err(McpError::new("onboarding_unavailable"));
    }
    validate_command(command)?;
    let mut created = None;
    let result = security.rotate_with(id, |issued| {
        let receipt = issue(root, command, issued, now)?;
        created = Some(receipt.package_id.clone());
        Ok(receipt)
    });
    if result.is_err() {
        discard_failed(root, created.as_deref());
    }
    result
}
pub(crate) fn load_ticket(root: &Path, id: &str, now: i64) -> Result<(Ticket, PathBuf), McpError> {
    let dir = ticket_directory(root, id)?;
    if dir.join("result.json").exists() {
        return Err(McpError::new("import_used"));
    }
    if dir.join("attempt.lock").exists() {
        return Err(McpError::new("import_in_doubt"));
    }
    let sealed = read_private(&dir.join("ticket.dpapi"))?;
    if sealed.len() > 128 * 1024 {
        return Err(McpError::new("import_invalid"));
    }
    let bytes = platform::seal(&sealed, true)?;
    let ticket: Ticket =
        serde_json::from_slice(&bytes).map_err(|_| McpError::new("import_invalid"))?;
    if ticket.version != 1
        || ticket.package_id != id
        || ticket.client != "codex"
        || ticket.machine != platform::machine_id()?
        || ticket.sid != platform::user_sid()?
        || !valid_id(&ticket.credentials.client_id)
        || !valid_id(&ticket.credentials.token)
    {
        return Err(McpError::new("import_invalid"));
    }
    if ticket.issued_at > now
        || ticket.expires_at
            != ticket
                .issued_at
                .checked_add(TICKET_SECONDS)
                .ok_or_else(|| McpError::new("import_invalid"))?
        || now >= ticket.expires_at
    {
        return Err(McpError::new("import_expired"));
    }
    validate_command(&ticket.command)?;
    Ok((ticket, dir))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub struct Fixture {
        pub root: PathBuf,
        pub security: Security,
        pub command: PathBuf,
        pub home: PathBuf,
    }
    impl Fixture {
        pub fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "inline-onboarding-{}",
                platform::random_secret().unwrap()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let security = Security::open_at(root.join("security")).unwrap();
            let command = root.join("space ' & client").join("in-line-mcp.exe");
            std::fs::create_dir_all(command.parent().unwrap()).unwrap();
            std::fs::write(&command, b"synthetic invalid executable").unwrap();
            let home = root.join("codex");
            std::fs::create_dir_all(&home).unwrap();
            Self {
                root,
                security,
                command,
                home,
            }
        }
        pub fn security_root(&self) -> PathBuf {
            self.root.join("security")
        }
        pub fn prepare(&self) -> Receipt {
            prepare_at(
                &self.security,
                &self.security_root(),
                &self.command,
                PrepareArgs {
                    client: "codex".into(),
                    permissions: Permissions::default(),
                    scope: Scope::default(),
                },
                1000,
            )
            .unwrap()
        }
        pub fn replace_ticket(&self, id: &str, change: impl FnOnce(&mut Ticket)) {
            let (mut ticket, dir) = load_ticket(&self.security_root(), id, 1000).unwrap();
            change(&mut ticket);
            let sealed = platform::seal(&serde_json::to_vec(&ticket).unwrap(), false).unwrap();
            std::fs::write(dir.join("ticket.dpapi"), sealed).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let resolved = std::fs::canonicalize(&self.root).unwrap();
            let temp = std::fs::canonicalize(std::env::temp_dir()).unwrap();
            if resolved.starts_with(temp)
                && self
                    .root
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("inline-onboarding-")
            {
                let _ = std::fs::remove_dir_all(&self.root);
            }
        }
    }
    #[test]
    fn receipt_has_no_long_credential_and_ticket_is_encrypted_private_and_bound() {
        let f = Fixture::new();
        let receipt = f.prepare();
        let (ticket, dir) = load_ticket(&f.security_root(), &receipt.package_id, 1000).unwrap();
        let text = serde_json::to_string(&receipt).unwrap();
        assert!(!text.contains(&ticket.credentials.token));
        assert!(!text.contains("IN_LINE_MCP_TOKEN"));
        assert!(receipt.prompt.contains("space '' & client"));
        assert!(receipt.prompt.contains("--import-client"));
        assert!(receipt.prompt.contains("currentSessionVerified"));
        assert_eq!(receipt.expires_at, 1600);
        assert!(f
            .security
            .view()
            .unwrap()
            .clients
            .iter()
            .any(|c| c.name == "Codex" && c.id == receipt.client_id));
        platform::Descriptor::new(false)
            .unwrap()
            .verify(&dir.join("ticket.dpapi"))
            .unwrap();
        assert!(
            !String::from_utf8_lossy(&std::fs::read(dir.join("ticket.dpapi")).unwrap())
                .contains(&ticket.credentials.token)
        );
    }
    #[test]
    fn expiration_and_backward_clock_are_rejected_at_exact_boundary() {
        let f = Fixture::new();
        let r = f.prepare();
        assert!(load_ticket(&f.security_root(), &r.package_id, 1599).is_ok());
        assert_eq!(
            load_ticket(&f.security_root(), &r.package_id, 1600)
                .err()
                .unwrap()
                .code,
            "import_expired"
        );
        assert_eq!(
            load_ticket(&f.security_root(), &r.package_id, 999)
                .err()
                .unwrap()
                .code,
            "import_expired"
        );
    }
    #[test]
    fn binding_and_format_tampering_are_rejected() {
        for kind in 0..5 {
            let f = Fixture::new();
            let r = f.prepare();
            f.replace_ticket(&r.package_id, |t| match kind {
                0 => t.machine = "other-machine".into(),
                1 => t.sid = "other-user".into(),
                2 => t.client = "other-client".into(),
                3 => t.package_id = platform::random_secret().unwrap(),
                _ => t.version = 999,
            });
            assert_eq!(
                load_ticket(&f.security_root(), &r.package_id, 1000)
                    .err()
                    .unwrap()
                    .code,
                "import_invalid"
            );
        }
        let f = Fixture::new();
        let r = f.prepare();
        let dir = ticket_directory(&f.security_root(), &r.package_id).unwrap();
        std::fs::write(dir.join("ticket.dpapi"), b"not DPAPI").unwrap();
        assert!(load_ticket(&f.security_root(), &r.package_id, 1000).is_err());
        assert!(load_ticket(&f.security_root(), "../authorization.dpapi", 1000).is_err());
    }
    #[test]
    fn interrupted_or_completed_ticket_cannot_be_replayed() {
        let f = Fixture::new();
        let r = f.prepare();
        let dir = ticket_directory(&f.security_root(), &r.package_id).unwrap();
        write_private(&dir.join("attempt.lock"), b"attempted").unwrap();
        assert_eq!(
            load_ticket(&f.security_root(), &r.package_id, 1000)
                .err()
                .unwrap()
                .code,
            "import_in_doubt"
        );
        write_private(&dir.join("result.json"), b"{}").unwrap();
        assert_eq!(
            load_ticket(&f.security_root(), &r.package_id, 1000)
                .err()
                .unwrap()
                .code,
            "import_used"
        );
    }
    #[test]
    fn package_failure_does_not_create_or_rotate_authorization() {
        let f = Fixture::new();
        std::fs::write(f.security_root().join("onboarding"), b"occupied").unwrap();
        let before = std::fs::read(f.security_root().join("authorization.dpapi")).unwrap();
        assert!(prepare_at(
            &f.security,
            &f.security_root(),
            &f.command,
            PrepareArgs {
                client: "codex".into(),
                permissions: Permissions::default(),
                scope: Scope::default()
            },
            1000
        )
        .is_err());
        assert!(f.security.view().unwrap().clients.is_empty());
        assert_eq!(
            before,
            std::fs::read(f.security_root().join("authorization.dpapi")).unwrap()
        );
        let issued = f
            .security
            .grant(Grant {
                name: "existing".into(),
                permissions: Permissions::default(),
                scope: Scope::default(),
            })
            .unwrap();
        assert!(rotate_at(
            &f.security,
            &f.security_root(),
            &f.command,
            &issued.client_id,
            1000
        )
        .is_err());
        Security::read_at(&f.security_root())
            .unwrap()
            .authorize(&Credentials {
                client_id: issued.client_id,
                token: issued.token,
            })
            .unwrap();
    }
    #[test]
    fn production_gate_rejects_unverified_or_unknown_clients() {
        assert!(!presets()[0].available);
        assert!(require_available("codex").is_err());
        assert!(require_available("other").is_err());
    }
    #[test]
    fn preparing_a_new_ticket_cleans_expired_unused_ciphertext_without_revoking_client() {
        let f = Fixture::new();
        let old = f.prepare();
        let next = prepare_at(
            &f.security,
            &f.security_root(),
            &f.command,
            PrepareArgs {
                client: "codex".into(),
                permissions: Permissions::default(),
                scope: Scope::default(),
            },
            1600,
        )
        .unwrap();
        assert!(!f
            .security_root()
            .join("onboarding")
            .join(&old.package_id)
            .exists());
        assert_eq!(f.security.view().unwrap().clients.len(), 2);
        assert!(load_ticket(&f.security_root(), &next.package_id, 1600).is_ok());
    }
}
