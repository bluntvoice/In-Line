//! Explicit local diagnostics only: never open a ticket, client config or business database.
//! Secret fields stay inside the existing Security reader and are never included in this DTO.
use super::{
    contract::McpError,
    platform,
    security::{Security, SecurityView},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    pub found: bool,
    pub kind: Option<String>,
    pub revoked: Option<bool>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoreSummary {
    pub revision: u64,
    pub client_count: usize,
    pub selected_client: Option<Selection>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Storage {
    pub root: String,
    pub resolved_root: Option<String>,
    pub physical_file: Option<String>,
    pub redirected: Option<bool>,
    pub authorization_file_exists: bool,
    pub file_identity: Option<String>,
    pub file_bytes: Option<u64>,
    pub modified_unix_ms: Option<u64>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub diagnostic_only: bool,
    pub configuration_verified: bool,
    pub current_session_verified: bool,
    pub process_id: u32,
    pub executable: Option<String>,
    pub build: serde_json::Value,
    pub selected_client_fingerprint: Option<String>,
    pub storage: Storage,
    pub storage_stable_during_read: bool,
    pub memory: Option<StoreSummary>,
    pub disk: Option<StoreSummary>,
    pub memory_matches_disk: Option<bool>,
    pub disk_error_code: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub diagnostic_only: bool,
    pub configuration_verified: bool,
    pub current_session_verified: bool,
    pub gui: Snapshot,
    pub importer: Option<Snapshot>,
    pub importer_error_code: Option<String>,
    pub same_storage_file: Option<bool>,
    pub same_revision: Option<bool>,
    pub same_selected_client: Option<bool>,
    pub same_build: Option<bool>,
}
fn validate_selection(id: Option<&str>) -> Result<(), McpError> {
    if id.is_some_and(|id| !super::onboarding::valid_id(id)) {
        return Err(McpError::new("invalid_arguments"));
    }
    Ok(())
}
fn summary(store: &SecurityView, id: Option<&str>) -> StoreSummary {
    StoreSummary {
        revision: store.revision,
        client_count: store.clients.len(),
        selected_client: id.map(|id| {
            let client = store.clients.iter().find(|client| client.id == id);
            Selection {
                found: client.is_some(),
                // Unknown strings and client names can contain arbitrary input; never echo them.
                kind: client.and_then(|c| c.kind.as_deref()).map(|kind| {
                    match kind {
                        "codex" => "codex",
                        "cli" => "cli",
                        _ => "unknown",
                    }
                    .to_owned()
                }),
                revoked: client.map(|c| c.revoked),
            }
        }),
    }
}
fn storage(root: &Path) -> Storage {
    use std::time::UNIX_EPOCH;
    let path = root.join("authorization.dpapi");
    let metadata = std::fs::metadata(&path).ok();
    // Handles are used for file identity only; no bytes are read here.
    let location = platform::file_location(&path).ok();
    Storage {
        root: root.to_string_lossy().into_owned(),
        resolved_root: std::fs::canonicalize(root)
            .ok()
            .map(|p| p.to_string_lossy().into_owned()),
        physical_file: location
            .as_ref()
            .map(|(p, _)| p.to_string_lossy().into_owned()),
        redirected: location
            .as_ref()
            .map(|(p, _)| !platform::same_location_name(&path, p)),
        authorization_file_exists: metadata.as_ref().is_some_and(|m| m.is_file()),
        file_identity: location.map(|(_, id)| id),
        file_bytes: metadata.as_ref().map(|m| m.len()),
        modified_unix_ms: metadata
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .and_then(|d| u64::try_from(d.as_millis()).ok()),
    }
}
pub fn snapshot(
    root: &Path,
    security: Option<&Security>,
    id: Option<&str>,
) -> Result<Snapshot, McpError> {
    validate_selection(id)?;
    // Security::open would create or change files. Diagnostics must use read_at only.
    platform::check_path(root)?;
    platform::check_path(&root.join("authorization.dpapi"))?;
    let before = storage(root);
    let (memory, disk, consistent) = match security {
        Some(security) => {
            let (memory, disk, matches) = security.diagnostic_views()?;
            (Some(memory), disk, matches)
        }
        None => (None, Security::read_view_at(root), None),
    };
    let after = storage(root);
    let stable = before == after;
    Ok(Snapshot {
        diagnostic_only: true,
        configuration_verified: false,
        current_session_verified: false,
        process_id: std::process::id(),
        executable: std::env::current_exe()
            .ok()
            .map(|p| p.to_string_lossy().into_owned()),
        build: super::build_identity::current(),
        selected_client_fingerprint: id.map(|id| format!("{:x}", Sha256::digest(id.as_bytes()))),
        storage: after,
        storage_stable_during_read: stable,
        memory: memory.as_ref().map(|store| summary(store, id)),
        disk: disk.as_ref().ok().map(|store| summary(store, id)),
        memory_matches_disk: if stable { consistent } else { None },
        disk_error_code: disk.err().map(|error| error.code),
    })
}
fn compare(gui: Snapshot, importer: Option<Snapshot>, error: Option<String>) -> Comparison {
    let stable_importer = importer
        .as_ref()
        .filter(|i| i.storage_stable_during_read && gui.storage_stable_during_read);
    let same_file = stable_importer.and_then(|i| {
        gui.storage
            .file_identity
            .as_ref()
            .zip(i.storage.file_identity.as_ref())
            .map(|(a, b)| a == b)
    });
    let stores = stable_importer.and_then(|i| gui.disk.as_ref().zip(i.disk.as_ref()));
    let same_revision = stores.map(|(a, b)| a.revision == b.revision);
    let same_selected = stores.and_then(|(a, b)| {
        a.selected_client
            .as_ref()
            .zip(b.selected_client.as_ref())
            .map(|(a, b)| a == b)
    });
    let same_build = importer.as_ref().map(|i| i.build == gui.build);
    Comparison {
        diagnostic_only: true,
        configuration_verified: false,
        current_session_verified: false,
        gui,
        importer,
        importer_error_code: error,
        same_storage_file: same_file,
        same_revision,
        same_selected_client: same_selected,
        same_build,
    }
}
pub async fn collect(
    security: &Security,
    executable: &Path,
    id: Option<&str>,
) -> Result<Comparison, McpError> {
    validate_selection(id)?;
    let gui = snapshot(security.private_root(), Some(security), id)?;
    let run = async {
        super::onboarding::validate_command(executable)?;
        let mut command = tokio::process::Command::new(executable);
        command
            .arg("--diagnose-authorization")
            .env_remove("IN_LINE_MCP_TOKEN")
            .env_remove("IN_LINE_MCP_CLIENT_ID")
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .creation_flags(0x08000000);
        if let Some(id) = id {
            command.arg(id);
        }
        let output = tokio::time::timeout(std::time::Duration::from_secs(5), command.output())
            .await
            .map_err(|_| McpError::new("host_unavailable"))?
            .map_err(|_| McpError::new("host_unavailable"))?;
        if !output.status.success() || output.stdout.len() > 16 * 1024 {
            return Err(McpError::new("incompatible"));
        }
        let envelope: super::contract::Envelope =
            serde_json::from_slice(&output.stdout).map_err(|_| McpError::new("incompatible"))?;
        if envelope.api_version != super::contract::API_VERSION
            || envelope.status != "ok"
            || envelope.error.is_some()
        {
            return Err(McpError::new("incompatible"));
        }
        let importer: Snapshot =
            serde_json::from_value(envelope.data.ok_or_else(|| McpError::new("incompatible"))?)
                .map_err(|_| McpError::new("incompatible"))?;
        if importer.selected_client_fingerprint != gui.selected_client_fingerprint {
            return Err(McpError::new("incompatible"));
        }
        if !importer.diagnostic_only
            || importer.configuration_verified
            || importer.current_session_verified
        {
            return Err(McpError::new("incompatible"));
        }
        Ok(importer)
    }
    .await;
    Ok(match run {
        Ok(importer) => compare(gui, Some(importer), None),
        Err(error) => compare(gui, None, Some(error.code)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::security::{Grant, Permissions, Scope};
    fn root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "inline-auth-diagnostic-{}",
            platform::random_secret().unwrap()
        ))
    }
    #[test]
    fn redacted_snapshot_distinguishes_duplicate_names_and_never_changes_files() {
        let root = root();
        let security = Security::open_at(root.clone()).unwrap();
        let grant = || Grant {
            name: "Codex secret-name-marker".into(),
            permissions: Permissions::default(),
            scope: Scope::default(),
        };
        let first = security.grant_with(grant(), Some("codex"), Ok).unwrap();
        let second = security.grant_with(grant(), Some("cli"), Ok).unwrap();
        let before = std::fs::read(root.join("authorization.dpapi")).unwrap();
        let entries = || {
            let mut names: Vec<_> = std::fs::read_dir(&root)
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect();
            names.sort();
            names
        };
        let entries_before = entries();
        let report = snapshot(&root, Some(&security), Some(&first.client_id)).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        for forbidden in [
            &first.token,
            &second.token,
            &first.client_id,
            &second.client_id,
            "secret-name-marker",
            "tokenHash",
            "ipcSecret",
        ] {
            assert!(!json.contains(forbidden));
        }
        assert_eq!(report.disk.as_ref().unwrap().client_count, 2);
        assert_eq!(report.memory_matches_disk, Some(true));
        assert_eq!(
            report
                .disk
                .unwrap()
                .selected_client
                .unwrap()
                .kind
                .as_deref(),
            Some("codex")
        );
        assert_eq!(
            snapshot(&root, None, Some(&second.client_id))
                .unwrap()
                .disk
                .unwrap()
                .selected_client
                .unwrap()
                .kind
                .as_deref(),
            Some("cli")
        );
        assert!(
            !snapshot(&root, None, Some(&"0".repeat(64)))
                .unwrap()
                .disk
                .unwrap()
                .selected_client
                .unwrap()
                .found
        );
        assert_eq!(
            std::fs::read(root.join("authorization.dpapi")).unwrap(),
            before
        );
        assert_eq!(entries(), entries_before);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_corrupt_and_replaced_store_are_reported_without_repair() {
        let root = root();
        assert!(snapshot(&root, None, Some("bad")).is_err());
        let report = snapshot(&root, None, None).unwrap();
        assert!(!root.exists());
        assert!(report.disk.is_none());
        let security = Security::open_at(root.clone()).unwrap();
        let second = Security::open_at(root.clone()).unwrap();
        second.pause(true).unwrap();
        let report = snapshot(&root, Some(&security), None).unwrap();
        assert_eq!(report.memory_matches_disk, Some(false));
        std::fs::write(root.join("authorization.dpapi"), b"corrupt-preserve-me").unwrap();
        let report = snapshot(&root, Some(&security), None).unwrap();
        assert!(report.disk.is_none());
        assert_eq!(
            std::fs::read(root.join("authorization.dpapi")).unwrap(),
            b"corrupt-preserve-me"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn comparisons_do_not_turn_diagnostics_into_connection_verification() {
        let root_a = root();
        let root_b = root();
        let a = Security::open_at(root_a.clone()).unwrap();
        let _b = Security::open_at(root_b.clone()).unwrap();
        let report = compare(
            snapshot(&root_a, Some(&a), None).unwrap(),
            Some(snapshot(&root_b, None, None).unwrap()),
            None,
        );
        assert_eq!(report.same_storage_file, Some(false));
        assert!(!report.configuration_verified && !report.current_session_verified);
        let report = compare(
            snapshot(&root_a, Some(&a), None).unwrap(),
            None,
            Some("incompatible".into()),
        );
        assert!(report.same_revision.is_none());
        assert!(report.same_storage_file.is_none());
        let mut unstable = snapshot(&root_a, None, None).unwrap();
        unstable.storage_stable_during_read = false;
        let report = compare(
            snapshot(&root_a, Some(&a), None).unwrap(),
            Some(unstable),
            None,
        );
        assert!(report.same_revision.is_none() && report.same_storage_file.is_none());
        std::fs::remove_dir_all(root_a).unwrap();
        std::fs::remove_dir_all(root_b).unwrap();
    }
    #[tokio::test]
    async fn unavailable_importer_keeps_gui_evidence_without_writing() {
        let root = root();
        let security = Security::open_at(root.clone()).unwrap();
        let before = std::fs::read(root.join("authorization.dpapi")).unwrap();
        let report = collect(&security, &root.join("absent-in-line-mcp.exe"), None)
            .await
            .unwrap();
        assert!(report.gui.disk.is_some());
        assert!(report.importer.is_none());
        assert_eq!(
            report.importer_error_code.as_deref(),
            Some("import_invalid")
        );
        assert_eq!(
            std::fs::read(root.join("authorization.dpapi")).unwrap(),
            before
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
