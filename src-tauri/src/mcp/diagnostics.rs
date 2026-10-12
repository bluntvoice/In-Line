//! Private, bounded technical observations. No raw request or business log is accepted.
use super::{contract::McpError, diagnostic_types::*, durable, platform};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
const FILE_ID: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_BYTES: usize = 8 * 1024 * 1024;
pub struct Diagnostics {
    root: PathBuf,
    state: Mutex<Option<State>>,
    storage_error: AtomicBool,
    notice: Mutex<Option<std::sync::Arc<dyn Fn() + Send + Sync>>>,
}
impl Diagnostics {
    pub fn new(parent: PathBuf) -> Self {
        let root = parent.join("diagnostics");
        let state: Result<State, McpError> = (|| {
            if root.exists() {
                platform::check_path(&root)?;
                platform::Descriptor::new(true)?.verify(&root)?;
            } else {
                durable::root(&parent, "diagnostics")?;
            }
            if root.join(format!("{FILE_ID}.json")).exists() {
                let mut loaded: State = durable::read(&root, FILE_ID, MAX_BYTES)?;
                validate_state(&loaded)?;
                sanitize_state(&mut loaded);
                Ok(loaded)
            } else {
                Ok(State::default())
            }
        })();
        let bad = state.is_err();
        Self {
            root,
            state: Mutex::new(state.ok()),
            storage_error: AtomicBool::new(bad),
            notice: Mutex::new(None),
        }
    }
    pub fn set_notice(&self, f: std::sync::Arc<dyn Fn() + Send + Sync>) {
        if let Ok(mut g) = self.notice.lock() {
            *g = Some(f);
        }
    }
    fn alert(&self) {
        if let Ok(g) = self.notice.lock() {
            if let Some(f) = g.as_ref() {
                f();
            }
        }
    }
    pub fn maintenance(&self, code: &str, basis: Option<super::basis::DataVersion>) {
        let Ok(id) = platform::random_secret() else {
            return;
        };
        let _ = self.record(
            Event {
                receipt_hash: None,
                request_id: id,
                client: None,
                tool: "inspect_data".into(),
                module: "query".into(),
                code: code.into(),
                at: chrono::Utc::now().timestamp(),
                duration_ms: 0,
                job_id: None,
                round: None,
                audit_id: None,
                data_version: basis,
                build: super::build_identity::current(),
            },
            false,
        );
    }
    fn change<T>(
        &self,
        action: impl FnOnce(&mut State) -> Result<T, McpError>,
    ) -> Result<T, McpError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| McpError::new("security_unavailable"))?;
        let original = guard
            .as_ref()
            .ok_or_else(|| McpError::new("security_unavailable"))?;
        let mut next = original.clone();
        let result = action(&mut next)?;
        validate_state(&next)?;
        if let Err(error) =
            durable::write(&self.root, FILE_ID, &next, MAX_BYTES, MAX_BYTES + 65536, 4)
        {
            self.storage_error.store(true, Ordering::SeqCst);
            return Err(error);
        }
        *guard = Some(next);
        self.storage_error.store(false, Ordering::SeqCst);
        Ok(result)
    }
    /// Host-owned typed outcome only. Callers supply no free-text diagnostics.
    pub fn record(&self, mut e: Event, current_verified: bool) -> Result<(), McpError> {
        if !valid_time(e.at) {
            return Err(McpError::new("invalid_arguments"));
        }
        e.code = known_code(&e.code).into();
        if !super::contract::TOOLS.contains(&e.tool.as_str()) {
            e.tool = "unsupported".into();
        }
        e.module = module(&e.tool).into();
        e.build = safe_build(&e.build);
        validate_basis(e.data_version.as_ref())?;
        if e.receipt_hash.as_deref().is_some_and(|h| !hex(h, 64)) {
            return Err(McpError::new("invalid_arguments"));
        }
        if !hex(&e.request_id, 64)
            || e.client.as_deref().is_some_and(|c| !hex(c, 64))
            || e.job_id.as_deref().is_some_and(|id| !hex(id, 64))
        {
            return Err(McpError::new("invalid_arguments"));
        }
        let mut alert = false;
        let result = self.change(|s| {
            cleanup(s, e.at);
            if let Some(client) = &e.client {
                s.last_clients
                    .retain(|old| old.client.as_ref() != Some(client));
                if s.last_clients.len() >= 256 {
                    s.last_clients.remove(0);
                }
                s.last_clients.push(e.clone());
            }
            if let Some(level) = severity(&e.code) {
                let index = s.faults.iter().position(|f| {
                    f.client == e.client
                        && f.tool == e.tool
                        && f.code == e.code
                        && (!matches!(e.code.as_str(), "result_unknown" | "verification_failed")
                            || f.episodes
                                .last()
                                .is_some_and(|r| r.receipt_hash == e.receipt_hash))
                });
                let index = if let Some(i) = index {
                    i
                } else {
                    if s.faults.len() >= 256 {
                        return Err(McpError::new("resource_limit"));
                    }
                    s.faults.push(Fault {
                        id: platform::random_secret()?,
                        client: e.client.clone(),
                        tool: e.tool.clone(),
                        module: e.module.clone(),
                        code: e.code.clone(),
                        severity: level.into(),
                        attention: "normal".into(),
                        episodes: vec![],
                    });
                    s.faults.len() - 1
                };
                let f = &mut s.faults[index];
                if level < f.severity.as_str() {
                    f.severity = level.into();
                }
                if f.episodes.last().is_none_or(|r| r.resolved_at.is_some()) {
                    if ["P0", "P1"].contains(&f.severity.as_str()) {
                        alert = true;
                    }
                    let number = match f.episodes.last() {
                        Some(r) => r
                            .number
                            .checked_add(1)
                            .ok_or_else(|| McpError::new("resource_limit"))?,
                        None => 1,
                    };
                    f.episodes.push(Episode {
                        receipt_hash: e.receipt_hash.clone(),
                        job_id: e.job_id.clone(),
                        execution_round: e.round,
                        audit_id: e.audit_id,
                        id: platform::random_secret()?,
                        number,
                        started: e.at,
                        last_seen: e.at,
                        evidence_count: 0,
                        first_error_id: e.request_id.clone(),
                        last_error_id: e.request_id.clone(),
                        resolved_at: None,
                        state: "unresolved".into(),
                        verification_started: None,
                        verification_successes: 0,
                        muted_at: None,
                        muted_until: None,
                        archived: false,
                        build: e.build.clone(),
                        data_version: e.data_version.clone(),
                    });
                }
                let r = f.episodes.last_mut().unwrap();
                r.last_seen = e.at;
                r.last_error_id = e.request_id.clone();
                r.evidence_count = r
                    .evidence_count
                    .checked_add(1)
                    .ok_or_else(|| McpError::new("resource_limit"))?;
                r.state = "unresolved".into();
                r.verification_started = None;
                r.verification_successes = 0;
                r.archived = false;
                if ["P0", "P1"].contains(&f.severity.as_str()) {
                    r.muted_at = None;
                    r.muted_until = None;
                }
                if f.episodes
                    .iter()
                    .filter(|r| e.at >= r.started && e.at - r.started < 30 * 86400)
                    .count()
                    >= 3
                {
                    if f.attention != "raised" {
                        alert = true;
                    }
                    f.attention = "raised".into();
                }
            } else if e.code == "ok" && current_verified {
                for f in &mut s.faults {
                    if f.client != e.client || f.tool != e.tool {
                        continue;
                    }
                    let Some(r) = f.episodes.last_mut().filter(|r| r.resolved_at.is_none()) else {
                        continue;
                    };
                    if ["result_unknown", "verification_failed"].contains(&f.code.as_str())
                        && (r.receipt_hash.is_none()
                            || r.receipt_hash != e.receipt_hash
                            || e.audit_id.is_none())
                    {
                        continue;
                    }
                    if e.at < r.last_seen {
                        continue;
                    }
                    r.state = "pending".into();
                    r.verification_successes = r
                        .verification_successes
                        .checked_add(1)
                        .ok_or_else(|| McpError::new("resource_limit"))?;
                    let first = *r.verification_started.get_or_insert(e.at);
                    // Two real related successes and a 30-second stable window; intentions do not count.
                    if r.verification_successes >= 2 && e.at - first >= 30 {
                        r.state = "resolved".into();
                        r.resolved_at = Some(e.at);
                        r.muted_at = None;
                        r.muted_until = None;
                    }
                }
            }
            s.events.push(e);
            if s.events.len() > 2000 {
                s.events.drain(..s.events.len() - 2000);
            }
            Ok(())
        });
        if result.is_ok() && alert {
            self.alert();
        }
        result
    }
    pub fn own(&self, client: &str, error: &str) -> Result<Value, McpError> {
        self.own_at(client, error, chrono::Utc::now().timestamp())
    }
    pub fn own_at(&self, client: &str, error: &str, now: i64) -> Result<Value, McpError> {
        if !valid_time(now) {
            return Err(McpError::new("invalid_arguments"));
        }
        if !hex(error, 64) {
            return Err(McpError::new("not_found"));
        }
        let guard = self
            .state
            .lock()
            .map_err(|_| McpError::new("security_unavailable"))?;
        let state = guard
            .as_ref()
            .ok_or_else(|| McpError::new("security_unavailable"))?;
        let e = state
            .events
            .iter()
            .rev()
            .find(|e| {
                e.client.as_deref() == Some(client)
                    && e.request_id == error
                    && now >= e.at
                    && now - e.at < 7 * 86400
            })
            .ok_or_else(|| McpError::new("not_found"))?;
        Ok(
            json!({"errorId":error,"requestId":e.request_id,"tool":if super::contract::TOOLS.contains(&e.tool.as_str()){e.tool.as_str()}else{"unsupported"},"module":module(&e.tool),"code":known_code(&e.code),"observedAt":e.at,"jobId":e.job_id.as_deref().filter(|id|hex(id,64)),"round":e.round,"auditId":e.audit_id,"receiptReference":e.receipt_hash,"dataVersion":e.data_version,"build":safe_build(&e.build),"facts":"This request returned the recorded technical outcome; no root cause is inferred from it alone","suggestion":guidance(known_code(&e.code)),"businessBodyIncluded":false,"rawLogsAvailableToAi":false}),
        )
    }
    pub fn action(&self, id: &str, action: &str, now: i64) -> Result<(), McpError> {
        if !valid_time(now) {
            return Err(McpError::new("invalid_arguments"));
        }
        self.change(|s| {
            cleanup(s, now);
            let f = s
                .faults
                .iter_mut()
                .find(|f| f.id == id)
                .ok_or_else(|| McpError::new("not_found"))?;
            let r = f
                .episodes
                .last_mut()
                .ok_or_else(|| McpError::new("not_found"))?;
            if action == "clearResolved" {
                if r.resolved_at.is_none() {
                    return Err(McpError::new("forbidden"));
                }
                f.episodes.retain(|r| r.resolved_at.is_none());
                return Ok(());
            }
            match action {
                "mute" => {
                    if (["P0", "P1"].contains(&f.severity.as_str())
                        || severity(&f.code) == Some("P1"))
                        || r.resolved_at.is_some()
                    {
                        return Err(McpError::new("forbidden"));
                    }
                    r.muted_at = Some(now);
                    r.muted_until = Some(now + 7 * 86400);
                }
                "unmute" => {
                    r.muted_at = None;
                    r.muted_until = None;
                }
                "verify" => {
                    if r.resolved_at.is_none() {
                        r.state = "pending".into();
                        r.verification_started = None;
                        r.verification_successes = 0;
                    }
                }
                "archive" => r.archived = true,
                _ => return Err(McpError::new("invalid_arguments")),
            }
            Ok(())
        })
    }
    pub fn local(&self, filter: &str, now: i64) -> Result<Value, McpError> {
        if !valid_time(now) {
            return Err(McpError::new("invalid_arguments"));
        }
        if !["all", "unresolved", "pending", "resolved", "archived"].contains(&filter) {
            return Err(McpError::new("invalid_arguments"));
        }
        let result=self.change(|s|{cleanup(s,now);sanitize_state(s);Ok(json!({"items":s.faults.iter().filter(|f|f.episodes.last().is_some_and(|r|filter=="all" || filter=="archived" && r.archived || !r.archived && r.state==filter)).collect::<Vec<_>>(),"storageBytes":serde_json::to_vec(s).map_err(|_|McpError::new("internal_error"))?.len(),"capacityBytes":MAX_BYTES,"storageError":self.storage_error.load(Ordering::SeqCst),"latestClients":s.last_clients,"eventRetentionDays":7,"resolvedRetentionDays":30,"unresolvedAgeCleanup":false,"recurrenceThreshold":3,"recurrenceWindowDays":30}))});
        match result {
            Ok(v) => Ok(v),
            Err(e) => {
                let state = self
                    .state
                    .lock()
                    .map_err(|_| McpError::new("security_unavailable"))?;
                if let Some(s) = state.as_ref() {
                    let mut s = s.clone();
                    cleanup(&mut s, now);
                    sanitize_state(&mut s);
                    Ok(
                        json!({"items":s.faults.into_iter().filter(|f|f.episodes.last().is_some_and(|r|filter=="all" || filter=="archived" && r.archived || !r.archived && r.state==filter)).collect::<Vec<_>>(),"latestClients":s.last_clients,"storageError":true,"capacityBytes":MAX_BYTES,"storageComplete":false}),
                    )
                } else {
                    Err(e)
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Diagnostics {
        Diagnostics::new(std::env::temp_dir().join(format!(
            "inline-p7-diag-{}",
            platform::random_secret().unwrap()
        )))
    }
    fn event(code: &str, at: i64) -> Event {
        Event {
            receipt_hash: None,
            request_id: platform::random_secret().unwrap(),
            client: Some("a".repeat(64)),
            tool: "get_report_summary".into(),
            module: "untrusted-module".into(),
            code: code.into(),
            at,
            duration_ms: 10,
            job_id: None,
            round: None,
            audit_id: None,
            data_version: None,
            build: json!({"softwareVersion":"0.5.0","mcpVersion":1,"databaseSchemaVersion":14,"baselineCommit":"1".repeat(40),"exactCommit":null,"sourceDirty":true,"commitVerification":"unverified_local_source","token":"secret-canary","path":"private-path"}),
        }
    }
    #[test]
    fn unrelated_write_receipt_cannot_resolve_unknown_commit_and_verified_original_can() {
        let d = fixture();
        let mut failed = event("result_unknown", 100);
        failed.receipt_hash = Some("1".repeat(64));
        d.record(failed, false).unwrap();
        let mut other = event("ok", 110);
        other.receipt_hash = Some("2".repeat(64));
        other.audit_id = Some(7);
        d.record(other, true).unwrap();
        assert_eq!(
            d.local("all", 110).unwrap()["items"][0]["episodes"][0]["state"],
            "unresolved"
        );
        for at in [120, 150] {
            let mut verified = event("ok", at);
            verified.receipt_hash = Some("1".repeat(64));
            verified.audit_id = Some(7);
            d.record(verified, true).unwrap();
        }
        assert_eq!(
            d.local("all", 150).unwrap()["items"][0]["episodes"][0]["state"],
            "resolved"
        );
    }
    #[test]
    fn duplicates_are_one_episode_and_recovery_requires_two_actual_stable_successes() {
        let d = fixture();
        let alerts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = alerts.clone();
        d.set_notice(std::sync::Arc::new(move || {
            observed.fetch_add(1, Ordering::SeqCst);
        }));
        for at in 100..110 {
            d.record(event("resync_required", at), false).unwrap();
        }
        let all = d.local("all", 110).unwrap();
        assert_eq!(all["items"][0]["episodes"].as_array().unwrap().len(), 1);
        assert_eq!(all["items"][0]["episodes"][0]["evidenceCount"], 10);
        assert_eq!(all["items"][0]["attention"], "normal");
        let id = all["items"][0]["id"].as_str().unwrap();
        d.action(id, "verify", 110).unwrap();
        assert_eq!(
            d.local("pending", 110).unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        d.record(event("ok", 120), false).unwrap();
        assert_eq!(
            d.local("resolved", 150).unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        d.record(event("ok", 120), true).unwrap();
        d.record(event("ok", 149), true).unwrap();
        assert_eq!(
            d.local("resolved", 149).unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        d.record(event("ok", 150), true).unwrap();
        assert_eq!(
            d.local("resolved", 150).unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        for (start, end) in [(200, 230), (300, 330)] {
            d.record(event("resync_required", start), false).unwrap();
            d.record(event("ok", start), true).unwrap();
            d.record(event("ok", end), true).unwrap();
        }
        let all = d.local("all", 330).unwrap();
        assert_eq!(all["items"][0]["episodes"].as_array().unwrap().len(), 3);
        assert_eq!(all["items"][0]["attention"], "raised");
        assert_eq!(all["items"][0]["severity"], "P2");
        assert_eq!(alerts.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn ownership_redaction_retention_and_ordinary_mute_never_reveal_raw_fields() {
        let d = fixture();
        let e = event("resync_required", 100);
        let id = e.request_id.clone();
        d.record(e, false).unwrap();
        let own = d.own_at(&"a".repeat(64), &id, 101).unwrap();
        assert!(!own.to_string().contains("secret-canary"));
        assert!(!own.to_string().contains("private-path"));
        assert_eq!(
            d.own_at(&"b".repeat(64), &id, 101).unwrap_err().code,
            "not_found"
        );
        let original = std::fs::read_to_string(d.root.join(format!("{FILE_ID}.json"))).unwrap();
        assert!(!original.contains("secret-canary"));
        assert!(!original.contains("untrusted-module"));
        let fault = d.local("all", 100).unwrap()["items"][0]["id"]
            .as_str()
            .unwrap()
            .to_string();
        d.action(&fault, "mute", 100).unwrap();
        assert_eq!(
            d.local("all", 100).unwrap()["items"][0]["episodes"][0]["mutedUntil"],
            100 + 7 * 86400
        );
        assert!(
            d.local("all", 100 + 7 * 86400).unwrap()["items"][0]["episodes"][0]["mutedUntil"]
                .is_null()
        );
        assert_eq!(
            d.local("unresolved", 100 + 100 * 86400).unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        d.record(event("ok", 100 + 100 * 86400), true).unwrap();
        d.record(event("ok", 130 + 100 * 86400), true).unwrap();
        assert_eq!(
            d.local("all", 130 + 130 * 86400).unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            d.local("keyword", 100).unwrap_err().code,
            "invalid_arguments"
        );
    }
    #[test]
    fn critical_faults_cannot_mute_and_risk_increase_unmutes_existing_episode() {
        let d = fixture();
        d.record(event("result_unknown", 100), false).unwrap();
        let fault = d.local("all", 100).unwrap()["items"][0]["id"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(d.action(&fault, "mute", 100).unwrap_err().code, "forbidden");
        {
            let mut state = d.state.lock().unwrap();
            let f = &mut state.as_mut().unwrap().faults[0];
            f.severity = "P3".into();
            f.episodes[0].muted_at = Some(100);
            f.episodes[0].muted_until = Some(100 + 7 * 86400);
        }
        d.record(event("result_unknown", 101), false).unwrap();
        let all = d.local("all", 101).unwrap();
        assert_eq!(all["items"][0]["severity"], "P1");
        assert!(all["items"][0]["episodes"][0]["mutedUntil"].is_null());
        d.state.lock().unwrap().as_mut().unwrap().faults[0].severity = "P0".into();
        assert_eq!(d.action(&fault, "mute", 101).unwrap_err().code, "forbidden");
    }
    #[test]
    fn extreme_live_values_cannot_panic_or_replace_the_last_valid_snapshot() {
        let d = fixture();
        d.record(event("internal_error", 100), false).unwrap();
        let file = d.root.join(format!("{FILE_ID}.json"));
        let original = std::fs::read(&file).unwrap();
        let fault = d.state.lock().unwrap().as_ref().unwrap().faults[0]
            .id
            .clone();
        for at in [i64::MIN, i64::MAX] {
            assert_eq!(
                d.record(event("internal_error", at), false)
                    .unwrap_err()
                    .code,
                "invalid_arguments"
            );
            assert_eq!(
                d.action(&fault, "mute", at).unwrap_err().code,
                "invalid_arguments"
            );
            assert_eq!(d.local("all", at).unwrap_err().code, "invalid_arguments");
            assert!(d.own_at(&"a".repeat(64), &"1".repeat(64), at).is_err());
            assert_eq!(std::fs::read(&file).unwrap(), original);
        }
        d.state.lock().unwrap().as_mut().unwrap().faults[0].episodes[0].evidence_count = u64::MAX;
        assert_eq!(
            d.record(event("internal_error", 101), false)
                .unwrap_err()
                .code,
            "resource_limit"
        );
        assert_eq!(std::fs::read(&file).unwrap(), original);
    }
    #[test]
    fn typed_numeric_corruption_is_rejected_without_overwriting_evidence() {
        for (field, value) in [
            ("started", json!(i64::MIN)),
            ("lastSeen", json!(i64::MAX)),
            ("resolvedAt", json!(i64::MIN)),
            ("number", json!(u32::MAX)),
            ("evidenceCount", json!(u64::MAX)),
            ("verificationSuccesses", json!(u32::MAX)),
            ("mutedUntil", json!(i64::MAX)),
        ] {
            let d = fixture();
            d.record(event("internal_error", 100), false).unwrap();
            let file = d.root.join(format!("{FILE_ID}.json"));
            let mut state: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
            state["faults"][0]["episodes"][0][field] = value;
            let bytes = serde_json::to_vec(&state).unwrap();
            std::fs::write(&file, &bytes).unwrap();
            let reopened = Diagnostics::new(d.root.parent().unwrap().to_path_buf());
            assert!(
                reopened.state.lock().unwrap().is_none(),
                "accepted corrupt {field}"
            );
            assert_eq!(
                reopened.local("all", 101).unwrap_err().code,
                "security_unavailable"
            );
            assert!(reopened
                .record(event("internal_error", 101), false)
                .is_err());
            assert_eq!(
                std::fs::read(&file).unwrap(),
                bytes,
                "overwrote corrupt {field}"
            );
        }
        for at in [i64::MIN, i64::MAX] {
            let d = fixture();
            d.record(event("internal_error", 100), false).unwrap();
            let file = d.root.join(format!("{FILE_ID}.json"));
            let mut state: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
            state["events"][0]["at"] = json!(at);
            state["lastClients"][0]["at"] = json!(at);
            let bytes = serde_json::to_vec(&state).unwrap();
            std::fs::write(&file, &bytes).unwrap();
            let reopened = Diagnostics::new(d.root.parent().unwrap().to_path_buf());
            assert!(reopened.state.lock().unwrap().is_none());
            assert!(reopened.local("all", 101).is_err());
            assert_eq!(std::fs::read(&file).unwrap(), bytes);
        }
    }
    #[test]
    fn persistence_failure_is_atomic_and_corrupt_store_is_never_overwritten() {
        let d = fixture();
        std::fs::write(d.root.join("occupied.data"), vec![0u8; 9 * 1024 * 1024]).unwrap();
        assert_eq!(
            d.record(event("internal_error", 100), false)
                .unwrap_err()
                .code,
            "resource_limit"
        );
        assert!(d.state.lock().unwrap().as_ref().unwrap().events.is_empty());
        assert!(d.storage_error.load(Ordering::SeqCst));
        let root = std::env::temp_dir().join(format!(
            "inline-p7-corrupt-{}",
            platform::random_secret().unwrap()
        ));
        let original = Diagnostics::new(root.clone());
        let file = original.root.join(format!("{FILE_ID}.json"));
        std::fs::write(&file, b"corrupt").unwrap();
        platform::Descriptor::new(false)
            .unwrap()
            .apply(&file)
            .unwrap();
        let reopened = Diagnostics::new(root);
        assert!(reopened
            .record(event("internal_error", 100), false)
            .is_err());
        assert_eq!(std::fs::read(file).unwrap(), b"corrupt");
    }
}
fn validate_basis(v: Option<&super::basis::DataVersion>) -> Result<(), McpError> {
    if v.is_some_and(|v| {
        !hex(&v.database_uuid, 64) || v.index_epoch.as_deref().is_some_and(|e| !hex(e, 64))
    }) {
        return Err(McpError::new("security_unavailable"));
    }
    Ok(())
}
fn validate_state(s: &State) -> Result<(), McpError> {
    if s.events.len() > 2000 || s.last_clients.len() > 256 || s.faults.len() > 256 {
        return Err(McpError::new("security_unavailable"));
    }
    for e in s.events.iter().chain(s.last_clients.iter()) {
        if !valid_time(e.at) {
            return Err(McpError::new("security_unavailable"));
        }
        if e.receipt_hash.as_deref().is_some_and(|h| !hex(h, 64)) {
            return Err(McpError::new("security_unavailable"));
        }
        if !hex(&e.request_id, 64)
            || e.client.as_deref().is_some_and(|c| !hex(c, 64))
            || e.job_id.as_deref().is_some_and(|c| !hex(c, 64))
        {
            return Err(McpError::new("security_unavailable"));
        }
        validate_basis(e.data_version.as_ref())?;
    }
    for f in &s.faults {
        if !hex(&f.id, 64) || f.client.as_deref().is_some_and(|c| !hex(c, 64)) {
            return Err(McpError::new("security_unavailable"));
        }
        for r in &f.episodes {
            if !valid_time(r.started)
                || !valid_time(r.last_seen)
                || [
                    r.resolved_at,
                    r.verification_started,
                    r.muted_at,
                    r.muted_until,
                ]
                .into_iter()
                .flatten()
                .any(|at| !valid_time(at))
                || r.number == 0
                || r.number == u32::MAX
                || r.evidence_count == 0
                || r.evidence_count == u64::MAX
                || r.verification_successes == u32::MAX
                || (r.state == "resolved") != r.resolved_at.is_some()
            {
                return Err(McpError::new("security_unavailable"));
            }
            if r.receipt_hash.as_deref().is_some_and(|h| !hex(h, 64))
                || r.job_id.as_deref().is_some_and(|h| !hex(h, 64))
            {
                return Err(McpError::new("security_unavailable"));
            }
            if !hex(&r.id, 64) || !hex(&r.first_error_id, 64) || !hex(&r.last_error_id, 64) {
                return Err(McpError::new("security_unavailable"));
            }
            validate_basis(r.data_version.as_ref())?;
        }
    }
    Ok(())
}
fn valid_time(at: i64) -> bool {
    // Unix seconds between 1970 and the end of year 9999. Differences and mute
    // deadlines stay safely inside i64; clock rollback remains separately handled.
    (0..=253_402_300_799).contains(&at)
}
fn sanitize_state(s: &mut State) {
    for f in &mut s.faults {
        f.tool = if super::contract::TOOLS.contains(&f.tool.as_str()) {
            f.tool.clone()
        } else {
            "unsupported".into()
        };
        f.module = module(&f.tool).into();
        f.code = known_code(&f.code).into();
        if !["P0", "P1", "P2", "P3"].contains(&f.severity.as_str()) {
            f.severity = "P1".into();
        }
        if f.attention != "raised" {
            f.attention = "normal".into();
        }
        for r in &mut f.episodes {
            r.build = safe_build(&r.build);
            if !["unresolved", "pending", "resolved"].contains(&r.state.as_str()) {
                r.state = "unresolved".into();
            }
        }
    }
    for e in &mut s.last_clients {
        e.build = safe_build(&e.build);
        e.module = module(&e.tool).into();
        e.tool = if super::contract::TOOLS.contains(&e.tool.as_str()) {
            e.tool.clone()
        } else {
            "unsupported".into()
        };
        e.code = known_code(&e.code).into();
    }
}
fn cleanup(s: &mut State, now: i64) {
    s.events.retain(|e| now < e.at || now - e.at < 7 * 86400);
    for f in &mut s.faults {
        f.episodes.retain(|r| {
            r.resolved_at
                .is_none_or(|at| now < at || now - at < 30 * 86400)
        });
        for r in &mut f.episodes {
            if r.muted_until.is_some_and(|until| now >= until)
                || r.muted_at.is_some_and(|at| now < at)
                || ["P0", "P1"].contains(&f.severity.as_str())
            {
                r.muted_at = None;
                r.muted_until = None;
            }
        }
    }
    s.faults.retain(|f| !f.episodes.is_empty());
}
fn hex(value: &str, len: usize) -> bool {
    value.len() == len && value.bytes().all(|b| b.is_ascii_hexdigit())
}
pub(crate) fn safe_build(v: &Value) -> Value {
    let baseline = v["baselineCommit"].as_str().filter(|s| hex(s, 40));
    let dirty = v["sourceDirty"].as_bool();
    let exact = v["exactCommit"].as_str().filter(|s| {
        hex(s, 40)
            && Some(*s) == baseline
            && dirty == Some(false)
            && v["commitVerification"] == "clean_checkout_readback"
    });
    json!({"softwareVersion":v["softwareVersion"].as_str().filter(|s|s.len()<80 && semver::Version::parse(s).is_ok()),"mcpVersion":v["mcpVersion"].as_u64().filter(|n|*n<10000),"databaseSchemaVersion":v["databaseSchemaVersion"].as_u64().filter(|n|*n<10000),"baselineCommit":baseline,"exactCommit":exact,"sourceDirty":dirty,"commitVerification":if exact.is_some(){"clean_checkout_readback"}else{"unverified_local_source"}})
}
