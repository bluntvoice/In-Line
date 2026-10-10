use super::{
    contract::{Credentials, McpError, API_VERSION},
    platform,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Permissions {
    pub regular_read: bool,
    pub full_read: bool,
    pub write: bool,
}
impl Default for Permissions {
    fn default() -> Self {
        Self {
            regular_read: true,
            full_read: false,
            write: false,
        }
    }
}
impl Permissions {
    pub fn intersection(&self, other: &Self) -> Self {
        Self {
            regular_read: self.regular_read && other.regular_read,
            full_read: self.full_read && other.full_read,
            write: self.write && other.write,
        }
    }
}
pub use super::scope::Scope;
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Client {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
    pub token_hash: String,
    pub revoked: bool,
    pub permissions: Permissions,
    pub scope: Scope,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    pub version: u32,
    pub machine: String,
    pub sid: String,
    pub ipc_secret: String,
    pub revision: u64,
    pub paused: bool,
    pub groups: Permissions,
    pub clients: Vec<Client>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientView {
    pub id: String,
    pub name: String,
    pub kind: Option<String>,
    pub revoked: bool,
    pub permissions: Permissions,
    pub scope: Scope,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityView {
    pub paused: bool,
    pub groups: Permissions,
    pub revision: u64,
    pub clients: Vec<ClientView>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Grant {
    pub name: String,
    pub permissions: Permissions,
    pub scope: Scope,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issued {
    pub client_id: String,
    pub token: String,
}
#[derive(Clone)]
pub struct Authorization {
    pub permissions: Permissions,
    pub scope: Scope,
    pub revision: u64,
}
fn digest(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn constant_equal(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |x, (a, b)| x | (a ^ b)) == 0
}
impl Store {
    pub fn authorize(&self, credentials: &Credentials) -> Result<Authorization, McpError> {
        if credentials.client_id.len() != 64 || credentials.token.len() != 64 {
            return Err(McpError::new("unauthenticated"));
        }
        let token_hash = digest(&credentials.token);
        let client = self
            .clients
            .iter()
            .find(|x| x.id == credentials.client_id && constant_equal(&x.token_hash, &token_hash))
            .ok_or_else(|| McpError::new("unauthenticated"))?;
        if client.revoked {
            return Err(McpError::new("revoked"));
        }
        if self.paused {
            return Err(McpError::new("paused"));
        }
        Ok(Authorization {
            permissions: self.groups.intersection(&client.permissions),
            scope: client.scope.clone(),
            revision: self.revision,
        })
    }
    pub fn view(&self) -> SecurityView {
        SecurityView {
            paused: self.paused,
            groups: self.groups.clone(),
            revision: self.revision,
            clients: self
                .clients
                .iter()
                .map(|x| ClientView {
                    id: x.id.clone(),
                    name: x.name.clone(),
                    kind: x.kind.clone(),
                    revoked: x.revoked,
                    permissions: x.permissions.clone(),
                    scope: x.scope.clone(),
                })
                .collect(),
        }
    }
}
pub struct Security {
    pub(crate) store: Mutex<Store>,
    path: PathBuf,
    pub(crate) limits: Mutex<Limiter>,
    pub(crate) queries: Mutex<super::query::QueryState>,
}
impl Security {
    pub fn open() -> Result<Self, McpError> {
        Self::open_at(platform::root()?)
    }
    pub fn open_at(root: PathBuf) -> Result<Self, McpError> {
        platform::check_path(&root)?;
        std::fs::create_dir_all(&root).map_err(|_| McpError::new("security_unavailable"))?;
        platform::Descriptor::new(true)?.apply(&root)?;
        let path = root.join("authorization.dpapi");
        let store = if path.exists() {
            Self::read_at(&root)?
        } else {
            Store {
                version: API_VERSION,
                machine: platform::machine_id()?,
                sid: platform::user_sid()?,
                ipc_secret: platform::random_secret()?,
                revision: 1,
                paused: false,
                groups: Permissions::default(),
                clients: vec![],
            }
        };
        let security = Self {
            store: Mutex::new(store),
            path,
            limits: Mutex::new(Limiter::default()),
            queries: Mutex::new(super::query::QueryState::new(root.clone())),
        };
        if !security.path.exists() {
            let guard = security
                .store
                .lock()
                .map_err(|_| McpError::new("security_unavailable"))?;
            security.save(&guard)?;
        }
        Ok(security)
    }
    pub fn read_at(root: &Path) -> Result<Store, McpError> {
        let path = root.join("authorization.dpapi");
        platform::check_path(&path)?;
        platform::Descriptor::new(true)?.verify(root)?;
        platform::Descriptor::new(false)?.verify(&path)?;
        if std::fs::metadata(&path)
            .map_err(|_| McpError::new("security_unavailable"))?
            .len()
            > 1024 * 1024
        {
            return Err(McpError::new("security_unavailable"));
        }
        let bytes = std::fs::read(&path).map_err(|_| McpError::new("security_unavailable"))?;
        if bytes.len() > 1024 * 1024 {
            return Err(McpError::new("security_unavailable"));
        }
        let store: Store = serde_json::from_slice(&platform::seal(&bytes, true)?)
            .map_err(|_| McpError::new("security_unavailable"))?;
        if store.version != API_VERSION {
            return Err(McpError::new("incompatible"));
        }
        if store.machine != platform::machine_id()? || store.sid != platform::user_sid()? {
            return Err(McpError::new("security_unavailable"));
        }
        Ok(store)
    }
    fn save(&self, store: &Store) -> Result<(), McpError> {
        use std::io::Write;
        platform::check_path(&self.path)?;
        let bytes = platform::seal(
            &serde_json::to_vec(store).map_err(|_| McpError::new("security_unavailable"))?,
            false,
        )?;
        if bytes.len() > 1024 * 1024 {
            return Err(McpError::new("resource_limit"));
        }
        let temp = self
            .path
            .with_file_name(format!("pending-{}", platform::random_secret()?));
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|_| McpError::new("security_unavailable"))?;
            platform::Descriptor::new(false)?.apply(&temp)?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| McpError::new("security_unavailable"))?;
            drop(file);
            platform::atomic_replace(&temp, &self.path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result
    }
    pub fn change<T>(
        &self,
        mutate: impl FnOnce(&mut Store) -> Result<T, McpError>,
    ) -> Result<T, McpError> {
        let mut guard = self
            .store
            .lock()
            .map_err(|_| McpError::new("security_unavailable"))?;
        let mut candidate = guard.clone();
        let result = mutate(&mut candidate)?;
        candidate.revision = candidate
            .revision
            .checked_add(1)
            .ok_or_else(|| McpError::new("security_unavailable"))?;
        self.save(&candidate)?;
        *guard = candidate;
        Ok(result)
    }
    pub fn view(&self) -> Result<SecurityView, McpError> {
        Ok(self
            .store
            .lock()
            .map_err(|_| McpError::new("security_unavailable"))?
            .view())
    }
    pub fn grant(&self, grant: Grant) -> Result<Issued, McpError> {
        self.grant_with(grant, None, Ok)
    }
    pub(crate) fn grant_with<T>(
        &self,
        grant: Grant,
        kind: Option<&str>,
        prepare: impl FnOnce(Issued) -> Result<T, McpError>,
    ) -> Result<T, McpError> {
        grant.scope.validate()?;
        if grant.name.trim().is_empty()
            || grant.name.len() > 160
            || grant.name.chars().any(char::is_control)
        {
            return Err(McpError::new("invalid_arguments"));
        }
        self.change(|store| {
            if store.clients.len() >= 100 {
                return Err(McpError::new("invalid_arguments"));
            }
            let id = platform::random_secret()?;
            let token = platform::random_secret()?;
            store.clients.push(Client {
                id: id.clone(),
                name: grant.name.trim().into(),
                kind: kind.map(String::from),
                token_hash: digest(&token),
                revoked: false,
                permissions: grant.permissions,
                scope: grant.scope,
            });
            prepare(Issued {
                client_id: id,
                token,
            })
        })
    }
    pub fn rotate(&self, id: &str) -> Result<Issued, McpError> {
        self.rotate_with(id, Ok)
    }
    pub(crate) fn rotate_with<T>(
        &self,
        id: &str,
        prepare: impl FnOnce(Issued) -> Result<T, McpError>,
    ) -> Result<T, McpError> {
        self.change(|store| {
            let client = store
                .clients
                .iter_mut()
                .find(|x| x.id == id && !x.revoked)
                .ok_or_else(|| McpError::new("revoked"))?;
            let token = platform::random_secret()?;
            client.token_hash = digest(&token);
            prepare(Issued {
                client_id: id.into(),
                token,
            })
        })
    }
    pub fn revoke(&self, id: &str) -> Result<(), McpError> {
        self.change(|store| {
            let client = store
                .clients
                .iter_mut()
                .find(|x| x.id == id)
                .ok_or_else(|| McpError::new("unauthenticated"))?;
            client.revoked = true;
            Ok(())
        })
    }
    pub fn update_client(
        &self,
        id: &str,
        permissions: Permissions,
        scope: Scope,
    ) -> Result<(), McpError> {
        scope.validate()?;
        self.change(|store| {
            let client = store
                .clients
                .iter_mut()
                .find(|x| x.id == id && !x.revoked)
                .ok_or_else(|| McpError::new("revoked"))?;
            client.permissions = permissions;
            client.scope = scope;
            Ok(())
        })
    }
    pub fn pause(&self, paused: bool) -> Result<(), McpError> {
        self.change(|store| {
            store.paused = paused;
            Ok(())
        })
    }
    pub fn set_groups(&self, groups: Permissions) -> Result<(), McpError> {
        self.change(|store| {
            store.groups = groups;
            Ok(())
        })
    }
    pub fn pipe_name(&self) -> Result<String, McpError> {
        Ok(format!(
            r"\\.\pipe\in-line-mcp-{}",
            digest(self.path.to_string_lossy().as_ref())
        ))
    }
}
struct Window {
    since: Instant,
    count: u32,
    until: Option<Instant>,
    strikes: u32,
}
#[derive(Default)]
pub struct Limiter {
    entries: HashMap<String, Window>,
}
impl Limiter {
    pub fn check(&mut self, key: String, valid: bool, now: Instant) -> Result<(), McpError> {
        self.entries
            .retain(|_, w| now.saturating_duration_since(w.since) < Duration::from_secs(600));
        if !self.entries.contains_key(&key) && self.entries.len() >= 256 {
            // Invalid claimed identities must not consume the capacity of real clients.
            if valid {
                self.entries.retain(|key, _| key.starts_with("valid:"));
            } else {
                return Err(McpError::new("rate_limited"));
            }
        }
        let w = self.entries.entry(key).or_insert(Window {
            since: now,
            count: 0,
            until: None,
            strikes: 0,
        });
        let limited = |seconds| {
            let mut e = McpError::new("rate_limited");
            e.retryable = true;
            e.retry_after_seconds = Some(seconds);
            e
        };
        if let Some(until) = w.until {
            if now < until {
                return Err(limited(until.duration_since(now).as_secs().max(1)));
            }
            w.until = None;
            w.count = 0;
            w.since = now;
        }
        if now.duration_since(w.since) >= Duration::from_secs(60) {
            w.count = 0;
            w.since = now;
        }
        w.count += 1;
        if w.count > if valid { 60 } else { 5 } {
            w.strikes = (w.strikes + 1).min(5);
            let secs = 60 * u64::from(w.strikes);
            w.until = Some(now + Duration::from_secs(secs));
            return Err(limited(secs));
        }
        Ok(())
    }
    pub fn key(credentials: &Credentials, valid: bool) -> String {
        if valid {
            format!("valid:{}", credentials.client_id)
        } else {
            format!(
                "invalid:{}",
                digest(&format!("{}:{}", credentials.client_id, credentials.token))
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "inline-security-{}",
            platform::random_secret().unwrap()
        ))
    }
    fn grant() -> Grant {
        Grant {
            name: "测试客户端".into(),
            permissions: Permissions::default(),
            scope: Scope::default(),
        }
    }
    fn credentials(issued: Issued) -> Credentials {
        Credentials {
            client_id: issued.client_id,
            token: issued.token,
        }
    }
    #[test]
    fn defaults_and_persistent_long_authorization() {
        let root = root();
        let security = Security::open_at(root.clone()).unwrap();
        let view = security.view().unwrap();
        assert!(view.groups.regular_read);
        assert!(!view.groups.full_read && !view.groups.write && !view.paused);
        assert!(view.clients.is_empty());
        let creds = credentials(security.grant(grant()).unwrap());
        for _ in 0..3 {
            let auth = Security::read_at(&root).unwrap().authorize(&creds).unwrap();
            assert!(auth.permissions.regular_read);
            assert!(!auth.permissions.full_read && !auth.permissions.write);
        }
        assert!(!std::fs::read(root.join("authorization.dpapi"))
            .unwrap()
            .windows(creds.token.len())
            .any(|x| x == creds.token.as_bytes()));
        assert!(!serde_json::to_string(&security.view().unwrap())
            .unwrap()
            .contains(&creds.token));
    }
    #[test]
    fn rotate_revoke_pause_and_group_intersection() {
        let root = root();
        let security = Security::open_at(root.clone()).unwrap();
        let old = credentials(security.grant(grant()).unwrap());
        let new = credentials(security.rotate(&old.client_id).unwrap());
        assert_eq!(
            Security::read_at(&root)
                .unwrap()
                .authorize(&old)
                .err()
                .unwrap()
                .code,
            "unauthenticated"
        );
        security.pause(true).unwrap();
        assert_eq!(
            Security::open_at(root.clone())
                .unwrap()
                .store
                .lock()
                .unwrap()
                .authorize(&new)
                .err()
                .unwrap()
                .code,
            "paused"
        );
        security.pause(false).unwrap();
        security
            .set_groups(Permissions {
                regular_read: false,
                full_read: true,
                write: true,
            })
            .unwrap();
        let auth = Security::read_at(&root).unwrap().authorize(&new).unwrap();
        assert!(
            !auth.permissions.regular_read
                && !auth.permissions.full_read
                && !auth.permissions.write
        );
        security.revoke(&new.client_id).unwrap();
        assert_eq!(
            Security::read_at(&root)
                .unwrap()
                .authorize(&new)
                .err()
                .unwrap()
                .code,
            "revoked"
        );
        assert!(security.rotate(&new.client_id).is_err());
    }
    #[test]
    fn corrupt_and_copied_machine_store_never_reset_or_restore_credentials() {
        let root = root();
        let security = Security::open_at(root.clone()).unwrap();
        let creds = credentials(security.grant(grant()).unwrap());
        let mut store = Security::read_at(&root).unwrap();
        store.machine = "other-machine".into();
        std::fs::write(
            root.join("authorization.dpapi"),
            platform::seal(&serde_json::to_vec(&store).unwrap(), false).unwrap(),
        )
        .unwrap();
        assert!(Security::open_at(root.clone()).is_err());
        std::fs::write(root.join("authorization.dpapi"), b"corrupt-test").unwrap();
        assert!(Security::open_at(root.clone()).is_err());
        assert_eq!(
            std::fs::read(root.join("authorization.dpapi")).unwrap(),
            b"corrupt-test"
        );
        assert!(Security::read_at(&root).is_err());
        let _ = creds;
    }
    #[test]
    fn persistence_failure_does_not_claim_revocation_or_change_memory() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = root();
        let security = Security::open_at(root.clone()).unwrap();
        let creds = credentials(security.grant(grant()).unwrap());
        let revision = security.store.lock().unwrap().revision;
        let error = security
            .change(|store| {
                store.clients[0].scope.departments = Some(vec!["超大合成范围".repeat(200_000)]);
                Ok(())
            })
            .unwrap_err();
        assert_eq!(error.code, "resource_limit");
        assert_eq!(security.store.lock().unwrap().revision, revision);
        assert_eq!(Security::read_at(&root).unwrap().revision, revision);
        assert!(security.store.lock().unwrap().authorize(&creds).is_ok());
        let file = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(root.join("authorization.dpapi"))
            .unwrap();
        assert!(security.revoke(&creds.client_id).is_err());
        assert!(security.store.lock().unwrap().authorize(&creds).is_ok());
        drop(file);
        security.revoke(&creds.client_id).unwrap();
        assert_eq!(
            Security::read_at(&root)
                .unwrap()
                .authorize(&creds)
                .err()
                .unwrap()
                .code,
            "revoked"
        );
    }
    #[test]
    fn invalid_id_spoofing_cannot_throttle_real_client_and_limits_expire() {
        let now = Instant::now();
        let mut limiter = Limiter::default();
        for i in 0..256 {
            limiter.check(format!("invalid:{i}"), false, now).unwrap();
        }
        assert!(limiter.check("valid:real".into(), true, now).is_ok());
        for _ in 1..60 {
            limiter.check("valid:real".into(), true, now).unwrap();
        }
        let error = limiter.check("valid:real".into(), true, now).unwrap_err();
        assert_eq!(error.code, "rate_limited");
        assert_eq!(error.retry_after_seconds, Some(60));
        assert!(limiter.check("valid:other".into(), true, now).is_ok());
        assert!(limiter
            .check("valid:real".into(), true, now + Duration::from_secs(61))
            .is_ok());
    }
}
