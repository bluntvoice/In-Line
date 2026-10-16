//! A user-authorized import may refresh existing stdio connections belonging to
//! that same Codex process. No anonymous grant, network endpoint or token output.
use super::{
    contract::{Credentials, McpError},
    durable, onboarding, platform,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

const DIRECTORY: &str = "runtime-credentials";
const MAX_BYTES: usize = 32 * 1024;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Host {
    pid: u32,
    created: u64,
    executable: PathBuf,
}
#[derive(Clone)]
struct Target {
    host: Host,
    sidecar: PathBuf,
    binary_hash: String,
}
impl Target {
    fn discover(allow_ancestors: bool) -> Result<Self, McpError> {
        let sidecar = std::fs::canonicalize(std::env::current_exe().map_err(|_| unavailable())?)
            .map_err(|_| unavailable())?;
        platform::check_path(&sidecar)?;
        let host = find_codex_host(allow_ancestors)?;
        let binary_hash = format!(
            "{:x}",
            Sha256::digest(std::fs::read(&sidecar).map_err(|_| unavailable())?)
        );
        Ok(Self {
            host,
            sidecar,
            binary_hash,
        })
    }
    fn id(&self) -> Result<String, McpError> {
        Ok(format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(&self.host, &self.sidecar, &self.binary_hash))
                    .map_err(|_| unavailable())?
            )
        ))
    }
    fn alive(&self) -> bool {
        process(self.host.pid).is_ok_and(|(current, _)| current == self.host)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Binding {
    version: u32,
    machine: String,
    sid: String,
    host: Host,
    sidecar: PathBuf,
    binary_hash: String,
    predecessor_client_id: Option<String>,
    credentials: Credentials,
}
fn unavailable() -> McpError {
    McpError::new("security_unavailable")
}
fn directory(root: &Path, create: bool) -> Result<Option<PathBuf>, McpError> {
    let dir = root.join(DIRECTORY);
    platform::check_path(&dir)?;
    if dir.exists() {
        platform::Descriptor::new(true)?.verify(&dir)?;
        Ok(Some(dir))
    } else if create {
        Ok(Some(durable::root(root, DIRECTORY)?))
    } else {
        Ok(None)
    }
}
fn read(dir: &Path, target: &Target) -> Result<Option<Binding>, McpError> {
    let id = target.id()?;
    let file = dir.join(format!("{id}.json"));
    platform::check_path(&file)?;
    if !file.exists() {
        return Ok(None);
    }
    let sealed: Vec<u8> = durable::read(dir, &id, MAX_BYTES)?;
    let clear = platform::seal(&sealed, true)?;
    let binding: Binding = serde_json::from_slice(&clear).map_err(|_| unavailable())?;
    if binding.version != 1
        || binding.host != target.host
        || binding.sidecar != target.sidecar
        || binding.binary_hash != target.binary_hash
        || !target.alive()
        || binding.machine != platform::machine_id()?
        || binding.sid != platform::user_sid()?
        || !onboarding::valid_id(&binding.credentials.client_id)
        || !onboarding::valid_id(&binding.credentials.token)
        || binding
            .predecessor_client_id
            .as_deref()
            .is_some_and(|id| !onboarding::valid_id(id))
    {
        return Err(unavailable());
    }
    Ok(Some(binding))
}

pub struct Provider {
    credentials: Mutex<Credentials>,
    target: Option<Target>,
}
impl Provider {
    pub fn new(credentials: Credentials) -> Self {
        Self {
            credentials: Mutex::new(credentials),
            target: Target::discover(false).ok(),
        }
    }
    pub fn resolve(&self) -> Result<(Credentials, bool), McpError> {
        self.resolve_at(&platform::root()?)
    }
    fn resolve_at(&self, root: &Path) -> Result<(Credentials, bool), McpError> {
        let mut current = self.credentials.lock().map_err(|_| unavailable())?;
        let empty = current.client_id.is_empty() && current.token.is_empty();
        // A partially configured identity never acquires a different identity.
        if !empty
            && (!onboarding::valid_id(&current.client_id) || !onboarding::valid_id(&current.token))
        {
            return Ok((current.clone(), false));
        }
        let Some(target) = &self.target else {
            return Ok((current.clone(), false));
        };
        let Some(dir) = directory(root, false)? else {
            return Ok((current.clone(), false));
        };
        let Some(binding) = read(&dir, target)? else {
            return Ok((current.clone(), false));
        };
        let related = if empty {
            binding.predecessor_client_id.is_none()
        } else {
            current.client_id == binding.credentials.client_id
                || binding.predecessor_client_id.as_ref() == Some(&current.client_id)
        };
        if !related {
            return Ok((current.clone(), false));
        }
        // A fresh environment after an explicit rotation must not be replaced
        // by a stale handoff. Revoked/paused identities still reach IPC so its
        // precise authorization error is preserved.
        if super::security::Security::read_at(root)?
            .credential_owner(&binding.credentials)
            .is_none()
        {
            return Ok((current.clone(), false));
        }
        *current = binding.credentials;
        // IPC independently validates the current authorization, revocation,
        // pause, scope and revision on every call, including after this refresh.
        Ok((current.clone(), true))
    }
}

/// Only called after the one-use ticket and the written config pass validation.
/// An unidentified/ambiguous client remains pending, never guessed or restarted.
pub(crate) fn publish(
    root: &Path,
    original: Option<&[u8]>,
    ticket: &onboarding::Ticket,
) -> Result<bool, McpError> {
    let target = match Target::discover(true) {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    if target.sidecar != std::fs::canonicalize(&ticket.command).map_err(|_| unavailable())? {
        return Ok(false);
    }
    let doc = std::str::from_utf8(original.unwrap_or_default())
        .map_err(|_| unavailable())?
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| unavailable())?;
    let mut predecessor = None;
    if let Some(servers) = doc.get("mcp_servers").and_then(toml_edit::Item::as_table) {
        for (name, value) in servers {
            let Some(command) = value.get("command").and_then(toml_edit::Item::as_str) else {
                continue;
            };
            // Several same-executable server entries cannot be identified by
            // parent process alone. Preserve their separate identities.
            if name != "in_line"
                && Path::new(command)
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("in-line-mcp.exe"))
            {
                return Ok(false);
            }
        }
        if let Some(entry) = servers.get("in_line") {
            let env = entry.get("env");
            let cid = env
                .and_then(|e| e.get("IN_LINE_MCP_CLIENT_ID"))
                .and_then(toml_edit::Item::as_str);
            let token = env
                .and_then(|e| e.get("IN_LINE_MCP_TOKEN"))
                .and_then(toml_edit::Item::as_str);
            match (cid, token) {
                (None, None) => {}
                (Some(id), Some(token))
                    if onboarding::valid_id(id) && onboarding::valid_id(token) =>
                {
                    predecessor = Some(id.to_owned())
                }
                _ => return Ok(false),
            }
        }
    }
    write_binding(root, &target, ticket, predecessor)
}
fn write_binding(
    root: &Path,
    target: &Target,
    ticket: &onboarding::Ticket,
    predecessor: Option<String>,
) -> Result<bool, McpError> {
    let binding = Binding {
        version: 1,
        machine: platform::machine_id()?,
        sid: platform::user_sid()?,
        host: target.host.clone(),
        sidecar: target.sidecar.clone(),
        binary_hash: target.binary_hash.clone(),
        predecessor_client_id: predecessor,
        credentials: ticket.credentials.clone(),
    };
    let dir = directory(root, true)?.ok_or_else(unavailable)?;
    // Refuse to erase an existing malformed binding.
    let _ = read(&dir, &target)?;
    let sealed = platform::seal(
        &serde_json::to_vec(&binding).map_err(|_| unavailable())?,
        false,
    )?;
    durable::write(&dir, &target.id()?, &sealed, MAX_BYTES, 1024 * 1024, 64)?;
    let checked = read(&dir, &target)?.ok_or_else(unavailable)?;
    if checked.credentials.client_id != ticket.credentials.client_id
        || !super::security::constant_equal(&checked.credentials.token, &ticket.credentials.token)
    {
        return Err(unavailable());
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context(f: &onboarding::tests::Fixture) -> Target {
        Target {
            host: process(std::process::id()).unwrap().0,
            sidecar: std::fs::canonicalize(&f.command).unwrap(),
            binary_hash: format!("{:x}", Sha256::digest(std::fs::read(&f.command).unwrap())),
        }
    }
    fn blank() -> Credentials {
        Credentials {
            client_id: String::new(),
            token: String::new(),
        }
    }
    #[test]
    fn refresh_is_bound_to_the_user_authorized_identity_and_encrypted_at_rest() {
        let f = onboarding::tests::Fixture::new();
        let r = f.prepare();
        let (ticket, _) = onboarding::load_ticket(&f.security_root(), &r.package_id, 1000).unwrap();
        let target = context(&f);
        let p = Provider {
            credentials: Mutex::new(blank()),
            target: Some(target.clone()),
        };
        assert!(p
            .resolve_at(&f.security_root())
            .unwrap()
            .0
            .client_id
            .is_empty());
        assert!(write_binding(&f.security_root(), &target, &ticket, None).unwrap());
        let (actual, refreshed) = p.resolve_at(&f.security_root()).unwrap();
        assert!(refreshed);
        assert_eq!(actual.client_id, ticket.credentials.client_id);
        assert!(super::super::security::constant_equal(
            &actual.token,
            &ticket.credentials.token
        ));
        let file = f
            .security_root()
            .join(DIRECTORY)
            .join(format!("{}.json", target.id().unwrap()));
        let stored = std::fs::read_to_string(&file).unwrap();
        assert!(!stored.contains(&ticket.credentials.token));
        let unrelated = Provider {
            credentials: Mutex::new(Credentials {
                client_id: "f".repeat(64),
                token: "e".repeat(64),
            }),
            target: Some(target.clone()),
        };
        let (actual, refreshed) = unrelated.resolve_at(&f.security_root()).unwrap();
        assert!(!refreshed);
        assert_eq!(actual.client_id, "f".repeat(64));
        let partial = Provider {
            credentials: Mutex::new(Credentials {
                client_id: "f".repeat(64),
                token: String::new(),
            }),
            target: Some(target),
        };
        assert!(!partial.resolve_at(&f.security_root()).unwrap().1);
        let rotated = f.security.rotate(&ticket.credentials.client_id).unwrap();
        let fresh = Provider {
            credentials: Mutex::new(Credentials {
                client_id: rotated.client_id,
                token: rotated.token.clone(),
            }),
            target: Some(context(&f)),
        };
        let (actual, refreshed) = fresh.resolve_at(&f.security_root()).unwrap();
        assert!(!refreshed);
        assert!(super::super::security::constant_equal(
            &actual.token,
            &rotated.token
        ));
    }
    #[test]
    fn pid_reuse_wrong_binary_and_corruption_never_acquire_a_binding() {
        let f = onboarding::tests::Fixture::new();
        let r = f.prepare();
        let (ticket, _) = onboarding::load_ticket(&f.security_root(), &r.package_id, 1000).unwrap();
        let target = context(&f);
        write_binding(&f.security_root(), &target, &ticket, None).unwrap();
        let dir = f.security_root().join(DIRECTORY);
        let ciphertext: Vec<u8> = durable::read(&dir, &target.id().unwrap(), MAX_BYTES).unwrap();
        for mode in 0..2 {
            let mut other = target.clone();
            if mode == 0 {
                other.host.created += 1
            } else {
                other.binary_hash = "0".repeat(64)
            }
            durable::write(
                &dir,
                &other.id().unwrap(),
                &ciphertext,
                MAX_BYTES,
                1024 * 1024,
                64,
            )
            .unwrap();
            let provider = Provider {
                credentials: Mutex::new(blank()),
                target: Some(other),
            };
            assert!(provider.resolve_at(&f.security_root()).is_err());
        }
        let file = dir.join(format!("{}.json", target.id().unwrap()));
        std::fs::write(&file, b"corrupt").unwrap();
        let provider = Provider {
            credentials: Mutex::new(blank()),
            target: Some(target.clone()),
        };
        assert!(provider.resolve_at(&f.security_root()).is_err());
        assert!(write_binding(&f.security_root(), &target, &ticket, None).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), b"corrupt");
    }
}

