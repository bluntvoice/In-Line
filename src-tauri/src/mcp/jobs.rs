//! Main-host jobs, never scheduled business mutations. Each saved page is a checkpoint.
use super::{
    basis::DataVersion,
    contract::{McpError, ReportItemsArgs},
    durable,
    job_types::JobsArgs,
    platform,
    query::{self, QueryState},
    query_types::*,
    security::{Authorization, Limiter, Store},
    write_types::BusinessDataArgs,
};
use crate::database::Database;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const MAX_FILE: usize = 18 * 1024 * 1024;
const MAX_ROWS: usize = 100_000;
const RESULT_BYTES: usize = 16 * 1024 * 1024;
#[derive(Clone, Serialize, Deserialize)]
struct Round {
    number: u32,
    basis: DataVersion,
    started: i64,
    ended: Option<i64>,
    outcome: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Job {
    #[serde(default)]
    request_id: Option<String>,
    id: String,
    client: String,
    revision: u64,
    fingerprint: String,
    kind: String,
    arguments: Value,
    allow_latest: bool,
    state: String,
    created: i64,
    checkpoint: i64,
    first_paused: Option<i64>,
    finished: Option<i64>,
    rounds: Vec<Round>,
    rows: Vec<Value>,
    meta: Value,
    total: usize,
    summary: Option<Value>,
    waiters: HashMap<String, bool>,
    error: Option<String>,
    dependencies: Vec<i64>,
    publish_started: bool,
}
fn active(state: &str) -> bool {
    [
        "queued",
        "running",
        "paused",
        "recovering",
        "cancelling",
        "publishing",
    ]
    .contains(&state)
}
fn fingerprint(
    kind: &str,
    args: &Value,
    auth: &Authorization,
    basis: &DataVersion,
    latest: bool,
) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&json!([
                kind,
                args,
                auth.revision,
                auth.permissions,
                auth.scope,
                basis,
                latest
            ]))
            .unwrap()
        )
    )
}
fn view(j: &Job) -> Value {
    json!({"requestId":j.request_id,"jobId":j.id,"clientId":j.client,"kind":j.kind,"state":j.state,"createdAt":j.created,"checkpointAt":j.checkpoint,"finishedAt":j.finished,"sourceSnapshotId":j.meta["snapshotId"],"reuseValidationStatus":"requires_current_request_validation","firstPausedAt":j.first_paused,"progress":{"processed":j.rows.len(),"total":j.total},"rounds":j.rounds,"finalRound":if j.state=="completed"{j.rounds.last().map(|r|r.number)}else{None},"errorCode":j.error,"complete":j.state=="completed"})
}
fn save(root: &std::path::Path, j: &Job) -> Result<(), McpError> {
    durable::write(root, &j.id, j, MAX_FILE, 64 * 1024 * 1024, 256)
}
fn finish(j: &mut Job, state: &str, error: Option<&str>, now: i64) {
    j.state = state.into();
    j.error = error.map(String::from);
    j.finished = Some(now);
    if let Some(r) = j.rounds.last_mut() {
        r.ended = Some(now);
        r.outcome = Some(state.into());
    }
    if state != "completed" {
        j.rows.clear();
        j.summary = None;
        j.meta = Value::Null;
    }
}
fn authorization(store: &Store, j: &Job) -> Result<Authorization, McpError> {
    let client = store
        .clients
        .iter()
        .find(|c| c.id == j.client)
        .ok_or_else(|| McpError::new("revoked"))?;
    if client.revoked {
        return Err(McpError::new("revoked"));
    }
    if store.paused {
        return Err(McpError::new("paused"));
    }
    let permissions = store.groups.intersection(&client.permissions);
    if store.revision != j.revision {
        return Err(McpError::new("authorization_changed"));
    }
    if !permissions.regular_read || (j.kind == "export" && !permissions.write) {
        return Err(McpError::new("forbidden"));
    }
    Ok(Authorization {
        permissions,
        scope: client.scope.clone(),
        revision: store.revision,
    })
}
/// Clock policy is pure and testable; neither request retries nor startup renew anchors.
fn expired(
    first_pause: Option<i64>,
    checkpoint: i64,
    restarting: bool,
    now: i64,
) -> Option<&'static str> {
    if now < checkpoint || first_pause.is_some_and(|p| now < p) {
        return Some("clock_invalid");
    }
    if first_pause.is_some_and(|p| now - p >= 1800) {
        return Some("pause_expired");
    }
    if restarting && now - checkpoint >= 86400 {
        return Some("recovery_expired");
    }
    None
}
struct State {
    initialized: bool,
    jobs: HashMap<String, Job>,
}
#[derive(Clone)]
pub struct Jobs {
    root: PathBuf,
    state: Arc<Mutex<State>>,
    store: Arc<Mutex<Store>>,
    limits: Arc<Mutex<Limiter>>,
    terminal_notice: Arc<Mutex<Option<Arc<dyn Fn(&str) + Send + Sync>>>>,
    diagnostics: Arc<Mutex<Option<Arc<super::diagnostics::Diagnostics>>>>,
    changed: Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>,
}
impl Jobs {
    pub fn new(root: PathBuf, store: Arc<Mutex<Store>>, limits: Arc<Mutex<Limiter>>) -> Self {
        Self {
            terminal_notice: Arc::new(Mutex::new(None)),
            diagnostics: Arc::new(Mutex::new(None)),
            root: root.join("jobs"),
            state: Arc::new(Mutex::new(State {
                initialized: false,
                jobs: HashMap::new(),
            })),
            store,
            limits,
            changed: Arc::new(Mutex::new(None)),
        }
    }
    pub fn set_terminal_notice(&self, f: Arc<dyn Fn(&str) + Send + Sync>) {
        if let Ok(mut g) = self.terminal_notice.lock() {
            *g = Some(f);
        }
    }
    pub fn set_diagnostics(&self, d: Arc<super::diagnostics::Diagnostics>) {
        if let Ok(mut g) = self.diagnostics.lock() {
            *g = Some(d);
        }
    }
    pub fn with_no_active<T>(
        &self,
        f: impl FnOnce() -> Result<T, McpError>,
    ) -> Result<T, McpError> {
        let s = self
            .state
            .lock()
            .map_err(|_| McpError::new("internal_error"))?;
        if s.jobs.values().any(|j| active(&j.state)) {
            return Err(McpError::new("conflict"));
        }
        f()
    }
    fn record_terminal(&self, db: &Database, id: &str) {
        let j = self.state.lock().ok().and_then(|s| s.jobs.get(id).cloned());
        let Some(j) = j.filter(|j| !active(&j.state)) else {
            return;
        };
        if let Ok(g) = self.terminal_notice.lock() {
            if let Some(f) = g.as_ref() {
                f(&j.state);
            }
        }
        let d = self.diagnostics.lock().ok().and_then(|d| d.clone());
        if let Some(d) = d {
            let current = j.state == "completed"
                && self
                    .store
                    .lock()
                    .ok()
                    .is_some_and(|s| authorization(&s, &j).is_ok())
                && db
                    .mcp_basis()
                    .is_ok_and(|b| j.rounds.last().is_some_and(|r| r.basis == b));
            let e = super::diagnostic_types::Event {
                receipt_hash: None,
                request_id: j.request_id.clone().unwrap_or_else(|| j.id.clone()),
                client: Some(j.client.clone()),
                tool: "manage_jobs".into(),
                module: "jobs".into(),
                code: j.error.clone().unwrap_or_else(|| {
                    if j.state == "completed" {
                        "ok"
                    } else {
                        "cancelled"
                    }
                    .into()
                }),
                at: chrono::Utc::now().timestamp(),
                duration_ms: 0,
                job_id: Some(j.id.clone()),
                round: j.rounds.last().map(|r| r.number),
                audit_id: None,
                data_version: j.rounds.last().map(|r| r.basis.clone()),
                build: super::build_identity::current(),
            };
            let _ = d.record(e, current);
        }
    }
    fn notify(&self) {
        if let Ok(callback) = self.changed.lock() {
            if let Some(callback) = callback.as_ref() {
                callback();
            }
        }
    }
    pub fn initialize(
        &self,
        db: &Database,
        callback: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<(), McpError> {
        if let Some(callback) = callback {
            *self
                .changed
                .lock()
                .map_err(|_| McpError::new("internal_error"))? = Some(callback);
        }
        let root = durable::root(
            self.root
                .parent()
                .ok_or_else(|| McpError::new("security_unavailable"))?,
            "jobs",
        )?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| McpError::new("internal_error"))?;
        if state.initialized {
            return Ok(());
        }
        let now = Utc::now().timestamp();
        let mut resume = vec![];
        let mut stopped = vec![];
        for id in durable::ids(&root)? {
            let mut j: Job = durable::read(&root, &id, MAX_FILE)?;
            let was_active = active(&j.state);
            if was_active {
                if let Some(e) = expired(j.first_paused, j.checkpoint, true, now) {
                    finish(&mut j, "failed", Some(e), now);
                    save(&root, &j)?;
                } else if j.state == "cancelling" {
                    finish(&mut j, "cancelled", None, now);
                    save(&root, &j)?;
                } else {
                    j.state = "recovering".into();
                    save(&root, &j)?;
                    resume.push(id.clone());
                }
            }
            if was_active && !active(&j.state) {
                stopped.push(id.clone());
            }
            state.jobs.insert(id, j);
        }
        // Every persisted active job retains its original anchor, even when queued.
        state.initialized = true;
        drop(state);
        for id in stopped {
            self.record_terminal(db, &id);
        }
        for id in resume {
            self.spawn(db.clone(), id, true);
        }
        Ok(())
    }
    fn spawn(&self, db: Database, id: String, restarting: bool) {
        let this = self.clone();
        std::thread::spawn(move || {
            if let Err(e) = this.run(&db, &id, restarting) {
                if let Ok(mut s) = this.state.lock() {
                    if let Some(j) = s.jobs.get_mut(&id) {
                        finish(j, "failed", Some(&e.code), Utc::now().timestamp());
                        let _ = save(&this.root, j);
                    }
                }
                this.notify();
            }
            this.record_terminal(&db, &id);
        });
    }
    pub fn execute(
        &self,
        db: &Database,
        client: &str,
        auth: &Authorization,
        args: JobsArgs,
    ) -> Result<Value, McpError> {
        self.initialize(db, None)?;
        self.local_cleanup(Utc::now().timestamp())?;
        let now = Utc::now().timestamp();
        let mut s = self
            .state
            .lock()
            .map_err(|_| McpError::new("internal_error"))?;
        if args.action == "list" {
            return Ok(
                json!({"items":s.jobs.values().filter(|j|j.client==client && j.finished.is_none_or(|at|now-at<30*86400)).map(view).collect::<Vec<_>>()}),
            );
        }
        if args.action == "start" {
            let kind = args
                .kind
                .as_deref()
                .ok_or_else(|| McpError::new("invalid_arguments"))?;
            let mut arguments = args
                .arguments
                .ok_or_else(|| McpError::new("invalid_arguments"))?;
            validate(kind, &arguments, auth)?;
            if ["get_report_summary", "query_work_calendar"].contains(&kind)
                && arguments["timezoneOffsetMinutes"].is_null()
            {
                arguments["timezoneOffsetMinutes"] =
                    json!(chrono::Local::now().offset().local_minus_utc() / 60);
            }
            let basis = db.mcp_basis()?;
            let hash = fingerprint(
                kind,
                &arguments,
                auth,
                &basis,
                args.allow_latest_after_invalidation,
            );
            let waiter = platform::random_secret()?;
            if !args.explicit_requery
                && !(kind == "get_report_summary" && arguments["explicitRequery"] == true)
            {
                if let Some(j) = s.jobs.values_mut().find(|j| {
                    j.client == client
                        && j.fingerprint == hash
                        && (active(&j.state)
                            || j.state == "completed"
                                && j.finished.is_some_and(|at| now >= at && now - at < 600))
                        && j.waiters.len() < 256
                }) {
                    j.waiters.insert(waiter.clone(), true);
                    save(&self.root, j)?;
                    return Ok(
                        json!({"job":view(j),"waiterId":waiter,"reused":true,"reuse":{"sourceJobId":j.id,"computedAt":j.finished,"sourceSnapshotId":j.meta["snapshotId"],"validatedAt":now,"validationStatus":"current_basis_and_authorization"}}),
                    );
                }
            }
            if s.jobs.values().filter(|j| active(&j.state)).count() >= 2
                || s.jobs
                    .values()
                    .any(|j| j.client == client && active(&j.state))
            {
                return Err(McpError::new("resource_limit"));
            }
            let id = platform::random_secret()?;
            let j = Job {
                request_id: super::observation::current(),
                id: id.clone(),
                client: client.into(),
                revision: auth.revision,
                fingerprint: hash,
                kind: kind.into(),
                arguments,
                allow_latest: args.allow_latest_after_invalidation,
                state: "queued".into(),
                created: now,
                checkpoint: now,
                first_paused: None,
                finished: None,
                rounds: vec![Round {
                    number: 1,
                    basis,
                    started: now,
                    ended: None,
                    outcome: None,
                }],
                rows: vec![],
                meta: Value::Null,
                total: 0,
                summary: None,
                waiters: HashMap::from([(waiter.clone(), true)]),
                error: None,
                dependencies: vec![],
                publish_started: false,
            };
            save(&self.root, &j)?;
            let output = json!({"job":view(&j),"waiterId":waiter,"reused":false});
            s.jobs.insert(id.clone(), j);
            drop(s);
            self.spawn(db.clone(), id, false);
            self.notify();
            return Ok(output);
        }
        let id = args
            .job_id
            .as_deref()
            .ok_or_else(|| McpError::new("invalid_arguments"))?;
        let j = s
            .jobs
            .get_mut(id)
            .filter(|j| j.client == client && j.finished.is_none_or(|at| now - at < 30 * 86400))
            .ok_or_else(|| McpError::new("not_found"))?;
        match args.action.as_str() {
            "get" => Ok(view(j)),
            "cancelWaiter" => {
                let token = args
                    .waiter_id
                    .as_deref()
                    .ok_or_else(|| McpError::new("invalid_arguments"))?;
                let waiting = j
                    .waiters
                    .get_mut(token)
                    .ok_or_else(|| McpError::new("not_found"))?;
                *waiting = false;
                save(&self.root, j)?;
                Ok(json!({"waiterId":token,"waiting":false,"job":view(j)}))
            }
            "cancelJob" => {
                if !args.explicit_user_request {
                    return Err(McpError::new("explicit_intent_required"));
                }
                if active(&j.state) && j.state != "publishing" {
                    j.state = "cancelling".into();
                    save(&self.root, j)?;
                }
                Ok(view(j))
            }
            "result" => {
                if j.revision != auth.revision {
                    return Err(McpError::new("authorization_changed"));
                }
                if args
                    .waiter_id
                    .as_ref()
                    .is_some_and(|w| j.waiters.get(w) != Some(&true))
                {
                    return Err(McpError::new("waiter_cancelled"));
                }
                if j.state != "completed" {
                    return Err(McpError::new("job_not_complete"));
                }
                if j.finished
                    .is_none_or(|at| now - at >= 7 * 86400 || now < at)
                {
                    return Err(McpError::new("result_expired"));
                }
                let live = db.mcp_basis()?;
                if !db.mcp_in_scope(&j.dependencies, &auth.scope)?
                    || live.data_generation != j.rounds.last().unwrap().basis.data_generation
                    || live.database_uuid != j.rounds.last().unwrap().basis.database_uuid
                    || live.index_epoch != j.rounds.last().unwrap().basis.index_epoch
                {
                    return Err(McpError::new("snapshot_invalid"));
                }
                let offset = args.offset.unwrap_or(0);
                let limit = args.limit.unwrap_or(100);
                if !(1..=100).contains(&limit) || offset > j.rows.len() {
                    return Err(McpError::new("invalid_arguments"));
                }
                let end = (offset + limit).min(j.rows.len());
                let result = json!({"job":view(j),"items":j.rows[offset..end],"offset":offset,"hasMore":end<j.rows.len(),"nextOffset":if end<j.rows.len(){Some(end)}else{None},"meta":j.meta,"summary":j.summary,"complete":true,"reuse":{"sourceJobId":j.id,"computedAt":j.finished,"sourceSnapshotId":j.meta["snapshotId"],"validatedAt":now,"validationStatus":if live==j.rounds.last().unwrap().basis{"current_basis_and_authorization"}else{"authorized_historical_basis"}}});
                if serde_json::to_vec(&result).unwrap().len() > 7 * 1024 * 1024 {
                    return Err(McpError::new("resource_limit"));
                }
                Ok(result)
            }
            _ => Err(McpError::new("invalid_arguments")),
        }
    }
    pub fn local_list(&self) -> Result<Value, McpError> {
        self.local_cleanup(Utc::now().timestamp())?;
        let s = self
            .state
            .lock()
            .map_err(|_| McpError::new("internal_error"))?;
        Ok(json!({"items":s.jobs.values().map(view).collect::<Vec<_>>()}))
    }
    pub fn local_cancel(&self, id: &str) -> Result<(), McpError> {
        let mut s = self
            .state
            .lock()
            .map_err(|_| McpError::new("internal_error"))?;
        let j = s
            .jobs
            .get_mut(id)
            .ok_or_else(|| McpError::new("not_found"))?;
        if active(&j.state) && j.state != "publishing" {
            j.state = "cancelling".into();
            save(&self.root, j)?;
        }
        drop(s);
        self.notify();
        Ok(())
    }
    pub fn local_cleanup(&self, now: i64) -> Result<Value, McpError> {
        let mut s = self
            .state
            .lock()
            .map_err(|_| McpError::new("internal_error"))?;
        let mut removed = vec![];
        let mut results = 0;
        for j in s.jobs.values_mut() {
            if active(&j.state) {
                continue;
            }
            if let Some(at) = j.finished {
                if now - at >= 30 * 86400 {
                    durable::remove(&self.root, &j.id)?;
                    removed.push(j.id.clone());
                } else if now - at >= 7 * 86400
                    && (!j.rows.is_empty() || !j.arguments.is_null() || j.summary.is_some())
                {
                    j.rows.clear();
                    j.arguments = Value::Null;
                    j.summary = None;
                    j.meta = Value::Null;
                    save(&self.root, j)?;
                    results += 1;
                }
            }
        }
        for id in &removed {
            s.jobs.remove(id);
        }
        Ok(
            json!({"summariesRemoved":removed.len(),"temporaryResultsCleared":results,"formalExportsRemoved":0}),
        )
    }
    fn run(&self, db: &Database, id: &str, restarting: bool) -> Result<(), McpError> {
        let mut q = QueryState::new(self.root.clone());
        let mut cursor = None;
        let mut replayed = 0usize;
        let mut recovering = restarting;
        let mut begun = false;
        loop {
            let now = Utc::now().timestamp();
            // Same order as calls/revocation: authorization, then job state, then database.
            let store = self
                .store
                .lock()
                .map_err(|_| McpError::new("security_unavailable"))?;
            let mut s = self
                .state
                .lock()
                .map_err(|_| McpError::new("internal_error"))?;
            let running = s
                .jobs
                .values()
                .filter(|j| ["running", "publishing"].contains(&j.state.as_str()))
                .count();
            let j = s
                .jobs
                .get_mut(id)
                .ok_or_else(|| McpError::new("not_found"))?;
            if !active(&j.state) {
                return Ok(());
            }
            if j.state == "cancelling" {
                finish(j, "cancelled", None, now);
                save(&self.root, j)?;
                drop(s);
                drop(store);
                self.notify();
                return Ok(());
            }
            if let Some(e) = expired(j.first_paused, j.checkpoint, recovering, now) {
                finish(j, "failed", Some(e), now);
                save(&self.root, j)?;
                drop(s);
                drop(store);
                self.notify();
                return Ok(());
            }
            if !begun && running >= 2 {
                drop(s);
                drop(store);
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            let auth = match authorization(&store, j) {
                Ok(a) => a,
                Err(e) => {
                    finish(j, "failed", Some(&e.code), now);
                    save(&self.root, j)?;
                    drop(s);
                    drop(store);
                    self.notify();
                    return Ok(());
                }
            };
            if j.publish_started {
                let args: BusinessDataArgs = serde_json::from_value(j.arguments.clone())
                    .map_err(|_| McpError::new("invalid_arguments"))?;
                let receipt = db.mcp_artifact_receipt(&j.client, &auth, args)?;
                j.summary = Some(receipt);
                j.rows.clear();
                finish(j, "completed", None, now);
                save(&self.root, j)?;
                drop(s);
                drop(store);
                self.notify();
                return Ok(());
            }
            let limited = self
                .limits
                .lock()
                .map_err(|_| McpError::new("security_unavailable"))?
                .is_limited(&j.client, Instant::now());
            if limited {
                j.first_paused.get_or_insert(now);
                j.state = "paused".into();
                save(&self.root, j)?;
                drop(s);
                drop(store);
                self.notify();
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            let live = db.mcp_basis()?;
            let baseline = &j.rounds.last().unwrap().basis;
            if (recovering && &live != baseline)
                || live.database_uuid != baseline.database_uuid
                || live.data_generation != baseline.data_generation
                || live.index_epoch != baseline.index_epoch
            {
                if j.allow_latest && j.rounds.len() == 1 && j.kind != "export" {
                    let r = j.rounds.last_mut().unwrap();
                    r.ended = Some(now);
                    r.outcome = Some("baseline_invalid".into());
                    j.rounds.push(Round {
                        number: 2,
                        basis: live,
                        started: now,
                        ended: None,
                        outcome: None,
                    });
                    j.rows.clear();
                    j.dependencies.clear();
                    j.summary = None;
                    j.meta = Value::Null;
                    j.total = 0;
                    q = QueryState::new(self.root.clone());
                    cursor = None;
                    replayed = 0;
                    recovering = false;
                } else {
                    finish(j, "failed", Some("snapshot_invalid"), now);
                    save(&self.root, j)?;
                    drop(s);
                    drop(store);
                    self.notify();
                    return Ok(());
                }
            }
            j.state = "running".into();
            begun = true;
            let (client, kind, arguments) = (j.client.clone(), j.kind.clone(), j.arguments.clone());
            drop(s);
            drop(store);
            let computed = page(&mut q, db, &client, &auth, &kind, arguments, cursor.clone());
            let now = Utc::now().timestamp();
            // Read-only calculation does not hold native settings or cancellation locks.
            let store = self
                .store
                .lock()
                .map_err(|_| McpError::new("security_unavailable"))?;
            let mut s = self
                .state
                .lock()
                .map_err(|_| McpError::new("internal_error"))?;
            let j = s
                .jobs
                .get_mut(id)
                .ok_or_else(|| McpError::new("not_found"))?;
            let auth = authorization(&store, j)?;
            if j.state == "cancelling" {
                finish(j, "cancelled", None, now);
                save(&self.root, j)?;
                drop(s);
                drop(store);
                self.notify();
                return Ok(());
            }
            let page = match computed {
                Ok(page) => page,
                Err(e)
                    if ["snapshot_expired", "snapshot_invalid"].contains(&e.code.as_str())
                        && j.allow_latest
                        && j.rounds.len() == 1
                        && j.kind != "export" =>
                {
                    let r = j.rounds.last_mut().unwrap();
                    r.ended = Some(now);
                    r.outcome = Some(e.code);
                    j.rounds.push(Round {
                        number: 2,
                        basis: db.mcp_basis()?,
                        started: now,
                        ended: None,
                        outcome: None,
                    });
                    j.rows.clear();
                    j.summary = None;
                    j.meta = Value::Null;
                    j.total = 0;
                    q = QueryState::new(self.root.clone());
                    cursor = None;
                    replayed = 0;
                    recovering = false;
                    begun = false;
                    save(&self.root, j)?;
                    drop(s);
                    drop(store);
                    continue;
                }
                Err(e) => return Err(e),
            };
            let actual: DataVersion = serde_json::from_value(page["meta"]["dataVersion"].clone())
                .map_err(|_| McpError::new("internal_error"))?;
            if actual != j.rounds.last().unwrap().basis {
                if j.allow_latest && j.rounds.len() == 1 && j.kind != "export" {
                    let r = j.rounds.last_mut().unwrap();
                    r.ended = Some(now);
                    r.outcome = Some("baseline_invalid".into());
                    j.rounds.push(Round {
                        number: 2,
                        basis: actual,
                        started: now,
                        ended: None,
                        outcome: None,
                    });
                    j.rows.clear();
                    j.dependencies.clear();
                    j.summary = None;
                    j.meta = Value::Null;
                    j.total = 0;
                    q = QueryState::new(self.root.clone());
                    cursor = None;
                    replayed = 0;
                    recovering = false;
                    begun = false;
                    save(&self.root, j)?;
                    drop(s);
                    drop(store);
                    continue;
                }
                return Err(McpError::new("snapshot_invalid"));
            }
            let rows = page["items"].as_array().cloned().unwrap_or_default();
            let new_progress = !(recovering && replayed < j.rows.len());
            if recovering && replayed < j.rows.len() {
                let end = replayed + rows.len();
                if end > j.rows.len() || j.rows[replayed..end] != rows {
                    return Err(McpError::new("snapshot_invalid"));
                }
                replayed = end;
                if replayed == j.rows.len() {
                    recovering = false;
                }
            } else {
                j.rows.extend(rows);
                recovering = false;
            }
            if j.rows.len() > MAX_ROWS || serde_json::to_vec(&j.rows).unwrap().len() > RESULT_BYTES
            {
                return Err(McpError::new("resource_limit"));
            }
            j.total = page["total"].as_u64().unwrap_or(j.rows.len() as u64) as usize;
            j.meta = page["meta"].clone();
            j.meta["projection"] = page["projection"].clone();
            j.meta["redactedFields"] = page["redactedFields"].clone();
            j.dependencies = q.dependencies(
                j.meta["snapshotId"]
                    .as_str()
                    .ok_or_else(|| McpError::new("internal_error"))?,
            )?;
            if !page["summary"].is_null() {
                j.summary = Some(page["summary"].clone());
            }
            cursor = page["nextCursor"].as_str().map(String::from);
            if new_progress {
                j.checkpoint = now;
                j.first_paused = None;
            }
            save(&self.root, j)?;
            if cursor.is_none() {
                if j.kind == "export" {
                    // Durable P4 key: interrupted publishing is verified by receipt, never a new key.
                    j.state = "publishing".into();
                    j.publish_started = true;
                    save(&self.root, j)?;
                    let args: BusinessDataArgs = serde_json::from_value(j.arguments.clone())
                        .map_err(|_| McpError::new("invalid_arguments"))?;
                    j.meta["complete"] = json!(true);
                    let bytes = export_bytes(j, &args)?;
                    let receipt = db.mcp_artifact(&j.client, &auth, args, || Ok(bytes))?;
                    j.summary = Some(receipt);
                    j.rows.clear();
                }
                j.meta["complete"] = json!(true);
                finish(j, "completed", None, now);
                save(&self.root, j)?;
                drop(s);
                drop(store);
                self.notify();
                return Ok(());
            }
            drop(s);
            drop(store);
            self.notify();
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}
fn validate(kind: &str, args: &Value, auth: &Authorization) -> Result<(), McpError> {
    let invalid = || McpError::new("invalid_arguments");
    if serde_json::to_vec(args).unwrap().len() > 256 * 1024 {
        return Err(McpError::new("resource_limit"));
    }
    match kind {
        "query_tasks" => {
            let a: QueryArgs = serde_json::from_value(args.clone()).map_err(|_| invalid())?;
            if a.cursor.is_some() || a.snapshot.is_some() {
                return Err(invalid());
            }
            query::validate(&a.filters.unwrap_or_default(), auth)?;
            query::projection(a.projection.as_ref(), auth)?;
        }
        "query_task_history" => {
            let a: HistoryArgs = serde_json::from_value(args.clone()).map_err(|_| invalid())?;
            if a.cursor.is_some() || a.snapshot.is_some() {
                return Err(invalid());
            }
        }
        "query_work_calendar" => {
            let a: CalendarArgs = serde_json::from_value(args.clone()).map_err(|_| invalid())?;
            if a.cursor.is_some() || a.snapshot.is_some() {
                return Err(invalid());
            }
        }
        "get_report_summary" => {
            let a: super::contract::DateRangeArgs =
                serde_json::from_value(args.clone()).map_err(|_| invalid())?;
            if a.snapshot.is_some() {
                return Err(invalid());
            }
        }
        "export" => {
            if !auth.permissions.write {
                return Err(McpError::new("forbidden"));
            }
            let a: BusinessDataArgs =
                serde_json::from_value(args.clone()).map_err(|_| invalid())?;
            if a.action != "export" || ![Some("json"), Some("csv")].contains(&a.format.as_deref()) {
                return Err(invalid());
            }
            query::validate_export(auth, &a)?;
            Database::mcp_validate_artifact_intent(auth, &a)?;
        }
        _ => return Err(McpError::new("unsupported")),
    }
    Ok(())
}
fn page(
    q: &mut QueryState,
    db: &Database,
    client: &str,
    auth: &Authorization,
    kind: &str,
    mut args: Value,
    cursor: Option<String>,
) -> Result<Value, McpError> {
    match kind {
        "query_tasks" => {
            args["limit"] = json!(100);
            args["cursor"] = json!(cursor);
            query::tasks(
                q,
                db,
                client,
                auth,
                serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?,
            )
        }
        "query_task_history" => {
            args["limit"] = json!(100);
            args["cursor"] = json!(cursor);
            query::history(
                q,
                db,
                client,
                auth,
                serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?,
            )
        }
        "query_work_calendar" => {
            args["limit"] = json!(100);
            args["cursor"] = json!(cursor);
            query::calendar(
                q,
                db,
                client,
                auth,
                serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?,
            )
        }
        "get_report_summary" => {
            let a: super::contract::DateRangeArgs =
                serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?;
            let result = query::report(
                q,
                db,
                client,
                auth,
                ReportItemsArgs {
                    start_date: a.start_date,
                    end_date: a.end_date,
                    limit: Some(100),
                    offset: None,
                    filters: a.filters,
                    snapshot: None,
                    cursor,
                    timezone_offset_minutes: a.timezone_offset_minutes,
                },
                true,
            )?;
            Ok(
                json!({"summary":result,"meta":result["meta"],"items":[],"total":0,"nextCursor":null}),
            )
        }
        "export" => {
            let a: BusinessDataArgs =
                serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?;
            query::tasks(
                q,
                db,
                client,
                auth,
                QueryArgs {
                    filters: a.filters,
                    projection: a.projection,
                    limit: Some(100),
                    cursor,
                    snapshot: None,
                },
            )
        }
        _ => Err(McpError::new("unsupported")),
    }
}
fn export_bytes(j: &Job, args: &BusinessDataArgs) -> Result<Vec<u8>, McpError> {
    if j.rows.len() > 10000 {
        return Err(McpError::new("resource_limit"));
    }
    if args.format.as_deref() == Some("json") {
        return serde_json::to_vec_pretty(&json!({"items":j.rows,"projection":j.meta["projection"],"redactedFields":j.meta["redactedFields"],"meta":j.meta}))
            .map_err(|_| McpError::new("internal_error"));
    }
    let keys = args.projection.clone().unwrap_or_else(|| {
        j.rows
            .first()
            .and_then(|r| r.as_object())
            .map(|o| o.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default()
    });
    let mut output = String::from("\u{feff}");
    output += &keys
        .iter()
        .map(|v| super::business_data::csv(v))
        .collect::<Vec<_>>()
        .join(",");
    output.push('\n');
    for row in &j.rows {
        output += &keys
            .iter()
            .map(|k| {
                super::business_data::csv(
                    &row[k]
                        .as_str()
                        .map(String::from)
                        .unwrap_or_else(|| row[k].to_string()),
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        output.push('\n');
    }
    Ok(output.into_bytes())
}
#[cfg(test)]
#[path = "jobs_tests.rs"]
mod tests;
