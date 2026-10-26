//! Shared storage outside AppData; only an explicit local GUI action may migrate.
use super::{contract::McpError, onboarding, platform, security::Security};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

const TOP_DIRECTORIES: &[&str] = &[
    "diagnostics",
    "jobs",
    "changes",
    "onboarding",
    "cli-onboarding",
    "cli-profiles",
    "runtime-credentials",
];
const TOP_FILES: &[&str] = &["authorization.dpapi", "saved-queries.json"];
const MAX_FILE: u64 = 32 * 1024 * 1024;
const MAX_TOTAL: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 8192;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageState {
    pub root: String,
    pub policy: &'static str,
    pub ready: bool,
    pub migration_required: bool,
    pub can_migrate: bool,
    pub legacy_root: Option<String>,
    pub legacy_physical_file: Option<String>,
    pub legacy_file_identity: Option<String>,
    pub legacy_redirected: bool,
    pub error_code: Option<String>,
}
pub fn state() -> Result<StorageState, McpError> {
    state_at(&platform::root()?, platform::legacy_root()?.as_deref())
}
fn state_at(root: &Path, legacy: Option<&Path>) -> Result<StorageState, McpError> {
    platform::check_path(root)?;
    let auth = root.join("authorization.dpapi");
    let location = legacy
        .filter(|p| p.join("authorization.dpapi").exists())
        .and_then(|p| platform::file_location(&p.join("authorization.dpapi")).ok());
    let redirected = location
        .as_ref()
        .zip(legacy)
        .is_some_and(|((p, _), legacy)| {
            !platform::same_location_name(&legacy.join("authorization.dpapi"), p)
        });
    let ready = auth.is_file();
    let needed = !ready && legacy.is_some_and(Path::exists);
    let error = if ready {
        Security::read_view_at(root).err().map(|e| e.code)
    } else if root.exists() {
        Some("storage_migration_conflict".into())
    } else if redirected {
        Some("storage_redirected".into())
    } else if needed && location.is_none() {
        Some("security_unavailable".into())
    } else {
        None
    };
    Ok(StorageState {
        root: root.to_string_lossy().into_owned(),
        policy: "profile_private_v1",
        ready: ready && error.is_none(),
        migration_required: needed,
        can_migrate: needed && error.is_none(),
        legacy_root: legacy.map(|p| p.to_string_lossy().into_owned()),
        legacy_physical_file: location
            .as_ref()
            .map(|(p, _)| p.to_string_lossy().into_owned()),
        legacy_file_identity: location.map(|(_, id)| id),
        legacy_redirected: redirected,
        error_code: error,
    })
}
pub(crate) fn require_host_ready(root: &Path, legacy: Option<&Path>) -> Result<(), McpError> {
    let s = state_at(root, legacy)?;
    if let Some(code) = s.error_code {
        return Err(McpError::new(&code));
    }
    if s.migration_required {
        return Err(McpError::new("storage_migration_required"));
    }
    Ok(())
}
/// Importers and profiles never create or migrate a missing shared store.
pub(crate) fn require_client_ready(root: &Path) -> Result<(), McpError> {
    if root == platform::root()? {
        if root.join("authorization.dpapi").is_file() {
            platform::require_unredirected(&root.join("authorization.dpapi"))?;
        } else {
            require_host_ready(root, platform::legacy_root()?.as_deref())?;
        }
    }
    Ok(())
}
pub fn require_import_ready(root: &Path) -> Result<(), McpError> {
    require_client_ready(root)
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationReceipt {
    pub policy: &'static str,
    pub source_file_identity: String,
    pub authorization_revision: u64,
    pub clients: usize,
    pub copied_files: usize,
    pub copied_bytes: u64,
    pub source_preserved: bool,
}
#[derive(PartialEq)]
struct Entry {
    relative: PathBuf,
    directory: bool,
    bytes: u64,
    modified: Option<std::time::SystemTime>,
    identity: Option<String>,
}
fn inventory(source: &Path) -> Result<Vec<Entry>, McpError> {
    fn visit(
        source: &Path,
        dir: &Path,
        depth: usize,
        entries: &mut Vec<Entry>,
        total: &mut u64,
    ) -> Result<(), McpError> {
        if depth > 6 {
            return Err(McpError::new("resource_limit"));
        }
        let mut children = std::fs::read_dir(dir)
            .map_err(|_| McpError::new("security_unavailable"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| McpError::new("security_unavailable"))?;
        children.sort_by_key(|e| e.file_name());
        for child in children {
            if entries.len() >= MAX_ENTRIES {
                return Err(McpError::new("resource_limit"));
            }
            let path = child.path();
            platform::check_path(&path)?;
            let meta =
                std::fs::metadata(&path).map_err(|_| McpError::new("security_unavailable"))?;
            let directory = meta.is_dir();
            if !directory && !meta.is_file() {
                return Err(McpError::new("storage_migration_conflict"));
            }
            if depth == 0 {
                let name = child.file_name();
                let name = name
                    .to_str()
                    .ok_or_else(|| McpError::new("storage_migration_conflict"))?;
                if !(if directory {
                    TOP_DIRECTORIES.contains(&name)
                } else {
                    TOP_FILES.contains(&name)
                }) {
                    return Err(McpError::new("storage_migration_conflict"));
                }
            }
            platform::Descriptor::new(directory)?.verify(&path)?;
            if !directory {
                platform::require_unredirected(&path)?;
                *total = total
                    .checked_add(meta.len())
                    .ok_or_else(|| McpError::new("resource_limit"))?;
                if meta.len() > MAX_FILE || *total > MAX_TOTAL {
                    return Err(McpError::new("resource_limit"));
                }
            }
            entries.push(Entry {
                relative: path
                    .strip_prefix(source)
                    .map_err(|_| McpError::new("security_unavailable"))?
                    .into(),
                directory,
                bytes: if directory { 0 } else { meta.len() },
                modified: meta.modified().ok(),
                identity: if directory {
                    None
                } else {
                    Some(platform::file_location(&path)?.1)
                },
            });
            if directory {
                visit(source, &path, depth + 1, entries, total)?;
            }
        }
        Ok(())
    }
    let mut entries = vec![];
    visit(source, source, 0, &mut entries, &mut 0)?;
    Ok(entries)
}
fn hash(path: &Path) -> Result<Vec<u8>, McpError> {
    let mut file = std::fs::File::open(path).map_err(|_| McpError::new("security_unavailable"))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| McpError::new("security_unavailable"))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest.finalize().to_vec())
}
pub fn migrate(
    expected_file_identity: &str,
    confirmed: bool,
) -> Result<MigrationReceipt, McpError> {
    let legacy =
        platform::legacy_root()?.ok_or_else(|| McpError::new("storage_migration_conflict"))?;
    migrate_at(
        &legacy,
        &platform::root()?,
        expected_file_identity,
        confirmed,
    )
}
pub(crate) fn migrate_at(
    source: &Path,
    target: &Path,
    expected: &str,
    confirmed: bool,
) -> Result<MigrationReceipt, McpError> {
    use std::os::windows::fs::OpenOptionsExt;
    if !confirmed {
        return Err(McpError::new("explicit_intent_required"));
    }
    platform::check_path(source)?;
    platform::check_path(target)?;
    if target.exists()
        || !source.is_absolute()
        || !target.is_absolute()
        || target.starts_with(source)
        || source.starts_with(target)
    {
        return Err(McpError::new("storage_migration_conflict"));
    }
    let auth = source.join("authorization.dpapi");
    platform::require_unredirected(&auth)?;
    if platform::file_location(&auth)?.1 != expected {
        return Err(McpError::new("storage_migration_conflict"));
    }
    // Keep the source authorization immutable while copying and committing.
    let _source_lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&auth)
        .map_err(|_| McpError::new("storage_migration_conflict"))?;
    if platform::file_location(&auth)?.1 != expected {
        return Err(McpError::new("storage_migration_conflict"));
    }
    let source_store = Security::read_at(source)?;
    let before = inventory(source)?;
    let parent = target
        .parent()
        .ok_or_else(|| McpError::new("security_unavailable"))?;
    platform::check_path(parent)?;
    if !parent.exists() {
        std::fs::create_dir_all(parent).map_err(|_| McpError::new("security_unavailable"))?;
        platform::Descriptor::new(true)?.apply(parent)?;
    }
    let lock_path = parent.join("mcp-storage-migration.lock");
    platform::check_path(&lock_path)?;
    if !lock_path.exists() {
        onboarding::write_private(&lock_path, b"")?;
    }
    platform::Descriptor::new(false)?.verify(&lock_path)?;
    let _lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(&lock_path)
        .map_err(|_| McpError::new("storage_migration_conflict"))?;
    if target.exists() {
        return Err(McpError::new("storage_migration_conflict"));
    }
    let staging = parent.join(format!(
        "mcp-security-pending-{}",
        platform::random_secret()?
    ));
    onboarding::private_directory(&staging)?;
    // On failure keep the checked private staging directory for review; never touch source files.
    for entry in &before {
        let from = source.join(&entry.relative);
        let to = staging.join(&entry.relative);
        if entry.directory {
            onboarding::private_directory(&to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|_| McpError::new("security_unavailable"))?;
            platform::Descriptor::new(false)?.apply(&to)?;
            std::fs::OpenOptions::new()
                .write(true)
                .open(&to)
                .and_then(|f| f.sync_all())
                .map_err(|_| McpError::new("security_unavailable"))?;
            if hash(&from)? != hash(&to)? {
                return Err(McpError::new("storage_migration_conflict"));
            }
        }
    }
    let staged = Security::read_at(&staging)?;
    if serde_json::to_vec(&source_store).map_err(|_| McpError::new("security_unavailable"))?
        != serde_json::to_vec(&staged).map_err(|_| McpError::new("security_unavailable"))?
        || before != inventory(source)?
    {
        return Err(McpError::new("storage_migration_conflict"));
    }
    let receipt = MigrationReceipt {
        policy: "profile_private_v1",
        source_file_identity: expected.into(),
        authorization_revision: source_store.revision,
        clients: source_store.clients.len(),
        copied_files: before.iter().filter(|e| !e.directory).count(),
        copied_bytes: before.iter().map(|e| e.bytes).sum(),
        source_preserved: true,
    };
    onboarding::write_private(
        &staging.join("migration.json"),
        &serde_json::to_vec(&receipt).map_err(|_| McpError::new("security_unavailable"))?,
    )?;
    // Windows rename does not overwrite an existing target directory.
    std::fs::rename(&staging, target).map_err(|_| McpError::new("storage_migration_conflict"))?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::security::{Grant, Permissions, Scope};
    fn fixture() -> PathBuf {
        std::env::temp_dir().join(format!(
            "inline-shared-storage-{}",
            platform::random_secret().unwrap()
        ))
    }
    fn grant(s: &Security, kind: &str) -> super::super::security::Issued {
        s.grant_with(
            Grant {
                name: "synthetic".into(),
                permissions: Permissions {
                    regular_read: true,
                    full_read: true,
                    write: true,
                },
                scope: Scope {
                    departments: Some(vec!["synthetic A".into()]),
                    task_types: Some(vec!["T".into()]),
                },
            },
            Some(kind),
            Ok,
        )
        .unwrap()
    }
    #[test]
    fn shared_root_is_independent_of_appdata_overlay_and_metadata_detects_shadow_path() {
        let base = fixture();
        let profile = base.join("profile");
        let common = platform::shared_root_at(&profile).unwrap();
        assert!(!common.starts_with(profile.join("AppData")));
        assert!(platform::same_location_name(
            Path::new(r"C:\Users\KB\file"),
            Path::new(r"\\?\c:\users\kb\FILE")
        ));
        assert!(platform::same_location_name(
            Path::new(r"\\server\users\file"),
            Path::new(r"\\?\UNC\SERVER\users\FILE")
        ));
        assert!(!platform::same_location_name(
            Path::new(
                r"C:\synthetic-profile\AppData\Local\in-line-mcp-security\authorization.dpapi"
            ),
            Path::new(
                r"\\?\C:\synthetic-profile\AppData\Local\Packages\OpenAI.Codex_test\LocalCache\Local\in-line-mcp-security\authorization.dpapi"
            )
        ));
        let source = profile.join("AppData/native");
        let overlay = profile.join("AppData/Codex/LocalCache");
        let native = Security::open_at(source.clone()).unwrap();
        let shadow = Security::open_at(overlay.clone()).unwrap();
        grant(&native, "codex");
        assert_eq!(Security::read_at(&overlay).unwrap().clients.len(), 0);
        let state = state_at(&common, Some(&source)).unwrap();
        assert!(state.migration_required && state.can_migrate);
        assert_eq!(
            require_host_ready(&common, Some(&source)).unwrap_err().code,
            "storage_migration_required"
        );
        assert!(!common.exists());
        migrate_at(&source, &common, &state.legacy_file_identity.unwrap(), true).unwrap();
        assert_eq!(Security::read_at(&common).unwrap().clients.len(), 1);
        assert_eq!(Security::read_at(&overlay).unwrap().clients.len(), 0);
        assert!(state_at(&common, Some(&overlay)).unwrap().ready);
        drop((native, shadow));
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn explicit_migration_preserves_permissions_revocation_pause_and_all_known_persistent_state() {
        let base = fixture();
        let source = base.join("legacy");
        let target = platform::shared_root_at(&base.join("profile")).unwrap();
        let s = Security::open_at(source.clone()).unwrap();
        s.acknowledge_sensitive_notice().unwrap();
        let active = grant(&s, "codex");
        let revoked = grant(&s, "cli");
        s.revoke(&revoked.client_id).unwrap();
        s.pause(true).unwrap();
        s.set_groups(Permissions {
            regular_read: true,
            full_read: false,
            write: false,
        })
        .unwrap();
        for dir in TOP_DIRECTORIES {
            let dir = source.join(dir);
            onboarding::private_directory(&dir).unwrap();
            onboarding::write_private(&dir.join("synthetic.dat"), b"preserve-derived-state")
                .unwrap();
        }
        onboarding::write_private(&source.join("saved-queries.json"), b"synthetic-query-state")
            .unwrap();
        let original = std::fs::read(source.join("authorization.dpapi")).unwrap();
        let id = platform::file_location(&source.join("authorization.dpapi"))
            .unwrap()
            .1;
        assert_eq!(
            migrate_at(&source, &target, &id, false).unwrap_err().code,
            "explicit_intent_required"
        );
        assert!(!target.exists());
        let r = migrate_at(&source, &target, &id, true).unwrap();
        assert_eq!(r.clients, 2);
        assert!(r.source_preserved);
        assert_eq!(
            std::fs::read(target.join("authorization.dpapi")).unwrap(),
            original
        );
        assert_eq!(
            std::fs::read(source.join("authorization.dpapi")).unwrap(),
            original
        );
        let migrated = Security::read_at(&target).unwrap();
        let before = Security::read_at(&source).unwrap();
        assert!(serde_json::to_vec(&migrated).unwrap() == serde_json::to_vec(&before).unwrap());
        assert!(migrated.paused && migrated.sensitive_notice_acknowledged);
        assert!(
            migrated
                .clients
                .iter()
                .find(|c| c.id == revoked.client_id)
                .unwrap()
                .revoked
        );
        assert_eq!(
            migrated
                .clients
                .iter()
                .find(|c| c.id == active.client_id)
                .unwrap()
                .scope
                .departments,
            Some(vec!["synthetic A".into()])
        );
        for dir in TOP_DIRECTORIES {
            assert_eq!(
                std::fs::read(target.join(dir).join("synthetic.dat")).unwrap(),
                b"preserve-derived-state"
            );
        }
        assert!(platform::require_unredirected(&target.join("authorization.dpapi")).is_ok());
        assert_eq!(
            migrate_at(&source, &target, &id, true).unwrap_err().code,
            "storage_migration_conflict"
        );
        drop(s);
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn existing_target_changed_source_corruption_and_unknown_files_never_merge_or_overwrite() {
        let base = fixture();
        let source = base.join("legacy");
        let target = base.join("target");
        let s = Security::open_at(source.clone()).unwrap();
        let old_id = platform::file_location(&source.join("authorization.dpapi"))
            .unwrap()
            .1;
        s.pause(true).unwrap();
        assert_eq!(
            migrate_at(&source, &target, &old_id, true)
                .unwrap_err()
                .code,
            "storage_migration_conflict"
        );
        assert!(!target.exists());
        let id = platform::file_location(&source.join("authorization.dpapi"))
            .unwrap()
            .1;
        onboarding::write_private(&source.join("unknown.txt"), b"preserve unknown").unwrap();
        assert_eq!(
            migrate_at(&source, &target, &id, true).unwrap_err().code,
            "storage_migration_conflict"
        );
        assert!(!target.exists());
        std::fs::remove_file(source.join("unknown.txt")).unwrap();
        let t = Security::open_at(target.clone()).unwrap();
        let target_bytes = std::fs::read(target.join("authorization.dpapi")).unwrap();
        assert_eq!(
            migrate_at(&source, &target, &id, true).unwrap_err().code,
            "storage_migration_conflict"
        );
        assert_eq!(
            std::fs::read(target.join("authorization.dpapi")).unwrap(),
            target_bytes
        );
        let corrupt_target = base.join("corrupt-target");
        std::fs::write(source.join("authorization.dpapi"), b"corrupt-keep").unwrap();
        assert!(migrate_at(&source, &corrupt_target, &id, true).is_err());
        assert!(!corrupt_target.exists());
        assert_eq!(
            std::fs::read(source.join("authorization.dpapi")).unwrap(),
            b"corrupt-keep"
        );
        drop((s, t));
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn incomplete_target_is_not_treated_as_fresh_authorization() {
        let base = fixture();
        let source = base.join("legacy");
        let target = base.join("target");
        let s = Security::open_at(source.clone()).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        assert_eq!(
            require_host_ready(&target, Some(&source)).unwrap_err().code,
            "storage_migration_conflict"
        );
        assert!(!target.join("authorization.dpapi").exists());
        drop(s);
        std::fs::remove_dir_all(base).unwrap();
    }
}