fn find_codex_host(allow_ancestors: bool) -> Result<Host, McpError> {
    let (_, mut parent) = process(std::process::id())?;
    let allowed = std::fs::canonicalize(
        dirs::data_local_dir()
            .ok_or_else(unavailable)?
            .join("OpenAI/Codex/bin"),
    )
    .ok();
    for _ in 0..8 {
        let (host, next) = process(parent)?;
        #[cfg(debug_assertions)]
        if let Some(expected) = std::env::var_os("IN_LINE_MCP_TEST_CLIENT_EXE") {
            if std::fs::canonicalize(expected).ok().as_ref() == Some(&host.executable) {
                return Ok(host);
            }
        }
        if host
            .executable
            .file_name()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("codex.exe"))
            && allowed
                .as_ref()
                .is_some_and(|root| host.executable.starts_with(root))
        {
            platform::check_path(&host.executable)?;
            return Ok(host);
        }
        if next == 0 || next == parent {
            break;
        }
        if !allow_ancestors {
            break;
        }
        parent = next;
    }
    Err(unavailable())
}
fn process(pid: u32) -> Result<(Host, u32), McpError> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME, INVALID_HANDLE_VALUE},
        System::{Diagnostics::ToolHelp::*, Threading::*},
    };
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(unavailable());
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut more = Process32FirstW(snapshot, &mut entry);
        let mut parent = None;
        while more != 0 {
            if entry.th32ProcessID == pid {
                parent = Some(entry.th32ParentProcessID);
                break;
            }
            more = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        let parent = parent.ok_or_else(unavailable)?;
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return Err(unavailable());
        }
        let mut created: FILETIME = std::mem::zeroed();
        let mut exit: FILETIME = std::mem::zeroed();
        let mut kernel: FILETIME = std::mem::zeroed();
        let mut user: FILETIME = std::mem::zeroed();
        let times = GetProcessTimes(handle, &mut created, &mut exit, &mut kernel, &mut user);
        let mut path = vec![0u16; 32768];
        let mut length = path.len() as u32;
        let image = QueryFullProcessImageNameW(handle, 0, path.as_mut_ptr(), &mut length);
        CloseHandle(handle);
        if times == 0 || image == 0 {
            return Err(unavailable());
        }
        let executable = std::fs::canonicalize(PathBuf::from(String::from_utf16_lossy(
            &path[..length as usize],
        )))
        .map_err(|_| unavailable())?;
        Ok((
            Host {
                pid,
                created: ((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64,
                executable,
            },
            parent,
        ))
    }
}
