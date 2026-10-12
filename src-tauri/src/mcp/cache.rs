//! Short-lived, client-owned report results. Paging leases and durable jobs are separate.
use super::{basis::DataVersion, contract::McpError, security::Authorization};
use chrono::Utc;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
const TTL: Duration = Duration::from_secs(60);
const CAPACITY: usize = 8 * 1024 * 1024;
struct Entry {
    client: String,
    basis: DataVersion,
    revision: u64,
    result: Value,
    bytes: usize,
    created: Instant,
    used: Instant,
    computed_at: i64,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::{scope::Scope, security::Permissions};
    fn basis() -> DataVersion {
        serde_json::from_value(json!({"databaseUuid":"fixture","dataGeneration":1,"commitSequence":1,"schemaVersion":14,"querySchemaVersion":1,"statisticsDefinitionVersion":1,"indexEpoch":"epoch"})).unwrap()
    }
    fn auth() -> Authorization {
        Authorization {
            revision: 1,
            permissions: Permissions::default(),
            scope: Scope::default(),
        }
    }
    #[test]
    fn quota_lru_expiry_scope_and_clock_guards_do_not_touch_other_stores() {
        let mut cache = Cache::default();
        let auth = auth();
        let b = basis();
        for n in 0..20 {
            let client = format!("client-{}", n / 4);
            cache
                .put(
                    n.to_string(),
                    &client,
                    &auth,
                    b.clone(),
                    json!({"meta":{"snapshotId":format!("snapshot-{n}")}}),
                )
                .unwrap();
        }
        assert!(cache.entries.len() <= 16);
        assert!(cache.entries.values().map(|e| e.bytes).sum::<usize>() <= CAPACITY);
        assert!(cache.evicted > 0);
        assert!(cache.get("19", "other", &auth, &b).is_none());
        cache.entries.get_mut("19").unwrap().created = Instant::now() - Duration::from_secs(61);
        assert!(cache.get("19", "client-4", &auth, &b).is_none());
        cache.entries.get_mut("18").unwrap().computed_at = Utc::now().timestamp() + 10;
        assert!(cache.get("18", "client-4", &auth, &b).is_none());
        let untouched = cache.info(Some("client-3"))["entries"].clone();
        let clear = cache.clear(Some("client-4"));
        assert_eq!(cache.info(Some("client-3"))["entries"], untouched);
        for field in [
            "pagingSnapshotsRemoved",
            "jobCheckpointsRemoved",
            "auditRemoved",
            "formalExportsRemoved",
        ] {
            assert_eq!(clear[field], 0);
        }
        let large = cache
            .put(
                "large".into(),
                "client-3",
                &auth,
                b.clone(),
                json!({"body":"x".repeat(2*1024*1024+1),"meta":{}}),
            )
            .unwrap();
        assert_eq!(large["meta"]["cache"]["stored"], false);
        assert!(!cache.entries.contains_key("large"));
        assert_eq!(cache.bypassed, 1);
        cache
            .put(
                "revision".into(),
                "client-3",
                &auth,
                b.clone(),
                json!({"meta":{}}),
            )
            .unwrap();
        let mut new = auth.clone();
        new.revision += 1;
        assert!(cache.get("revision", "client-3", &new, &b).is_none());
        let old:DataVersion=serde_json::from_value(json!({"databaseUuid":"fixture","dataGeneration":1,"commitSequence":1,"schemaVersion":14,"querySchemaVersion":1,"statisticsDefinitionVersion":1})).unwrap();
        assert!(old.index_epoch.is_none());
        assert_ne!(old, b);
        assert!(Cache::default().entries.is_empty());
    }
}
#[derive(Default)]
pub struct Cache {
    entries: HashMap<String, Entry>,
    evicted: u64,
    bypassed: u64,
}
impl Cache {
    pub fn key(client: &str, args: &Value, auth: &Authorization, basis: &DataVersion) -> String {
        format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&json!([
                    client,
                    args,
                    auth.revision,
                    auth.permissions,
                    auth.scope,
                    basis
                ]))
                .unwrap()
            )
        )
    }
    fn prune(&mut self, client: &str, auth: &Authorization, basis: &DataVersion) {
        let now = Utc::now().timestamp();
        let before = self.entries.len();
        self.entries.retain(|_, e| {
            e.created.elapsed() < TTL
                && now >= e.computed_at
                && (e.client != client || e.revision == auth.revision && &e.basis == basis)
        });
        self.evicted += (before - self.entries.len()) as u64;
    }
    pub fn get(
        &mut self,
        key: &str,
        client: &str,
        auth: &Authorization,
        basis: &DataVersion,
    ) -> Option<Value> {
        self.prune(client, auth, basis);
        let e = self.entries.get_mut(key)?;
        if e.client != client || e.revision != auth.revision || &e.basis != basis {
            return None;
        }
        e.used = Instant::now();
        let mut result = e.result.clone();
        result["meta"]["cache"] = json!({"reused":true,"source":"memory_report","sourceJobId":null,"sourceSnapshotId":result["meta"]["snapshotId"],"computedAt":e.computed_at,"validatedAt":Utc::now().timestamp(),"validationStatus":"current_basis_and_authorization","ttlSeconds":TTL.as_secs()});
        Some(result)
    }
    pub fn put(
        &mut self,
        key: String,
        client: &str,
        auth: &Authorization,
        basis: DataVersion,
        mut result: Value,
    ) -> Result<Value, McpError> {
        self.prune(client, auth, &basis);
        let now = Utc::now().timestamp();
        result["meta"]["cache"] = json!({"reused":false,"source":"fresh_calculation","sourceJobId":null,"sourceSnapshotId":result["meta"]["snapshotId"],"computedAt":now,"validatedAt":now,"validationStatus":"current_basis_and_authorization","ttlSeconds":TTL.as_secs()});
        result["meta"]["cache"]["stored"] = json!(true);
        let bytes = serde_json::to_vec(&result)
            .map_err(|_| McpError::new("internal_error"))?
            .len();
        if bytes > 7 * 1024 * 1024 {
            return Err(McpError::new("resource_limit"));
        }
        if bytes > 2 * 1024 * 1024 {
            self.bypassed += 1;
            result["meta"]["cache"]["stored"] = json!(false);
            return Ok(result);
        }
        self.entries.remove(&key);
        while !self.entries.is_empty()
            && (self.entries.len() >= 16
                || self.entries.values().filter(|e| e.client == client).count() >= 4
                || self.entries.values().map(|e| e.bytes).sum::<usize>() + bytes > CAPACITY)
        {
            let owned_full = self.entries.values().filter(|e| e.client == client).count() >= 4;
            let oldest = self
                .entries
                .iter()
                .filter(|(_, e)| !owned_full || e.client == client)
                .min_by_key(|(_, e)| e.used)
                .map(|(k, _)| k.clone());
            if let Some(oldest) = oldest {
                self.entries.remove(&oldest);
                self.evicted += 1;
            } else {
                break;
            }
        }
        result["meta"]["cache"]["stored"] = json!(true);
        self.entries.insert(
            key,
            Entry {
                client: client.into(),
                basis,
                revision: auth.revision,
                result: result.clone(),
                bytes,
                created: Instant::now(),
                used: Instant::now(),
                computed_at: now,
            },
        );
        Ok(result)
    }
    pub fn clear(&mut self, client: Option<&str>) -> Value {
        let before = self.entries.len();
        self.entries
            .retain(|_, e| client.is_some_and(|c| e.client != c));
        json!({"entriesRemoved":before-self.entries.len(),"pagingSnapshotsRemoved":0,"jobCheckpointsRemoved":0,"auditRemoved":0,"formalExportsRemoved":0})
    }
    pub fn info(&mut self, client: Option<&str>) -> Value {
        let before = self.entries.len();
        self.entries.retain(|_, e| e.created.elapsed() < TTL);
        self.evicted += (before - self.entries.len()) as u64;
        let items = self
            .entries
            .values()
            .filter(|e| client.is_none_or(|c| e.client == c))
            .collect::<Vec<_>>();
        json!({"tier":"memory","diskCacheEnabled":false,"bytes":items.iter().map(|e|e.bytes).sum::<usize>(),"entries":items.len(),"capacityBytes":CAPACITY,"ttlSeconds":TTL.as_secs(),"autoEvicted":if client.is_none(){Some(self.evicted)}else{None},"capacityBypasses":if client.is_none(){Some(self.bypassed)}else{None},"sources":items.iter().map(|e|json!({"snapshotId":e.result["meta"]["snapshotId"],"computedAt":e.computed_at,"dataVersion":e.basis,"validationStatus":"requires_current_request_validation"})).collect::<Vec<_>>(),"diskPolicy":"No report cache is persisted; existing private checkpoint files use Windows current-user ACL only, which does not isolate processes running as the same user"})
    }
}
