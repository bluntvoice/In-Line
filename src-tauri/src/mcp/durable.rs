//! Bounded private derived state. No paths supplied by clients, no business backup.
use super::{contract::McpError, platform};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
pub(super) fn root(parent: &Path, name: &str) -> Result<PathBuf, McpError> {
    let dir = parent.join(name);
    platform::check_path(&dir)?;
    std::fs::create_dir_all(&dir).map_err(|_| McpError::new("security_unavailable"))?;
    platform::Descriptor::new(true)?.apply(&dir)?;
    Ok(dir)
}
fn path(root: &Path, id: &str) -> Result<PathBuf, McpError> {
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(McpError::new("not_found"));
    }
    let p = root.join(format!("{id}.json"));
    platform::check_path(&p)?;
    Ok(p)
}
pub(super) fn read<T: DeserializeOwned>(root: &Path, id: &str, max: usize) -> Result<T, McpError> {
    let p = path(root, id)?;
    platform::Descriptor::new(false)?.verify(&p)?;
    if std::fs::metadata(&p)
        .map_err(|_| McpError::new("not_found"))?
        .len()
        > max as u64
    {
        return Err(McpError::new("resource_limit"));
    }
    let data = std::fs::read(p).map_err(|_| McpError::new("not_found"))?;
    if data.len() > max {
        return Err(McpError::new("resource_limit"));
    }
    serde_json::from_slice(&data).map_err(|_| McpError::new("snapshot_invalid"))
}
pub(super) fn write<T: Serialize>(
    root: &Path,
    id: &str,
    value: &T,
    max: usize,
    quota: usize,
    count: usize,
) -> Result<(), McpError> {
    let p = path(root, id)?;
    let bytes = serde_json::to_vec(value).map_err(|_| McpError::new("internal_error"))?;
    if bytes.len() > max {
        return Err(McpError::new("resource_limit"));
    }
    let mut total = 0usize;
    let mut files = 0usize;
    for item in std::fs::read_dir(root).map_err(|_| McpError::new("security_unavailable"))? {
        let item = item.map_err(|_| McpError::new("security_unavailable"))?;
        platform::check_path(&item.path())?;
        if item.path() != p {
            total = total.saturating_add(
                item.metadata()
                    .map_err(|_| McpError::new("security_unavailable"))?
                    .len() as usize,
            );
            files += 1;
        }
    }
    if total.saturating_add(bytes.len()) > quota || files >= count {
        return Err(McpError::new("resource_limit"));
    }
    let tmp = root.join(format!("pending-{}", platform::random_secret()?));
    let result = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|_| McpError::new("security_unavailable"))?;
        platform::Descriptor::new(false)?.apply(&tmp)?;
        f.write_all(&bytes)
            .and_then(|_| f.sync_all())
            .map_err(|_| McpError::new("security_unavailable"))?;
        drop(f);
        platform::atomic_replace(&tmp, &p)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(tmp);
    }
    result
}
pub(super) fn ids(root: &Path) -> Result<Vec<String>, McpError> {
    let mut ids = vec![];
    for item in std::fs::read_dir(root).map_err(|_| McpError::new("security_unavailable"))? {
        let item = item.map_err(|_| McpError::new("security_unavailable"))?;
        platform::check_path(&item.path())?;
        if let Some(id) = item.path().file_stem().and_then(|v| v.to_str()) {
            if id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()) {
                ids.push(id.to_string());
            }
        }
        if ids.len() > 256 {
            return Err(McpError::new("resource_limit"));
        }
    }
    Ok(ids)
}
pub(super) fn remove(root: &Path, id: &str) -> Result<(), McpError> {
    let p = path(root, id)?;
    platform::Descriptor::new(false)?.verify(&p)?;
    std::fs::remove_file(p).map_err(|_| McpError::new("security_unavailable"))
}
