//! Real console processes against the isolated protected host; no installed app/data.
use super::*;
use in_line_lib::mcp::security::{Permissions, Scope};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

fn fixture(full: bool) -> Fixture {
    let f = Fixture::new();
    let p = Permissions {
        regular_read: true,
        full_read: full,
        write: true,
    };
    f.security.set_groups(p.clone()).unwrap();
    f.security
        .update_client(
            &f.credentials.client_id,
            p,
            Scope {
                departments: Some(vec!["A".into()]),
                task_types: None,
            },
        )
        .unwrap();
    f.save("chosen", &f.profile());
    f
}
fn body(key: &str) -> Value {
    json!({"idempotencyKey":key,"intent":{"summary":"synthetic explicit request","explicitUserRequest":true,"confirmedRealWork":true},"reason":"synthetic user request"})
}
fn snapshot(path: &Path) -> Value {
    let conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let mut result = serde_json::Map::new();
    for table in [
        "tasks",
        "task_queue_entries",
        "daily_sequences",
        "status_history",
        "task_work_events",
        "urgent_records",
    ] {
        let mut statement = conn
            .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
            .unwrap();
        let columns = statement.column_count();
        let values = statement
            .query_map([], |row| {
                (0..columns)
                    .map(|i| {
                        Ok(match row.get_ref(i)? {
                            rusqlite::types::ValueRef::Null => Value::Null,
                            rusqlite::types::ValueRef::Integer(n) => json!(n),
                            rusqlite::types::ValueRef::Real(n) => json!(n),
                            rusqlite::types::ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                            rusqlite::types::ValueRef::Blob(b) => json!(b),
                        })
                    })
                    .collect::<Result<Vec<Value>, rusqlite::Error>>()
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        result.insert(table.into(), json!(values));
    }
    Value::Object(result)
}
async fn run(f: &Fixture, words: &[&str], input: Option<Value>, execute: bool) -> (i32, Value) {
    let mut args = words.to_vec();
    args.extend(["--profile", "chosen", "--format", "json"]);
    if execute {
        args.push("--execute");
    }
    let bytes = input.map(|v| serde_json::to_vec(&v).unwrap());
    if bytes.is_some() {
        args.extend(["--input", "-"]);
    }
    f.run(&args, bytes.as_deref()).await
}
fn event(db: &Database, id: i64, result: &str, at: &str) {
    db.record_work_event(serde_json::from_value(json!({"taskId":id,"resultStatus":result,"handledAt":at,"note":"private-note-not-in-daily","syncStatus":false})).unwrap()).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn phase5_batch_forces_preview_no_number_consumption_atomic_conflict_and_local_approval() {
    let f = fixture(false);
    let db = Arc::new(Database::open_at(f.root.join("batch.db")).unwrap());
    let a = db.save_task(task("first", "A")).unwrap();
    let b = db.save_task(task("second", "A")).unwrap();
    let calls = Arc::new(Mutex::new(Vec::<Value>::new()));
    let trace = calls.clone();
    let security = f.security.clone();
    let d = db.clone();
    in_line_lib::mcp::ipc::start(
        f.security.clone(),
        Arc::new(move |c, tool, args| {
            if tool == "batch_mutate" {
                trace.lock().unwrap().push(args.clone());
            }
            in_line_lib::mcp::service::execute(&security, &d, c, tool, args)
        }),
    )
    .unwrap();
    let version = |id| db.mcp_task_basis(id, true).unwrap()["taskVersion"].clone();
    let mut input = body("batch-preview");
    input["operations"] = json!([
        {"operation":{"action":"setStatus","target":{"taskId":a.id},"taskVersion":version(a.id),"status":"processing"}},
        {"operation":{"action":"setUrgent","target":{"taskId":b.id},"taskVersion":version(b.id),"isUrgent":true,"requester":"synthetic","urgentReason":"requested"}}
    ]);
    let before = snapshot(&f.root.join("batch.db"));
    let basis = in_line_lib::mcp::service::execute(
        &f.security,
        &db,
        &f.credentials,
        "get_report_summary",
        json!({"startDate":"2026-10-22","endDate":"2026-10-22"}),
    )
    .unwrap()["meta"]["dataVersion"]
        .clone();
    let (code, preview) = run(&f, &["batch", "preview"], Some(input.clone()), false).await;
    assert_eq!(code, 0, "{preview}");
    assert_eq!(preview["data"]["commitStatus"], "dry_run");
    assert_eq!(snapshot(&f.root.join("batch.db")), before);
    assert_eq!(preview["data"]["basis"], basis);
    input["idempotencyKey"] = json!("batch-apply");
    let (code, applied) = run(&f, &["batch", "execute"], Some(input.clone()), true).await;
    assert_eq!(code, 0, "{applied}");
    assert_eq!(
        applied["data"]["verificationStatus"],
        "verified_after_commit"
    );
    let trace = calls.lock().unwrap().clone();
    assert_eq!(trace.len(), 3);
    assert_eq!(trace[1]["action"], "dryRun");
    assert_eq!(trace[2]["action"], "apply");
    assert_eq!(trace[2]["expectedBasis"], preview["data"]["basis"]);
    assert_eq!(db.get_task(a.id).unwrap().status, "processing");
    assert!(db.get_task(b.id).unwrap().is_urgent);
    input["idempotencyKey"] = json!("batch-stale");
    input["operations"][0]["operation"]["status"] = json!("cancelled");
    let before = snapshot(&f.root.join("batch.db"));
    let (code, failed) = run(&f, &["batch", "execute"], Some(input), true).await;
    assert_eq!(code, 5, "{failed}");
    assert_eq!(snapshot(&f.root.join("batch.db")), before);
    assert_eq!(calls.lock().unwrap().last().unwrap()["action"], "dryRun");
    let mut dates = body("batch-dates");
    dates["operations"] = json!([
        {"operation":{"action":"reschedule","target":{"taskId":a.id},"taskVersion":version(a.id),"plannedDate":"2030-01-03"}},
        {"operation":{"action":"reschedule","target":{"taskId":b.id},"taskVersion":version(b.id),"plannedDate":"2030-01-03"}}
    ]);
    let before = snapshot(&f.root.join("batch.db"));
    let (code, pending) = run(&f, &["batch", "execute"], Some(dates), true).await;
    assert_eq!(code, 8, "{pending}");
    assert_eq!(pending["data"]["commitStatus"], "needs_user_approval");
    assert_eq!(snapshot(&f.root.join("batch.db")), before);
    let (_, receipt) = run(
        &f,
        &["receipt", "get", "--idempotency-key", "batch-dates"],
        None,
        false,
    )
    .await;
    assert_eq!(
        receipt["data"]["receipt"]["commitStatus"],
        "needs_user_approval"
    );
    let mut too_many = body("too-many");
    too_many["operations"] = json!(vec![
        json!({"operation":{"action":"setStatus","target":{"taskId":a.id},"taskVersion":version(a.id),"status":"processing"}});
        21
    ]);
    let (code, v) = run(&f, &["batch", "preview"], Some(too_many), false).await;
    assert_eq!(code, 6, "{v}");
    assert_eq!(v["error"]["code"], "resource_limit");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn phase5_saved_queries_preferences_incremental_inspection_and_jobs_use_original_services() {
    let f = fixture(false);
    let db = Arc::new(Database::open_at(f.root.join("automation.db")).unwrap());
    let t = db.save_task(task("inside", "A")).unwrap();
    db.save_task(task("outside", "B")).unwrap();
    event(&db, t.id, "processed", "2026-10-22T09:00:00+08:00");
    host(&f, db.clone());
    for words in [
        vec!["data", "cache"],
        vec!["preferences", "get"],
        vec!["query", "list"],
        vec!["jobs", "list"],
    ] {
        let (code, v) = run(&f, &words, None, false).await;
        assert_eq!(code, 0, "{v}");
    }
    let (code, v) = run(
        &f,
        &["query", "save", "--name", "mine", "--explicit-intent"],
        Some(json!({"filters":{"departments":["A"]},"projection":["id","title"]})),
        true,
    )
    .await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["saved"], true);
    let (code, v) = run(&f, &["query", "get", "--name", "mine"], None, false).await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(
        v["data"]["definition"]["projection"],
        json!(["id", "title"])
    );
    let (code, v) = run(
        &f,
        &["query", "delete", "--name", "mine", "--explicit-intent"],
        None,
        true,
    )
    .await;
    assert_eq!(code, 0, "{v}");
    let (code, initial) = run(
        &f,
        &["changes", "list"],
        Some(json!({"projection":["id","title","status"],"mode":"current"})),
        false,
    )
    .await;
    assert_eq!(code, 0, "{initial}");
    assert_eq!(initial["data"]["fullSyncComplete"], true);
    assert_eq!(initial["data"]["items"].as_array().unwrap().len(), 1);
    db.set_status(t.id, "processing".into()).unwrap();
    let (code,delta)=run(&f,&["changes","list"],Some(json!({"projection":["id","title","status"],"mode":"current","cursor":initial["data"]["nextCursor"]})),false).await;
    assert_eq!(code, 0, "{delta}");
    assert_eq!(delta["data"]["phase"], "incremental");
    assert!(!delta.to_string().contains("outside"));
    assert!(!delta["data"]["items"].as_array().unwrap().is_empty());
    let (code, summary) = run(
        &f,
        &[
            "report",
            "daily",
            "--date",
            "2026-10-22",
            "--timezone-offset-minutes",
            "480",
        ],
        None,
        false,
    )
    .await;
    assert_eq!(code, 0, "{summary}");
    assert_eq!(summary["data"]["statistics"]["summary"]["handledTasks"], 1);
    let report = json!({"startDate":"2026-10-22","endDate":"2026-10-22","timezoneOffsetMinutes":480,"snapshot":summary["data"]["meta"]["snapshotId"]});
    let (code, trace) = run(
        &f,
        &["stats", "trace"],
        Some(json!({"report":report,"metric":"summary.handledTasks"})),
        false,
    )
    .await;
    assert_eq!(code, 0, "{trace}");
    let (code, v) = run(
        &f,
        &["stats", "compare"],
        Some(json!({"report":report,"right":report})),
        false,
    )
    .await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["differences"], json!([]));
    let (_, prefs) = run(&f, &["preferences", "get"], None, false).await;
    let mut patch = body("pref-week");
    patch["patch"] = json!({"week_start_day":"sunday"});
    patch["fieldBase"] = json!({"week_start_day":prefs["data"]["preferences"]["week_start_day"]});
    patch["fieldVersions"] =
        json!({"week_start_day":prefs["data"]["fieldVersions"]["week_start_day"]});
    let (code, v) = run(&f, &["preferences", "patch"], Some(patch), true).await;
    assert_eq!(code, 0, "{v}");
    let (code, v) = run(
        &f,
        &["report", "weekly", "--date", "2026-10-22"],
        None,
        false,
    )
    .await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["startDate"], "2026-10-18");
    let (code, job) = run(
        &f,
        &["jobs", "start"],
        Some(json!({"kind":"query_tasks","arguments":{"projection":["id","title"],"limit":100}})),
        true,
    )
    .await;
    assert_eq!(code, 8, "{job}");
    let job_id = job["data"]["job"]["jobId"].as_str().unwrap();
    let mut completed = false;
    for _ in 0..20 {
        let (code, v) = run(&f, &["jobs", "result", "--job-id", job_id], None, false).await;
        if code == 0 {
            assert_eq!(v["data"]["items"].as_array().unwrap().len(), 1);
            completed = true;
            break;
        }
        assert_eq!(code, 8, "{v}");
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(completed, "bounded job completion");
    let (code, v) = run(
        &f,
        &[
            "jobs",
            "cancel-waiter",
            "--job-id",
            job_id,
            "--waiter-id",
            job["data"]["waiterId"].as_str().unwrap(),
        ],
        None,
        true,
    )
    .await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["waiting"], false);
    let (code, v) = run(
        &f,
        &[
            "jobs",
            "cancel",
            "--job-id",
            job_id,
            "--explicit-user-request",
        ],
        None,
        true,
    )
    .await;
    assert_eq!(code, 0, "{v}");
    let mut export = body("export");
    export["format"] = json!("json");
    export["projection"] = json!(["id", "title"]);
    let (code, v) = run(&f, &["data", "export"], Some(export), true).await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["commitStatus"], "artifact_created");
    assert!(v["data"].get("items").is_none());
    assert!(!v.to_string().contains("private-contact"));
    let before = snapshot(&f.root.join("automation.db"));
    let (code, receipt) = run(
        &f,
        &[
            "receipt",
            "get",
            "--kind",
            "artifact",
            "--idempotency-key",
            "export",
        ],
        None,
        false,
    )
    .await;
    assert_eq!(code, 0, "{receipt}");
    assert_eq!(receipt["data"]["artifactContentsReturned"], false);
    assert_eq!(
        receipt["data"]["receipt"]["artifactName"],
        v["data"]["artifactName"]
    );
    assert_eq!(snapshot(&f.root.join("automation.db")), before);
    let (code, backup) = run(&f, &["data", "backup"], Some(body("backup")), true).await;
    assert_eq!(code, 0, "{backup}");
    assert_eq!(backup["data"]["kind"], "backup");
    assert!(backup["data"]["artifactName"]
        .as_str()
        .unwrap()
        .ends_with(".db"));
    assert!(backup["data"].get("items").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn phase5_daily_full_read_same_snapshot_paging_and_raw_text_are_complete() {
    let f = fixture(true);
    let db = Arc::new(Database::open_at(f.root.join("daily.db")).unwrap());
    let mut first = None;
    for n in 0..101 {
        let t = db.save_task(task(&format!("daily-{n:03}"), "A")).unwrap();
        event(&db, t.id, "processed", "2026-10-22T09:00:00+08:00");
        if n == 0 {
            first = Some(t.id);
        }
    }
    let waiting = db.save_task(task("excluded-latest-waiting", "A")).unwrap();
    event(&db, waiting.id, "processed", "2026-10-22T09:00:00+08:00");
    event(
        &db,
        waiting.id,
        "waiting_materials",
        "2026-10-22T10:00:00+08:00",
    );
    let outside = db.save_task(task("outside", "B")).unwrap();
    event(&db, outside.id, "completed", "2026-10-22T09:00:00+08:00");
    host(&f, db.clone());
    let args =
        json!({"period":"daily","date":"2026-10-22","text":true,"timezoneOffsetMinutes":480});
    let daily = in_line_lib::mcp::service::execute(
        &f.security,
        &db,
        &f.credentials,
        "cli_report",
        args.clone(),
    )
    .unwrap();
    assert_eq!(daily["items"].as_array().unwrap().len(), 101);
    assert_eq!(daily["complete"], true);
    assert_eq!(daily["meta"]["complete"], true);
    let text = daily["text"].as_str().unwrap();
    assert_eq!(text.lines().count(), 103);
    assert!(text.starts_with("2026-10-22 今日计划完成事项\n口径："));
    assert!(text.contains("T-daily-000-A-private-contact"));
    assert!(!text.contains("outside"));
    assert!(!text.contains("excluded-latest-waiting"));
    assert!(!daily.to_string().contains("private-note-not-in-daily"));
    let mut changed = task("changed-after-snapshot", "A");
    changed.id = first;
    changed.contacts = vec!["new-contact".into()];
    changed.contact = "new-contact".into();
    db.save_task(changed).unwrap();
    let mut frozen = args.clone();
    frozen["snapshot"] = daily["meta"]["snapshotId"].clone();
    let frozen =
        in_line_lib::mcp::service::execute(&f.security, &db, &f.credentials, "cli_report", frozen)
            .unwrap();
    assert_eq!(frozen["text"], daily["text"]);
    let output = launch(
        &f.exe,
        &f.security_root(),
        &[
            "report",
            "daily",
            "--date",
            "2026-10-22",
            "--timezone-offset-minutes",
            "480",
            "--render",
            "text",
            "--profile",
            "chosen",
        ],
        None,
        None,
    )
    .await;
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("changed-after-snapshot"));
    assert!(text.contains("new-contact"));
    assert!(!text.contains(&f.credentials.token));
    let p = Permissions {
        regular_read: true,
        full_read: false,
        write: true,
    };
    f.security.set_groups(p).unwrap();
    let output = launch(
        &f.exe,
        &f.security_root(),
        &[
            "report",
            "daily",
            "--date",
            "2026-10-22",
            "--render",
            "text",
            "--profile",
            "chosen",
        ],
        None,
        None,
    )
    .await;
    assert_eq!(output.status.code(), Some(3));
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("private-contact"));
    let (code, v) = run(
        &f,
        &["report", "daily", "--date", "2026-10-22"],
        None,
        false,
    )
    .await;
    assert_eq!(code, 0, "{v}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn phase5_batch_basis_race_rejects_changed_preview_without_retry() {
    let f = fixture(false);
    let db = Arc::new(Database::open_at(f.root.join("race.db")).unwrap());
    let t = db.save_task(task("race", "A")).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let n = count.clone();
    let security = f.security.clone();
    let d = db.clone();
    in_line_lib::mcp::ipc::start(
        f.security.clone(),
        Arc::new(move |c, tool, args| {
            if tool == "batch_mutate" {
                n.fetch_add(1, Ordering::SeqCst);
            }
            let result = in_line_lib::mcp::service::execute(&security, &d, c, tool, args.clone());
            if tool == "batch_mutate" && args["action"] == "dryRun" {
                d.set_status(t.id, "processing".into()).unwrap();
            }
            result
        }),
    )
    .unwrap();
    let mut input = body("race-batch");
    input["operations"] = json!([{ "operation":{"action":"setStatus","target":{"taskId":t.id},"taskVersion":db.mcp_task_basis(t.id,true).unwrap()["taskVersion"],"status":"cancelled"}}]);
    let (code, v) = run(&f, &["batch", "execute"], Some(input), true).await;
    assert_eq!(code, 5, "{v}");
    assert_eq!(v["error"]["code"], "conflict");
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert_eq!(db.get_task(t.id).unwrap().status, "processing");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn phase5_batch_ambiguous_committed_response_preserves_key_and_only_reads_receipt() {
    let f = fixture(false);
    let db = Arc::new(Database::open_at(f.root.join("unknown.db")).unwrap());
    let t = db.save_task(task("unknown", "A")).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let n = count.clone();
    let security = f.security.clone();
    let d = db.clone();
    in_line_lib::mcp::ipc::start(
        f.security.clone(),
        Arc::new(move |c, tool, args| {
            if tool == "batch_mutate" {
                n.fetch_add(1, Ordering::SeqCst);
            }
            let result = in_line_lib::mcp::service::execute(&security, &d, c, tool, args.clone());
            if tool == "batch_mutate" && args["action"] == "apply" && result.is_ok() {
                return Err(in_line_lib::mcp::contract::McpError::new("result_unknown"));
            }
            result
        }),
    )
    .unwrap();
    let mut input = body("unknown-batch");
    input["operations"] = json!([{ "operation":{"action":"setStatus","target":{"taskId":t.id},"taskVersion":db.mcp_task_basis(t.id,true).unwrap()["taskVersion"],"status":"cancelled"}}]);
    let (code, v) = run(&f, &["batch", "execute"], Some(input), true).await;
    assert_eq!(code, 9, "{v}");
    assert_eq!(v["error"]["cliReceipt"]["idempotencyKey"], "unknown-batch");
    assert_eq!(count.load(Ordering::SeqCst), 2);
    assert_eq!(db.get_task(t.id).unwrap().status, "cancelled");
    let (code, v) = run(
        &f,
        &["receipt", "get", "--idempotency-key", "unknown-batch"],
        None,
        false,
    )
    .await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["receipt"]["commitStatus"], "committed");
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn phase5_actual_mcp_and_cli_batch_share_future_numbers_transactions_and_audit_rules() {
    use rmcp::model::CallToolRequestParams;
    let f = fixture(false);
    let other = fixture(false);
    let db = Arc::new(Database::open_at(f.root.join("same.db")).unwrap());
    let remote = Arc::new(Database::open_at(other.root.join("same.db")).unwrap());
    let a = db.save_task(task("same", "A")).unwrap();
    let b = remote.save_task(task("same", "A")).unwrap();
    assert_eq!(a.id, b.id);
    host(&f, db.clone());
    host(&other, remote.clone());
    let issued = other
        .security
        .grant(Grant {
            name: "MCP independent batch".into(),
            permissions: Permissions {
                regular_read: true,
                full_read: false,
                write: true,
            },
            scope: Scope {
                departments: Some(vec!["A".into()]),
                task_types: None,
            },
        })
        .unwrap();
    let credentials = Credentials {
        client_id: issued.client_id,
        token: issued.token,
    };
    let client = super::phase4::mcp_client(&other, &credentials).await;
    let mut input = body("same-batch");
    input["operations"] = json!([
        {"operation":{"action":"setStatus","target":{"taskId":a.id},"taskVersion":db.mcp_task_basis(a.id,true).unwrap()["taskVersion"],"status":"processing"}},
        {"refKey":"future","operation":{"action":"create","task":{"title":"same-future","plannedDate":"2030-01-03","departments":["A"],"contacts":["private-contact"],"taskType":"T","details":"","internalNotes":""}}}
    ]);
    let (code, v) = run(&f, &["batch", "execute"], Some(input.clone()), true).await;
    assert_eq!(code, 0, "{v}");
    input["action"] = json!("dryRun");
    let preview = client
        .call_tool(
            CallToolRequestParams::new("batch_mutate")
                .with_arguments(input.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(preview["status"], "ok", "{preview}");
    input["expectedBasis"] = preview["data"]["basis"].clone();
    input["action"] = json!("apply");
    let applied = client
        .call_tool(
            CallToolRequestParams::new("batch_mutate")
                .with_arguments(input.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(applied["status"], "ok", "{applied}");
    assert_eq!(
        applied["data"]["verificationStatus"],
        "verified_after_commit"
    );
    assert_eq!(
        snapshot(&f.root.join("same.db"))["daily_sequences"],
        snapshot(&other.root.join("same.db"))["daily_sequences"]
    );
    assert_eq!(
        super::phase4::business_snapshot(&f.root.join("same.db")),
        super::phase4::business_snapshot(&other.root.join("same.db"))
    );
    assert!(
        db.list_work_events(2).unwrap().is_empty(),
        "future reservation never creates actual work"
    );
    client.cancel().await.unwrap();
}
