//! P3-C additions to S only. No product hooks, extra positive calls or local writes.
use super::*;
use in_line_lib::mcp::contract::Credentials;
use rusqlite::{Connection, OpenFlags};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

pub(super) fn enabled() -> bool {
    if std::env::var_os("GITHUB_ACTIONS").is_none() {
        writeln!(std::io::stdout().lock(), "P3C_NOT_RUN {{\"fixture\":\"S-additions\",\"businessExecuted\":false,\"reason\":\"non_github_environment\"}}").unwrap();
        return false;
    }
    assert_eq!(
        std::env::var("GITHUB_ACTIONS").as_deref(),
        Ok("true"),
        "P3-C requires GITHUB_ACTIONS=true"
    );
    assert_eq!(
        std::env::var("RUNNER_ENVIRONMENT").as_deref(),
        Ok("github-hosted"),
        "P3-C requires RUNNER_ENVIRONMENT=github-hosted"
    );
    assert_eq!(
        std::env::var("RUNNER_OS").as_deref(),
        Ok("Windows"),
        "P3-C requires RUNNER_OS=Windows"
    );
    true
}
pub(super) fn record(case: &str, expected: Value, actual: Value) {
    let passed = expected == actual;
    writeln!(
        std::io::stdout().lock(),
        "P3C_EVIDENCE {}",
        json!({"case":case,"expected":expected,"actual":actual,"passed":passed})
    )
    .unwrap();
    assert!(passed, "P3-C mismatch: {case}");
}
fn read(path: &Path) -> Connection {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}
// All raw rows stay in memory. Comparisons emit only predicates/counts.
fn tables(path: &Path) -> Value {
    let c = read(path);
    let mut q = c
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap();
    let names = q
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let mut out = serde_json::Map::new();
    for name in names {
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        let mut s = c
            .prepare(&format!("SELECT * FROM {name} ORDER BY rowid"))
            .unwrap();
        let columns = s
            .column_names()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let rows = s
            .query_map([], |r| {
                let mut row = serde_json::Map::new();
                for (i, key) in columns.iter().enumerate() {
                    use rusqlite::types::ValueRef;
                    let v = match r.get_ref(i)? {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(n) => json!(n),
                        ValueRef::Real(n) => json!(n),
                        ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                        ValueRef::Blob(_) => panic!("unexpected synthetic blob"),
                    };
                    row.insert(key.clone(), v);
                }
                Ok(Value::Object(row))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        out.insert(name, json!(rows));
    }
    Value::Object(out)
}
fn rows<'a>(state: &'a Value, table: &str) -> &'a Vec<Value> {
    state[table].as_array().unwrap()
}
fn by<'a>(state: &'a Value, table: &str, key: &str, value: &Value) -> &'a Value {
    rows(state, table)
        .iter()
        .find(|r| &r[key] == value)
        .expect("synthetic row missing")
}
fn created(child: &tokio::process::Child) -> u64 {
    use windows_sys::Win32::{Foundation::FILETIME, System::Threading::GetProcessTimes};
    let mut c: FILETIME = unsafe { std::mem::zeroed() };
    let mut e = c;
    let mut k = c;
    let mut u = c;
    assert_ne!(
        unsafe { GetProcessTimes(child.raw_handle().unwrap(), &mut c, &mut e, &mut k, &mut u) },
        0
    );
    ((c.dwHighDateTime as u64) << 32) | c.dwLowDateTime as u64
}
pub(super) struct Probe {
    root: PathBuf,
    db: Arc<Database>,
    owner: Credentials,
    other: Credentials,
    pid: u32,
    creation: u64,
}
impl Probe {
    pub(super) fn new(
        root: &Path,
        db: Arc<Database>,
        security: Arc<Security>,
        client: &str,
        token: &str,
    ) -> Self {
        let other = security
            .grant(Grant {
                name: "p3c-same-scope-control".into(),
                permissions: Permissions {
                    regular_read: true,
                    full_read: false,
                    write: true,
                },
                scope: Scope {
                    departments: Some(vec!["synthetic A".into()]),
                    task_types: Some(vec!["synthetic T".into()]),
                },
            })
            .unwrap();
        Self {
            root: root.into(),
            db,
            owner: Credentials {
                client_id: client.into(),
                token: token.into(),
            },
            other: Credentials {
                client_id: other.client_id,
                token: other.token,
            },
            pid: 0,
            creation: 0,
        }
    }
    pub(super) fn bind(&mut self, child: &tokio::process::Child) {
        use sha2::{Digest, Sha256};
        self.pid = child.id().unwrap();
        self.creation = created(child);
        let exe = env!("CARGO_BIN_EXE_in-line-mcp");
        writeln!(std::io::stdout().lock(), "P3C_BUILD {}", json!({"fixture":"S-additions","sha":std::env::var("GITHUB_SHA").unwrap(),"sidecarSha256":format!("{:x}",Sha256::digest(std::fs::read(exe).unwrap())),"pid":self.pid,"creationTime":self.creation.to_string(),"runnerEnvironment":std::env::var("RUNNER_ENVIRONMENT").unwrap(),"runnerOs":std::env::var("RUNNER_OS").unwrap()})).unwrap();
    }
    pub(super) fn snapshot(&self) -> Value {
        tables(&self.root.join("synthetic.db"))
    }
    pub(super) fn auth(&self) -> Vec<u8> {
        std::fs::read(self.root.join("security/authorization.dpapi")).unwrap()
    }
    fn permissions(&self, other: bool) -> Value {
        let a = Security::read_at(&self.root.join("security"))
            .unwrap()
            .authorize(if other { &self.other } else { &self.owner })
            .unwrap();
        json!({"regularRead":a.permissions.regular_read,"fullRead":a.permissions.full_read,"write":a.permissions.write,"scope":a.scope})
    }
    pub(super) fn observe(
        &self,
        child: &mut tokio::process::Child,
        id: i64,
        params: &Value,
        response: &Value,
    ) {
        if ![8, 9, 10, 11, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22].contains(&id) {
            return;
        }
        self.log_call(child, id, params, response, false, self.pid, self.creation);
    }
    fn log_call(
        &self,
        child: &mut tokio::process::Child,
        id: i64,
        params: &Value,
        response: &Value,
        other: bool,
        pid: u32,
        creation: u64,
    ) {
        let e = &response["result"]["structuredContent"];
        let status = if e["status"] == "ok" {
            "ok"
        } else {
            e["error"]["code"]
                .as_str()
                .unwrap_or("missing_contract_error")
        };
        let mut args = params["arguments"].clone();
        if args.get("auditId").is_some() {
            args["auditId"] = json!("patch-audit");
        }
        let permission = if id == 13 {
            json!({"paused":true,"effectivePermissions":"denied_by_pause","configuredPermissions":{"regularRead":true,"fullRead":false,"write":true},"scope":{"departments":["synthetic A"],"taskTypes":["synthetic T"]}})
        } else {
            self.permissions(other)
        };
        let same = child.id() == Some(pid)
            && child.try_wait().unwrap().is_none()
            && created(child) == creation;
        record(&format!("connection/{id}"), json!(true), json!(same));
        writeln!(std::io::stdout().lock(), "P3C_CALL {}",json!({"requestId":id,"tool":params["name"],"action":args["action"],"arguments":args,"identity":if other {"S-control"} else {"S-owner"},"permission":permission,"pid":pid,"creationTime":creation.to_string(),"level":"L3","status":status,"isError":response["result"]["isError"]})).unwrap();
    }
    pub(super) fn unchanged(&self, case: &str, before: &Value, auth: &[u8]) {
        let after = self.snapshot();
        let table_checks = before
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), json!(after[k] == *v)))
            .collect::<serde_json::Map<_, _>>();
        let expected = table_checks
            .keys()
            .map(|k| (k.clone(), json!(true)))
            .collect::<serde_json::Map<_, _>>();
        record(
            &format!("{case}/all_tables"),
            json!(expected),
            json!(table_checks),
        );
        record(
            &format!("{case}/authorization"),
            json!(true),
            json!(auth == self.auth()),
        );
    }
    pub(super) fn title(&self, case: &str, before: &Value, value: &str, version: i64, auth: &[u8]) {
        let after = self.snapshot();
        let mut old = by(before, "tasks", "id", &json!(1)).clone();
        let mut new = by(&after, "tasks", "id", &json!(1)).clone();
        for key in ["title", "updated_at"] {
            old.as_object_mut().unwrap().remove(key);
            new.as_object_mut().unwrap().remove(key);
        }
        record(
            &format!("{case}/title_version"),
            json!({"title":value,"titleVersion":version,"taskVersionAdvanced":true,"otherTaskFields":true}),
            json!({"title":self.db.get_task(1).unwrap().title,"titleVersion":rows(&after,"mcp_field_versions").iter().find(|r|r["task_id"]==1&&r["field"]=="title").unwrap()["version"],"taskVersionAdvanced":by(&after,"mcp_task_versions","task_id",&json!(1))["version"].as_i64()>by(before,"mcp_task_versions","task_id",&json!(1))["version"].as_i64(),"otherTaskFields":old==new}),
        );
        for table in [
            "daily_sequences",
            "queue_number_allocations",
            "task_queue_entries",
            "status_history",
            "task_work_events",
            "urgent_records",
            "master_values",
            "settings",
            "mcp_preference_versions",
        ] {
            record(
                &format!("{case}/preserve/{table}"),
                json!(true),
                json!(before[table] == after[table]),
            );
        }
        record(
            &format!("{case}/other_fields_versions"),
            json!(true),
            json!(rows(before, "mcp_field_versions")
                .iter()
                .filter(|r| !(r["task_id"] == 1 && r["field"] == "title"))
                .eq(rows(&after, "mcp_field_versions")
                    .iter()
                    .filter(|r| !(r["task_id"] == 1 && r["field"] == "title")))),
        );
        record(
            &format!("{case}/other_tasks"),
            json!(true),
            json!(rows(before, "tasks")
                .iter()
                .filter(|r| r["id"] != 1)
                .eq(rows(&after, "tasks").iter().filter(|r| r["id"] != 1))),
        );
        record(
            &format!("{case}/old_logs_retained"),
            json!(true),
            json!(rows(before, "task_logs")
                .iter()
                .all(|r| rows(&after, "task_logs").contains(r))),
        );
        record(
            &format!("{case}/authorization"),
            json!(true),
            json!(auth == self.auth()),
        );
    }
    pub(super) fn receipt(&self, key: &str, data: &Value, action: &str) {
        let state = self.snapshot();
        let r = rows(&state, "mcp_receipts")
            .iter()
            .find(|r| r["client_id"] == self.owner.client_id && r["request_key"] == key)
            .unwrap();
        let stored: Value = serde_json::from_str(r["result_json"].as_str().unwrap()).unwrap();
        let audit = by(&state, "mcp_ai_audit", "id", &data["auditId"]);
        record(
            &format!("{key}/receipt_audit"),
            json!({"linked":true,"owner":true,"action":action,"wireVerification":"verified_after_commit","storedVerification":if action=="batch"{Value::Null}else{json!("verified_in_transaction")},"keyCount":1}),
            json!({"linked":r["audit_id"]==data["auditId"]&&stored["auditId"]==data["auditId"],"owner":audit["client_id"]==self.owner.client_id,"action":audit["action"],"wireVerification":data["verificationStatus"],"storedVerification":stored["verificationStatus"],"keyCount":rows(&state,"mcp_receipts").iter().filter(|r|r["client_id"]==self.owner.client_id&&r["request_key"]==key).count()}),
        );
    }
    pub(super) fn pending(&self, before: &Value, data: &Value, auth: &[u8]) {
        let after = self.snapshot();
        for t in [
            "tasks",
            "task_queue_entries",
            "daily_sequences",
            "queue_number_allocations",
            "task_logs",
            "status_history",
            "task_work_events",
            "urgent_records",
            "master_values",
            "mcp_task_versions",
            "mcp_field_versions",
            "mcp_ai_audit",
        ] {
            record(
                &format!("undo_pending/preserve/{t}"),
                json!(true),
                json!(before[t] == after[t]),
            );
        }
        let u = by(&after, "mcp_undo_requests", "id", &data["undoRequestId"]);
        record(
            "undo_pending/request",
            json!({"status":"pending","linked":true,"owner":true,"title":"MCP changed","requestsAdded":1,"receiptsAdded":1}),
            json!({"status":u["status"],"linked":u["audit_id"]==data["auditId"],"owner":u["client_id"]==self.owner.client_id,"title":self.db.get_task(1).unwrap().title,"requestsAdded":rows(&after,"mcp_undo_requests").len()-rows(before,"mcp_undo_requests").len(),"receiptsAdded":rows(&after,"mcp_receipts").len()-rows(before,"mcp_receipts").len()}),
        );
        record(
            "undo_pending/authorization",
            json!(true),
            json!(auth == self.auth()),
        );
    }
    pub(super) fn approved(&self, original: &Value, pending: &Value, undo: &Value, auth: &[u8]) {
        self.title("approved_undo", pending, "original-first", 2, auth);
        let after = self.snapshot();
        let aid = &undo["auditId"];
        let comp = rows(&after, "mcp_ai_audit")
            .iter()
            .filter(|r| r["undo_of"] == *aid)
            .collect::<Vec<_>>();
        let u = by(&after, "mcp_undo_requests", "id", &undo["undoRequestId"]);
        record(
            "approved_undo/history",
            json!({"originalAuditRetained":true,"compensations":1,"action":"approvedUndo","status":"approved","receiptRetained":true,"auditsAdded":1}),
            json!({"originalAuditRetained":by(original,"mcp_ai_audit","id",aid)==by(&after,"mcp_ai_audit","id",aid),"compensations":comp.len(),"action":comp.first().map(|v|&v["action"]),"status":u["status"],"receiptRetained":pending["mcp_receipts"]==after["mcp_receipts"],"auditsAdded":rows(&after,"mcp_ai_audit").len()-rows(pending,"mcp_ai_audit").len()}),
        );
        writeln!(std::io::stdout().lock(),"P3C_APPROVAL {}",json!({"method":"db.mcp_resolve_undo(true)","count":1,"audit":"patch-audit","scope":"self_synthetic_title_only","level":"L1_local_approval_after_L3_request","guiVerified":false})).unwrap();
    }
    pub(super) fn batch_applied(&self, before: &Value, data: &Value, auth: &[u8]) {
        let after = self.snapshot();
        let id = data["items"][0]["taskId"].as_i64().unwrap();
        let c = read(&self.root.join("synthetic.db"));
        let allocated: i64 = c
            .query_row(
                "SELECT count(*) FROM queue_number_allocations WHERE task_id=?",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        let old_max = rows(before, "tasks")
            .iter()
            .map(|r| r["daily_sequence"].as_i64().unwrap())
            .max()
            .unwrap();
        let t = by(&after, "tasks", "id", &json!(id));
        let old = rows(before, "tasks");
        record(
            "batch_apply/known_create",
            json!({"added":1,"title":"p4-stdio-created","nextSequence":true,"newPermanentNumber":true,"oldTasksRetained":true,"allocations":1,"itemVersionMatches":true,"noWorkEvent":true,"authorization":true}),
            json!({"added":rows(&after,"tasks").len()-old.len(),"title":t["title"],"nextSequence":t["daily_sequence"]==old_max+1,"newPermanentNumber":!old.iter().any(|r|r["permanent_number"]==t["permanent_number"]),"oldTasksRetained":old.iter().all(|r|rows(&after,"tasks").contains(r)),"allocations":allocated,"itemVersionMatches":by(&after,"mcp_task_versions","task_id",&json!(id))["version"]==data["taskVersions"][id.to_string()],"noWorkEvent":before["task_work_events"]==after["task_work_events"],"authorization":auth==self.auth()}),
        );
        self.receipt("p4-wire", data, "batch");
    }
    pub(super) fn sentinel(&self) {
        let before = self.db.get_task(1).unwrap();
        let input:TaskInput=serde_json::from_value(json!({"title":"p3c-outside-sentinel","department":"synthetic B","departments":["synthetic B"],"contact":"secret-outside-contact","contacts":["secret-outside-contact"],"taskType":"synthetic T","details":"secret-outside-details","internalNotes":"secret-outside-notes","status":"pending","priority":"normal","workload":"standard","isUrgent":false,"urgentRequester":"","urgentReason":""})).unwrap();
        let t = self.db.save_task(input).unwrap();
        record(
            "sentinel/late_initialization",
            json!({"id":5,"originalId1Unchanged":true,"tasks":5}),
            json!({"id":t.id,"originalId1Unchanged":serde_json::to_value(before).unwrap()==serde_json::to_value(self.db.get_task(1).unwrap()).unwrap(),"tasks":rows(&self.snapshot(),"tasks").len()}),
        );
    }
    fn files(&self) -> Vec<PathBuf> {
        let root = self.root.join("backups/mcp-created");
        if !root.exists() {
            return vec![];
        }
        let mut paths = std::fs::read_dir(root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }
    pub(super) fn artifact(&self, action: &str, data: &Value, before: &Value, auth: &[u8]) {
        let root = self
            .root
            .join("backups/mcp-created")
            .canonicalize()
            .unwrap();
        let p = root
            .join(data["artifactName"].as_str().unwrap())
            .canonicalize()
            .unwrap();
        let paths = self.files();
        record(
            &format!("{action}/file_boundary"),
            json!({"inside":true,"files":if action=="backup"{1}else{2},"allFinalFiles":true,"extension":if action=="backup"{"db"}else{"json"},"noPartial":true,"noBody":true}),
            json!({"inside":p.starts_with(&root)&&root.starts_with(self.root.canonicalize().unwrap())&&self.db.backup_directory().canonicalize().unwrap().join("mcp-created")==root,"files":paths.len(),"allFinalFiles":paths.iter().all(|p|p.is_file()),"extension":p.extension().unwrap().to_str(),"noPartial":paths.iter().all(|p|p.extension().is_some_and(|x|x=="db"||x=="json")),"noBody":data.get("items").is_none()}),
        );
        if action == "backup" {
            let c = read(&p);
            let count: i64 = c
                .query_row("SELECT count(*) FROM tasks", [], |r| r.get(0))
                .unwrap();
            let outside: i64 = c
                .query_row(
                    "SELECT count(*) FROM tasks WHERE title='p3c-outside-sentinel'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            let secrets:i64=c.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND (name LIKE '%token%' OR name LIKE '%credential%' OR name LIKE '%authorization%')",[],|r|r.get(0)).unwrap();
            let backup = tables(&p);
            record(
                "backup/content",
                json!({"tasks":5,"outsideSentinels":1,"credentialTables":0,"businessMatchesBefore":true}),
                json!({"tasks":count,"outsideSentinels":outside,"credentialTables":secrets,"businessMatchesBefore":["tasks","task_queue_entries","daily_sequences","queue_number_allocations","task_work_events","master_values","mcp_field_versions","mcp_task_versions"].iter().all(|t|backup[*t]==before[*t])}),
            );
        } else {
            let bytes = std::fs::read(&p).unwrap();
            let content: Value = serde_json::from_slice(&bytes).unwrap();
            let items = content["items"].as_array().unwrap();
            let mut titles = items
                .iter()
                .map(|r| r["title"].as_str().unwrap())
                .collect::<Vec<_>>();
            titles.sort();
            let mut expected = vec![
                "original-first",
                "original-second",
                "uncertain-create",
                "p4-stdio-created",
            ];
            expected.sort();
            record(
                "export/content",
                json!({"rows":4,"knownTitles":true,"noOutside":true,"privateKeysAbsent":true,"privateValuesAbsent":true,"complete":true}),
                json!({"rows":items.len(),"knownTitles":titles==expected,"noOutside":!items.iter().any(|r|r["title"]=="p3c-outside-sentinel"),"privateKeysAbsent":items.iter().all(|r|["contacts","contact","details","internalNotes","urgentRequester","urgentReason"].iter().all(|k|r.get(*k).is_none())) ,"privateValuesAbsent":!String::from_utf8(bytes).unwrap().contains("secret-"),"complete":content["meta"]["complete"]}),
            );
        }
        let after = self.snapshot();
        let audit = by(&after, "mcp_ai_audit", "id", &data["auditId"]);
        let job = by(
            &after,
            "mcp_artifact_jobs",
            "request_key",
            &json!(format!("p4-wire-{action}")),
        );
        record(
            &format!("{action}/audit_receipt"),
            json!({"action":action,"owner":true,"complete":true,"linked":true,"businessPreserved":true,"authorization":true,"auditsAdded":1}),
            json!({"action":audit["action"],"owner":audit["client_id"]==self.owner.client_id,"complete":job["status"]=="complete","linked":serde_json::from_str::<Value>(job["result_json"].as_str().unwrap()).unwrap()["auditId"]==data["auditId"],"businessPreserved":["tasks","daily_sequences","queue_number_allocations","task_queue_entries","task_logs","task_work_events","status_history","urgent_records","master_values","mcp_task_versions","mcp_field_versions"].iter().all(|t|before[*t]==after[*t]),"authorization":auth==self.auth(),"auditsAdded":rows(&after,"mcp_ai_audit").len()-rows(before,"mcp_ai_audit").len()}),
        );
    }
    pub(super) async fn negative(
        &self,
        input: &mut tokio::process::ChildStdin,
        output: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
        child: &mut tokio::process::Child,
        id: i64,
        tool: &str,
        args: Value,
        code: &str,
    ) {
        self.reject(
            input,
            output,
            child,
            id,
            tool,
            args,
            code,
            false,
            self.pid,
            self.creation,
        )
        .await;
    }
    async fn reject(
        &self,
        input: &mut tokio::process::ChildStdin,
        output: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
        child: &mut tokio::process::Child,
        id: i64,
        tool: &str,
        args: Value,
        code: &str,
        other: bool,
        pid: u32,
        creation: u64,
    ) {
        let before = self.snapshot();
        let auth = self.auth();
        let files = self.files();
        let permission = self.permissions(other);
        record(
            &format!("N{id}/effective_permissions"),
            json!({"regularRead":true,"fullRead":false,"write":id!=803,"scope":{"departments":["synthetic A"],"taskTypes":["synthetic T"]}}),
            permission,
        );
        if id == 802 {
            record(
                "N2/absent_target",
                json!(false),
                json!(rows(&before, "tasks").iter().any(|r| r["id"] == 9999)),
            );
        }
        let params = json!({"name":tool,"arguments":args});
        let r = send(input, output, id, "tools/call", params.clone()).await;
        self.log_call(child, id, &params, &r, other, pid, creation);
        record(
            &format!("N{id}/refusal"),
            json!({"code":code,"isError":true}),
            json!({"code":r["result"]["structuredContent"]["error"]["code"],"isError":r["result"]["isError"]}),
        );
        self.unchanged(&format!("N{id}"), &before, &auth);
        record(
            &format!("N{id}/no_files"),
            json!(true),
            json!(self.files() == files),
        );
    }
    pub(super) async fn foreign_undo(&self, audit: &Value) {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_in-line-mcp"))
            .env("IN_LINE_MCP_TEST_SECURITY_ROOT", self.root.join("security"))
            .env(
                "IN_LINE_MCP_TEST_HOST_EXE",
                std::env::current_exe().unwrap(),
            )
            .env("IN_LINE_MCP_CLIENT_ID", &self.other.client_id)
            .env("IN_LINE_MCP_TOKEN", &self.other.token)
            .env_remove("RUST_LOG")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .creation_flags(0x08000000)
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let pid = child.id().unwrap();
        let creation = created(&child);
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
        let init=send(&mut input,&mut output,1,"initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"p3c-control","version":"1"}})).await;
        assert!(init["result"].is_object());
        input
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
            .await
            .unwrap();
        self.reject(&mut input,&mut output,&mut child,801,"request_undo",json!({"auditId":audit,"idempotencyKey":"p3c-foreign-undo","intent":{"summary":"synthetic foreign audit isolation","explicitUserRequest":true},"reason":"synthetic foreign audit isolation"}),"not_found",true,pid,creation).await;
        drop(input);
        let status = tokio::time::timeout(std::time::Duration::from_secs(5), child.wait())
            .await
            .unwrap()
            .unwrap();
        record("control/eof", json!(true), json!(status.success()));
    }
}
async fn send(
    input: &mut tokio::process::ChildStdin,
    output: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    id: i64,
    method: &str,
    params: Value,
) -> Value {
    input
        .write_all(
            format!(
                "{}\n",
                json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let line = tokio::time::timeout(std::time::Duration::from_secs(20), output.next_line())
        .await
        .expect("P3-C response deadline")
        .unwrap()
        .expect("P3-C premature EOF");
    let response: Value = serde_json::from_str(&line).expect("P3-C JSON required");
    assert_eq!(response["id"], id);
    response
}

#[test]
fn p3c_gate_entry_without_resources() {
    let _ = enabled();
}

#[test]
fn p3c_environment_gate_precedes_all_additional_resources() {
    for (case, github, runner, os, rejection) in [
        ("local", None, "github-hosted", "Windows", None),
        (
            "false_github",
            Some("false"),
            "github-hosted",
            "Windows",
            Some("P3-C requires GITHUB_ACTIONS=true"),
        ),
        (
            "self_hosted",
            Some("true"),
            "self-hosted",
            "Windows",
            Some("P3-C requires RUNNER_ENVIRONMENT=github-hosted"),
        ),
        (
            "linux_label",
            Some("true"),
            "github-hosted",
            "Linux",
            Some("P3-C requires RUNNER_OS=Windows"),
        ),
        (
            "missing_runner",
            Some("true"),
            "",
            "Windows",
            Some("P3-C requires RUNNER_ENVIRONMENT=github-hosted"),
        ),
    ] {
        let temp =
            std::env::temp_dir().join(format!("inline-p3c-gate-{}-{case}", std::process::id()));
        assert!(!temp.exists());
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "p3c::p3c_gate_entry_without_resources",
            "--nocapture",
        ])
        .env_remove("GITHUB_ACTIONS")
        .env("RUNNER_ENVIRONMENT", runner)
        .env("RUNNER_OS", os)
        .env("TMP", &temp)
        .env("TEMP", &temp)
        .env_remove("IN_LINE_MCP_CLIENT_ID")
        .env_remove("IN_LINE_MCP_TOKEN")
        .env_remove("IN_LINE_MCP_TEST_SECURITY_ROOT")
        .env_remove("IN_LINE_MCP_TEST_HOST_EXE")
        .creation_flags(0x08000000);
        if let Some(g) = github {
            cmd.env("GITHUB_ACTIONS", g);
        }
        let out = cmd.output().unwrap();
        let stdout = String::from_utf8(out.stdout).unwrap();
        let stderr = String::from_utf8(out.stderr).unwrap();
        let passed = out.status.success() == rejection.is_none()
            && !temp.exists()
            && stdout.contains("P3C_NOT_RUN ") == rejection.is_none()
            && !["P3C_BUILD ", "P3C_CALL ", "P3C_EVIDENCE "]
                .iter()
                .any(|p| stdout.contains(p))
            && rejection.is_none_or(|r| stderr.contains(r));
        writeln!(std::io::stdout().lock(),"P3C_GATE_CHECK {}",json!({"case":case,"passed":passed,"resourceRootAbsent":!temp.exists(),"businessExecuted":false,"notRunMarker":stdout.contains("P3C_NOT_RUN "),"exitCode":out.status.code()})).unwrap();
        assert!(passed, "P3-C gate case {case}");
    }
}
