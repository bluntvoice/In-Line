//! CLI credential adapter. GUI issuance/import uses the same bound profile.
//! No environment/default-profile fallback and no Codex runtime refresh provider.
use crate::mcp::{
    contract::{Credentials, McpError},
    platform,
    security::Security,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Profile {
    pub version: u32,
    pub purpose: String,
    pub name: String,
    pub machine: String,
    pub sid: String,
    pub executable: PathBuf,
    pub binary_hash: String,
    pub software_version: String,
    pub credentials: Credentials,
}
pub(super) fn load_at(root: &Path, name: &str, executable: &Path) -> Result<Credentials, McpError> {
    crate::mcp::storage::require_client_ready(root)?;
    let invalid = || McpError::new("security_unavailable");
    if !super::parse::valid_profile(name) {
        return Err(McpError::new("invalid_arguments"));
    }
    let dir = root.join("cli-profiles");
    let path = dir.join(format!("{name}.dpapi"));
    platform::check_path(&path)?;
    if !path.exists() {
        return Err(McpError::new("unauthenticated"));
    }
    platform::Descriptor::new(true)?.verify(root)?;
    platform::Descriptor::new(true)?.verify(&dir)?;
    platform::Descriptor::new(false)?.verify(&path)?;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| invalid())?
        .take(32769)
        .read_to_end(&mut bytes)
        .map_err(|_| invalid())?;
    if bytes.len() > 32768 {
        return Err(invalid());
    }
    let clear = platform::seal(&bytes, true)?;
    let p: Profile = serde_json::from_slice(&clear).map_err(|_| invalid())?;
    platform::check_path(executable)?;
    let executable = std::fs::canonicalize(executable).map_err(|_| invalid())?;
    let hash = format!(
        "{:x}",
        Sha256::digest(std::fs::read(&executable).map_err(|_| invalid())?)
    );
    if p.version != 1
        || p.purpose != "cli"
        || p.name != name
        || p.machine != platform::machine_id()?
        || p.sid != platform::user_sid()?
        || p.executable != executable
        || p.binary_hash != hash
        || p.software_version != env!("CARGO_PKG_VERSION")
    {
        return Err(invalid());
    }
    let store = Security::read_at(root)?;
    store.authorize(&p.credentials)?;
    if !store
        .clients
        .iter()
        .any(|c| c.id == p.credentials.client_id && c.kind.as_deref() == Some("cli"))
    {
        return Err(McpError::new("forbidden"));
    }
    Ok(p.credentials)
}
