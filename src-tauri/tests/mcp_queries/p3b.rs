//! P3-B only: fresh hosted-runner Q/J stores, original stdio connections, no product hooks.
use super::*;
use in_line_lib::mcp::contract::Credentials;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::PathBuf, process::Stdio, time::Duration};
use tokio::process::{Child, ChildStdin, ChildStdout};

fn record(case: &str, expected: Value, actual: Value) {
    use std::io::Write;
    let passed = expected == actual;
    writeln!(
        std::io::stdout().lock(),
        "P3B_EVIDENCE {}",
        json!({
            "case":case,"expected":expected,"actual":actual,"passed":passed
        })
    )
    .unwrap();
    assert!(passed, "P3-B mismatch: {case}");
}
fn created(child: &Child) -> u64 {
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
struct Wire {
    child: Child,
    input: Option<ChildStdin>,
    output: tokio::io::Lines<BufReader<ChildStdout>>,
    sequence: u64,
    created: u64,
    identity: &'static str,
    context: Value,
    aliases: HashMap<String, String>,
}
impl Wire {
    async fn open(
        f: &Fixture,
        credentials: &Credentials,
        identity: &'static str,
        department: &str,
        write: bool,
    ) -> Self {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_in-line-mcp"))
            .env("IN_LINE_MCP_TEST_SECURITY_ROOT", f.root.join("security"))
            .env(
                "IN_LINE_MCP_TEST_HOST_EXE",
                std::env::current_exe().unwrap(),
            )
            .env("IN_LINE_MCP_CLIENT_ID", &credentials.client_id)
            .env("IN_LINE_MCP_TOKEN", &credentials.token)
            .env_remove("RUST_LOG")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        let creation = created(&child);
        let mut wire = Self {
            input: child.stdin.take(),
            output: BufReader::new(child.stdout.take().unwrap()).lines(),
            child,
            sequence: 0,
            created: creation,
            identity,
            context: json!({"regularRead":true,"fullRead":false,"write":write,"scope":{"departments":[department],"taskTypes":["T"]}}),
            aliases: HashMap::new(),
        };
        let init = wire.request("initialize", json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"p3b-synthetic","version":"1"}})).await;
        assert!(init["result"].is_object(), "P3-B initialization failed");
        wire.input
            .as_mut()
            .unwrap()
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
            .await
            .unwrap();
        let caps = wire
            .call("get_capabilities", json!({}), "capabilities", "ok")
            .await;
        record(
            &format!("{identity}/effective_authorization"),
            wire.context.clone(),
            json!({
                "regularRead":caps["permissions"]["regularRead"],"fullRead":caps["permissions"]["fullRead"],"write":caps["permissions"]["write"],"scope":caps["scope"]
            }),
        );
        record(
            &format!("{identity}/exact_build_commit"),
            json!(std::env::var("GITHUB_SHA").unwrap()),
            caps["commit"].clone(),
        );
        record(
            &format!("{identity}/protocol_versions"),
            json!({"software":"0.5.0","mcp":1,"schema":14}),
            json!({"software":caps["softwareVersion"],"mcp":caps["mcpVersion"],"schema":caps["schemaVersion"]}),
        );
        wire
    }
    async fn request(&mut self, method: &str, params: Value) -> Value {
        self.sequence += 1;
        self.input
            .as_mut()
            .unwrap()
            .write_all(
                format!(
                    "{}\n",
                    json!({"jsonrpc":"2.0","id":self.sequence,"method":method,"params":params})
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        loop {
            let line = tokio::time::timeout(Duration::from_secs(30), self.output.next_line())
                .await
                .expect("P3-B protocol deadline")
                .unwrap()
                .expect("P3-B premature EOF");
            let v: Value = serde_json::from_str(&line).expect("P3-B JSON response required");
            if v.get("id").is_none() {
                continue;
            }
            assert_eq!(v["id"], self.sequence);
            return v;
        }
    }
    // Only authored inputs and selected metadata are logged. Opaque identifiers
    // stay inside this fixture, including on failure; never print response bodies.
    async fn call(&mut self, tool: &str, args: Value, case: &str, expected: &str) -> Value {
        let response = self
            .request("tools/call", json!({"name":tool,"arguments":args}))
            .await;
        let envelope = &response["result"]["structuredContent"];
        let actual = if envelope["status"] == "ok" {
            "ok"
        } else {
            envelope["error"]["code"]
                .as_str()
                .unwrap_or("missing_contract_error")
        };
        let mut safe_args = args;
        for key in ["jobId", "waiterId"] {
            if let Some(id) = safe_args[key].as_str() {
                safe_args[key] = json!(self
                    .aliases
                    .get(id)
                    .expect("P3-B opaque ID must have an alias"));
            }
        }
        let alive =
            self.child.try_wait().unwrap().is_none() && created(&self.child) == self.created;
        record(
            &format!("{}/{case}", self.identity),
            json!({"code":expected,"sameProcess":true,"isError":expected!="ok"}),
            json!({"code":actual,"sameProcess":alive,"isError":response["result"]["isError"]}),
        );
        use std::io::Write;
        writeln!(std::io::stdout().lock(), "P3B_CALL {}", json!({"case":case,"identity":self.identity,"tool":tool,"arguments":safe_args,"permission":self.context,"level":"L3","sequence":self.sequence,"pid":self.child.id(),"creationTime":self.created.to_string(),"status":actual})).unwrap();
        if expected == "ok" {
            envelope["data"].clone()
        } else {
            Value::Null
        }
    }
    fn alias(&mut self, id: &Value, alias: &str) {
        self.aliases
            .insert(id.as_str().unwrap().into(), alias.into());
    }
    async fn close(mut self) {
        drop(self.input.take());
        let status = tokio::time::timeout(Duration::from_secs(10), self.child.wait())
            .await
            .expect("P3-B EOF exit deadline")
            .unwrap();
        record(
            &format!("{}/eof_exit", self.identity),
            json!(true),
            json!(status.success()),
        );
    }
}
struct Fixture {
    root: PathBuf,
    db: Arc<Database>,
    owner: Credentials,
    other: Credentials,
}
impl Fixture {
    fn new(kind: &str) -> Self {
        // This gate precedes all database/security/process creation. Never run
        // these acceptance actions against a local workstation or self-hosted host.
        assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(), Ok("true"));
        assert_eq!(
            std::env::var("RUNNER_ENVIRONMENT").as_deref(),
            Ok("github-hosted")
        );
        assert_eq!(std::env::var("RUNNER_OS").as_deref(), Ok("Windows"));
        let root = std::env::temp_dir().join(format!(
            "inline-p3b-{kind}-{}",
            platform::random_secret().unwrap()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let db = Arc::new(Database::open_at(root.join("synthetic.db")).unwrap());
        for (title, department) in [("p3b-inside", "A"), ("p3b-outside", "B")] {
            let task: TaskInput = serde_json::from_value(json!({"title":title,"department":department,"departments":[department],"contact":"synthetic-private","contacts":["synthetic-private"],"taskType":"T","details":"synthetic-private","internalNotes":"synthetic-private","status":"pending","priority":"normal","workload":"standard","isUrgent":false})).unwrap();
            db.save_task(task).unwrap();
        }
        db.set_setting("week_start_day".into(), "monday".into())
            .unwrap();
        let security = Arc::new(Security::open_at(root.join("security")).unwrap());
        let p = Permissions {
            regular_read: true,
            full_read: false,
            write: kind == "Q",
        };
        security.set_groups(p.clone()).unwrap();
        let grant = |name: &str, department: &str, write: bool| {
            let issued = security
                .grant(Grant {
                    name: name.into(),
                    permissions: Permissions { write, ..p.clone() },
                    scope: Scope {
                        departments: Some(vec![department.into()]),
                        task_types: Some(vec!["T".into()]),
                    },
                })
                .unwrap();
            Credentials {
                client_id: issued.client_id,
                token: issued.token,
            }
        };
        let owner = grant("P3-B owner", "A", kind == "Q");
        let other = grant("P3-B other", if kind == "Q" { "B" } else { "A" }, false);
        let sec = security.clone();
        let data = db.clone();
        ipc::start(
            security.clone(),
            Arc::new(move |c, t, a| service::execute(&sec, &data, c, t, a)),
        )
        .unwrap();
        let binary = std::fs::read(env!("CARGO_BIN_EXE_in-line-mcp")).unwrap();
        use std::io::Write;
        writeln!(std::io::stdout().lock(), "P3B_BUILD {}", json!({"fixture":kind,"commit":std::env::var("GITHUB_SHA").unwrap(),"runnerEnvironment":std::env::var("RUNNER_ENVIRONMENT").unwrap(),"runnerOS":std::env::var("RUNNER_OS").unwrap(),"runnerImage":std::env::var("ImageOS").ok(),"runnerImageVersion":std::env::var("ImageVersion").ok(),"sidecarSha256":format!("{:x}",Sha256::digest(binary)),"debug":cfg!(debug_assertions)})).unwrap();
        Self {
            root,
            db,
            owner,
            other,
        }
    }
    fn authorization(&self) -> Vec<u8> {
        std::fs::read(self.root.join("security/authorization.dpapi")).unwrap()
    }
    fn tables(&self, tables: &[&str]) -> Value {
        let c = rusqlite::Connection::open_with_flags(
            self.root.join("synthetic.db"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let mut result = serde_json::Map::new();
        for table in tables {
            let mut s = c
                .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
                .unwrap();
            let columns = s.column_count();
            let rows = s
                .query_map([], |r| {
                    (0..columns)
                        .map(|i| {
                            use rusqlite::types::ValueRef;
                            Ok(match r.get_ref(i)? {
                                ValueRef::Null => Value::Null,
                                ValueRef::Integer(n) => json!(n),
                                ValueRef::Real(n) => json!(n),
                                ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                                ValueRef::Blob(b) => json!(b),
                            })
                        })
                        .collect::<Result<Vec<Value>, rusqlite::Error>>()
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            result.insert((*table).into(), json!(rows));
        }
        Value::Object(result)
    }
    fn business(&self) -> Value {
        self.tables(&[
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
        ])
    }
    fn preferences(&self) -> Value {
        self.tables(&[
            "settings",
            "mcp_preference_versions",
            "mcp_ai_audit",
            "mcp_receipts",
            "mcp_undo_requests",
        ])
    }
    fn basis(&self) -> Value {
        // Integration tests cannot call crate-private mcp_basis. Compare every
        // stored basis input through this synthetic database's read-only handle;
        // query/statistics schema constants are fixed by the same test SHA.
        self.tables(&["mcp_data_basis", "mcp_change_index", "schema_meta"])
    }
}
fn patch(key: &str, base: &str, value: &str, version: i64, summary: &str) -> Value {
    json!({"action":"patch","patch":{"week_start_day":value},"fieldBase":{"week_start_day":base},"fieldVersions":{"week_start_day":version},"idempotencyKey":key,"intent":{"summary":summary,"explicitUserRequest":true},"reason":summary})
}
fn receipt(
    f: &Fixture,
    key: &str,
    data: &Value,
    base: &str,
    value: &str,
    version: i64,
    summary: &str,
) {
    let c = rusqlite::Connection::open_with_flags(
        f.root.join("synthetic.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let row: (Option<i64>, String) = c
        .query_row(
            "SELECT audit_id,result_json FROM mcp_receipts WHERE client_id=? AND request_key=?",
            rusqlite::params![f.owner.client_id, key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let stored: Value = serde_json::from_str(&row.1).unwrap();
    // mcp_transaction stores the operation receipt before COMMIT, then performs
    // an independent readback under the same mutex and promotes the wire result.
    // Assert both exact stages; every other receipt field must match unchanged.
    let mut transaction_receipt = data.clone();
    transaction_receipt["verificationStatus"] = json!("verified_in_transaction");
    let audits = f.db.mcp_audit_state().unwrap();
    let audit = audits["audits"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == data["auditId"])
        .unwrap();
    record(
        &format!("Q/{key}/receipt_audit"),
        json!({"linked":true,"storedReceiptMatchesTransactionStage":true,"storedVerification":"verified_in_transaction","wireVerification":"verified_after_commit","action":"preferences","ownerMatches":true,"base":base,"value":value,"beforeVersion":version,"afterVersion":version+1,"reason":summary,"intent":summary,"undoOf":null}),
        json!({"linked":row.0==data["auditId"].as_i64(),"storedReceiptMatchesTransactionStage":stored==transaction_receipt,"storedVerification":stored["verificationStatus"],"wireVerification":data["verificationStatus"],"action":audit["action"],"ownerMatches":audit["clientId"]==f.owner.client_id,"base":audit["before"]["week_start_day"],"value":audit["after"]["week_start_day"],"beforeVersion":audit["before"]["fieldVersions"]["week_start_day"],"afterVersion":audit["after"]["fieldVersions"]["week_start_day"],"reason":audit["reason"],"intent":audit["intent"],"undoOf":audit["undoOf"]}),
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn p3b_q_saved_query_and_shared_preference_compensation_over_original_stdio() {
    let f = Fixture::new("Q");
    let business = f.business();
    let auth = f.authorization();
    let initial = f.preferences();
    let mut owner = Wire::open(&f, &f.owner, "Q-owner", "A", true).await;
    let mut other = Wire::open(&f, &f.other, "Q-other", "B", false).await;
    let name = "p3b-only-query";
    let list = owner
        .call(
            "manage_saved_query",
            json!({"action":"list"}),
            "initial_list",
            "ok",
        )
        .await;
    record("Q/initial_names", json!([]), list["names"].clone());
    let definition = json!({"action":"save","name":name,"filters":{"departments":["A"],"taskTypes":["T"]},"projection":["id","title"],"explicitIntent":true});
    for case in ["save", "equivalent_save"] {
        let saved = owner
            .call("manage_saved_query", definition.clone(), case, "ok")
            .await;
        record(
            &format!("Q/{case}"),
            json!({"name":name,"saved":true}),
            saved,
        );
        let list = owner
            .call(
                "manage_saved_query",
                json!({"action":"list"}),
                &format!("{case}_list"),
                "ok",
            )
            .await;
        record(
            &format!("Q/{case}_names"),
            json!([name]),
            list["names"].clone(),
        );
        let get = owner
            .call(
                "manage_saved_query",
                json!({"action":"get","name":name}),
                &format!("{case}_get"),
                "ok",
            )
            .await;
        record(
            &format!("Q/{case}_definition"),
            json!({"name":name,"departments":["A"],"taskTypes":["T"],"projection":["id","title"]}),
            json!({"name":get["name"],"departments":get["definition"]["filters"]["departments"],"taskTypes":get["definition"]["filters"]["taskTypes"],"projection":get["definition"]["projection"]}),
        );
    }
    let list = other
        .call(
            "manage_saved_query",
            json!({"action":"list"}),
            "private_list",
            "ok",
        )
        .await;
    record("Q/other_names", json!([]), list["names"].clone());
    other
        .call(
            "manage_saved_query",
            json!({"action":"get","name":name}),
            "private_get",
            "not_found",
        )
        .await;
    let tasks = owner
        .call(
            "query_tasks",
            json!({"filters":{"departments":["A"],"taskTypes":["T"]},"projection":["id","title"]}),
            "definition_execution_current_scope",
            "ok",
        )
        .await;
    record(
        "Q/known_scoped_tasks",
        json!({"total":1,"inside":true}),
        json!({"total":tasks["total"],"inside":tasks["items"][0]["title"]=="p3b-inside"}),
    );
    let hidden = owner
        .call(
            "query_tasks",
            json!({"filters":{"ids":[2]},"projection":["id","title"]}),
            "outside_id_scope",
            "ok",
        )
        .await;
    record("Q/outside_scope_rows", json!(0), hidden["total"].clone());
    let deleted = owner
        .call(
            "manage_saved_query",
            json!({"action":"delete","name":name,"explicitIntent":true}),
            "delete",
            "ok",
        )
        .await;
    record("Q/deleted", json!({"name":name,"deleted":true}), deleted);
    owner
        .call(
            "manage_saved_query",
            json!({"action":"get","name":name}),
            "deleted_get",
            "not_found",
        )
        .await;
    let list = owner
        .call(
            "manage_saved_query",
            json!({"action":"list"}),
            "deleted_list",
            "ok",
        )
        .await;
    record("Q/deleted_names", json!([]), list["names"].clone());
    record(
        "Q/definition_only_preservation",
        json!(true),
        json!(f.business() == business && f.preferences() == initial && f.authorization() == auth),
    );

    let prefs = owner
        .call(
            "manage_preferences",
            json!({"action":"get"}),
            "initialized_get",
            "ok",
        )
        .await;
    let version = prefs["fieldVersions"]["week_start_day"].as_i64().unwrap();
    record(
        "Q/initialized_value",
        json!("monday"),
        prefs["preferences"]["week_start_day"].clone(),
    );
    let first = patch(
        "p3b-Q-monday-to-sunday",
        "monday",
        "sunday",
        version,
        "P3-B explicitly approved monday to sunday",
    );
    let committed = owner
        .call("manage_preferences", first.clone(), "patch_forward", "ok")
        .await;
    record(
        "Q/first_commit",
        json!({"commitStatus":"committed","value":"sunday","version":version+1,"replayed":false,"changedFields":["week_start_day"],"verification":"verified_after_commit"}),
        json!({"commitStatus":committed["commitStatus"],"value":committed["preferences"]["week_start_day"],"version":committed["fieldVersions"]["week_start_day"],"replayed":committed["replayed"],"changedFields":committed["changedFields"],"verification":committed["verificationStatus"]}),
    );
    receipt(
        &f,
        "p3b-Q-monday-to-sunday",
        &committed,
        "monday",
        "sunday",
        version,
        "P3-B explicitly approved monday to sunday",
    );
    let after_first = f.preferences();
    let replay = owner
        .call(
            "manage_preferences",
            first.clone(),
            "same_key_same_parameters",
            "ok",
        )
        .await;
    let mut expected = committed.clone();
    expected["replayed"] = json!(true);
    record(
        "Q/replay_original_receipt",
        json!(true),
        json!(replay == expected && f.preferences() == after_first),
    );
    let mut different = first.clone();
    different["patch"]["week_start_day"] = json!("monday");
    owner
        .call(
            "manage_preferences",
            different,
            "same_key_different_parameters",
            "idempotency_conflict",
        )
        .await;
    record(
        "Q/idempotency_conflict_atomic",
        json!(true),
        json!(f.preferences() == after_first),
    );
    for (case, mut args, code) in [
        (
            "stale_base",
            patch(
                "p3b-stale-base",
                "monday",
                "monday",
                version + 1,
                "P3-B stale base probe",
            ),
            "conflict",
        ),
        (
            "stale_version",
            patch(
                "p3b-stale-version",
                "sunday",
                "monday",
                version,
                "P3-B stale version probe",
            ),
            "conflict",
        ),
        (
            "not_whitelisted",
            patch(
                "p3b-nonwhite",
                "sunday",
                "monday",
                version + 1,
                "P3-B whitelist probe",
            ),
            "invalid_arguments",
        ),
        (
            "missing_explicit_intent",
            patch(
                "p3b-no-intent",
                "sunday",
                "monday",
                version + 1,
                "P3-B explicit intent probe",
            ),
            "explicit_intent_required",
        ),
    ] {
        if case == "not_whitelisted" {
            args["patch"]["launch_at_login"] = json!("true");
            args["fieldBase"]["launch_at_login"] = Value::Null;
            args["fieldVersions"]["launch_at_login"] = json!(0);
        }
        if case == "missing_explicit_intent" {
            args["intent"]["explicitUserRequest"] = json!(false);
        }
        owner.call("manage_preferences", args, case, code).await;
        record(
            &format!("Q/{case}_atomic"),
            json!(true),
            json!(
                f.preferences() == after_first
                    && f.business() == business
                    && f.authorization() == auth
            ),
        );
    }
    other
        .call(
            "manage_preferences",
            patch(
                "p3b-other-denied",
                "sunday",
                "monday",
                version + 1,
                "P3-B read-only denial",
            ),
            "no_write_permission",
            "forbidden",
        )
        .await;
    record(
        "Q/forbidden_atomic",
        json!(true),
        json!(f.preferences() == after_first),
    );
    for (wire, value, ver, case) in [
        (&mut owner, "sunday", version + 1, "owner_committed_get"),
        (&mut other, "sunday", version + 1, "shared_committed_get"),
    ] {
        let read = wire
            .call("manage_preferences", json!({"action":"get"}), case, "ok")
            .await;
        record(
            &format!("Q/{case}"),
            json!({"value":value,"version":ver}),
            json!({"value":read["preferences"]["week_start_day"],"version":read["fieldVersions"]["week_start_day"]}),
        );
    }
    // Compensation is reached only after the first committed receipt, audit,
    // state and version have all passed independent readback above.
    let second = patch(
        "p3b-Q-sunday-to-monday",
        "sunday",
        "monday",
        version + 1,
        "P3-B separately approved sunday to monday compensation",
    );
    let compensated = owner
        .call("manage_preferences", second, "patch_compensation", "ok")
        .await;
    record(
        "Q/second_commit",
        json!({"commitStatus":"committed","value":"monday","version":version+2,"replayed":false,"distinctAudit":true}),
        json!({"commitStatus":compensated["commitStatus"],"value":compensated["preferences"]["week_start_day"],"version":compensated["fieldVersions"]["week_start_day"],"replayed":compensated["replayed"],"distinctAudit":compensated["auditId"]!=committed["auditId"]}),
    );
    receipt(
        &f,
        "p3b-Q-sunday-to-monday",
        &compensated,
        "sunday",
        "monday",
        version + 1,
        "P3-B separately approved sunday to monday compensation",
    );
    // The original audit and receipt remain intact after compensation.
    receipt(
        &f,
        "p3b-Q-monday-to-sunday",
        &committed,
        "monday",
        "sunday",
        version,
        "P3-B explicitly approved monday to sunday",
    );
    for wire in [&mut owner, &mut other] {
        let read = wire
            .call(
                "manage_preferences",
                json!({"action":"get"}),
                "compensated_shared_get",
                "ok",
            )
            .await;
        record(
            &format!("{}/final_preference", wire.identity),
            json!({"value":"monday","version":version+2}),
            json!({"value":read["preferences"]["week_start_day"],"version":read["fieldVersions"]["week_start_day"]}),
        );
    }
    let final_state = f.preferences();
    let audit_count = f.db.mcp_audit_state().unwrap();
    record(
        "Q/final_preservation",
        json!({"business":true,"authorization":true,"settingsRestored":true,"audits":2,"receipts":2,"pendingUndo":0,"otherVersionsUnchanged":true}),
        json!({"business":f.business()==business,"authorization":f.authorization()==auth,"settingsRestored":final_state["settings"]==initial["settings"],"audits":audit_count["audits"].as_array().unwrap().len(),"receipts":final_state["mcp_receipts"].as_array().unwrap().len(),"pendingUndo":audit_count["pending"].as_array().unwrap().len(),"otherVersionsUnchanged":final_state["mcp_preference_versions"].as_array().unwrap().iter().filter(|r|r[0]!="week_start_day").eq(initial["mcp_preference_versions"].as_array().unwrap().iter().filter(|r|r[0]!="week_start_day"))}),
    );
    // No database/config/artifact cleanup: hosted-runner disposal owns these
    // stores; failed states remain available for diagnosis during this job.
    owner.close().await;
    other.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn p3b_j_one_read_job_two_waiters_and_owner_isolation_over_original_stdio() {
    let f = Fixture::new("J");
    let business = f.business();
    let prefs = f.preferences();
    let auth = f.authorization();
    let basis = f.basis();
    let mut owner = Wire::open(&f, &f.owner, "J-owner", "A", false).await;
    let mut other = Wire::open(&f, &f.other, "J-other", "A", false).await;
    let start = json!({"action":"start","kind":"query_tasks","arguments":{"filters":{"departments":["A"],"taskTypes":["T"]},"projection":["id","title"],"limit":100}});
    let first = owner
        .call("manage_jobs", start.clone(), "start_job", "ok")
        .await;
    record(
        "J/first_job",
        json!({"reused":false,"kind":"query_tasks"}),
        json!({"reused":first["reused"],"kind":first["job"]["kind"]}),
    );
    let job = first["job"]["jobId"].clone();
    let w1 = first["waiterId"].clone();
    owner.alias(&job, "job-1");
    owner.alias(&w1, "waiter-1");
    // Wait for the known single-row job before the second start. This verifies
    // completed-job reuse and waiter cancellation, not in-flight cancellation.
    let mut completed = false;
    for _ in 0..100 {
        let get = owner
            .call(
                "manage_jobs",
                json!({"action":"get","jobId":job}),
                "poll_original_job",
                "ok",
            )
            .await;
        if get["state"] == "completed" {
            completed = true;
            break;
        }
        assert!(
            ["queued", "running", "publishing"].contains(&get["state"].as_str().unwrap()),
            "P3-B job failed before completion"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(completed, "P3-B bounded job completion");
    record(
        "J/stable_basis_before_reuse",
        json!(true),
        json!(f.basis() == basis),
    );
    let second = owner
        .call("manage_jobs", start, "equivalent_start", "ok")
        .await;
    let w2 = second["waiterId"].clone();
    owner.alias(&w2, "waiter-2");
    for (id, alias) in [(&job, "job-1"), (&w1, "waiter-1"), (&w2, "waiter-2")] {
        other.alias(id, alias);
    }
    record(
        "J/job_and_waiter_identity",
        json!({"sameJob":true,"distinctWaiters":true,"reused":true,"state":"completed"}),
        json!({"sameJob":second["job"]["jobId"]==job,"distinctWaiters":w1!=w2,"reused":second["reused"],"state":second["job"]["state"]}),
    );
    let list = owner
        .call("manage_jobs", json!({"action":"list"}), "owner_list", "ok")
        .await;
    record(
        "J/owner_one_job",
        json!({"jobs":1,"sameJob":true}),
        json!({"jobs":list["items"].as_array().unwrap().len(),"sameJob":list["items"][0]["jobId"]==job}),
    );
    let list = other
        .call("manage_jobs", json!({"action":"list"}), "other_list", "ok")
        .await;
    record("J/other_no_jobs", json!([]), list["items"].clone());
    for (case, args) in [
        ("other_get", json!({"action":"get","jobId":job})),
        (
            "other_result_impersonate",
            json!({"action":"result","jobId":job,"waiterId":w2}),
        ),
        (
            "other_cancel_waiter",
            json!({"action":"cancelWaiter","jobId":job,"waiterId":w1}),
        ),
        (
            "other_cancel_job_no_intent",
            json!({"action":"cancelJob","jobId":job}),
        ),
    ] {
        other.call("manage_jobs", args, case, "not_found").await;
    }
    let before = owner
        .call(
            "manage_jobs",
            json!({"action":"get","jobId":job}),
            "before_cancel_intent_denial",
            "ok",
        )
        .await;
    owner
        .call(
            "manage_jobs",
            json!({"action":"cancelJob","jobId":job}),
            "cancel_job_without_intent",
            "explicit_intent_required",
        )
        .await;
    let after = owner
        .call(
            "manage_jobs",
            json!({"action":"get","jobId":job}),
            "after_cancel_intent_denial",
            "ok",
        )
        .await;
    record(
        "J/cancel_job_denial_preserves_state",
        json!(true),
        json!(before == after),
    );
    let cancelled = owner
        .call(
            "manage_jobs",
            json!({"action":"cancelWaiter","jobId":job,"waiterId":w1}),
            "cancel_waiter_1",
            "ok",
        )
        .await;
    record(
        "J/completed_waiter_cancel",
        json!({"waiting":false,"sameWaiter":true,"jobState":"completed"}),
        json!({"waiting":cancelled["waiting"],"sameWaiter":cancelled["waiterId"]==w1,"jobState":cancelled["job"]["state"]}),
    );
    owner
        .call(
            "manage_jobs",
            json!({"action":"result","jobId":job,"waiterId":w1}),
            "cancelled_waiter_result",
            "waiter_cancelled",
        )
        .await;
    let result = owner
        .call(
            "manage_jobs",
            json!({"action":"result","jobId":job,"waiterId":w2}),
            "surviving_waiter_result",
            "ok",
        )
        .await;
    // Log only predicates/counts, never the job result body or real opaque IDs.
    record(
        "J/known_synthetic_result",
        json!({"rows":1,"knownInside":true,"complete":true,"state":"completed","sameJob":true,"keys":["id","title"]}),
        json!({"rows":result["items"].as_array().unwrap().len(),"knownInside":result["items"][0]["id"]==1 && result["items"][0]["title"]=="p3b-inside","complete":result["complete"],"state":result["job"]["state"],"sameJob":result["job"]["jobId"]==job,"keys":result["items"][0].as_object().unwrap().keys().collect::<Vec<_>>()}),
    );
    record(
        "J/final_preservation",
        json!({"business":true,"preferencesAuditsReceipts":true,"authorization":true,"basis":true}),
        json!({"business":f.business()==business,"preferencesAuditsReceipts":f.preferences()==prefs,"authorization":f.authorization()==auth,"basis":f.basis()==basis}),
    );
    owner.close().await;
    other.close().await;
}
