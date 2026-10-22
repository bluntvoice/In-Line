//! Actual CLI writers, synthetic protected host, and actual MCP stdio comparison.
use super::*;
use in_line_lib::mcp::security::{Permissions, Scope};
use rmcp::{model::CallToolRequestParams, transport::TokioChildProcess, ServiceExt};
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) async fn mcp_client(
    f: &Fixture,
    c: &Credentials,
) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_in-line-mcp"));
    command
        .env("IN_LINE_MCP_TEST_SECURITY_ROOT", f.security_root())
        .env(
            "IN_LINE_MCP_TEST_HOST_EXE",
            std::env::current_exe().unwrap(),
        )
        .env("IN_LINE_MCP_CLIENT_ID", &c.client_id)
        .env("IN_LINE_MCP_TOKEN", &c.token)
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000)
        .kill_on_drop(true);
    ().serve(TokioChildProcess::new(command).unwrap())
        .await
        .unwrap()
}
async fn mcp_write(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    args: Value,
) -> Value {
    let result = client
        .call_tool(
            CallToolRequestParams::new("mutate_task")
                .with_arguments(args.as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    result.structured_content.unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_and_actual_mcp_concurrent_versions_conflict_and_profiles_cannot_expand_scope() {
    let f = fixture();
    let db = Arc::new(Database::open_at(f.root.join("synthetic.db")).unwrap());
    let row = db.save_task(task("race", "A")).unwrap();
    let outside = db.save_task(task("outside", "B")).unwrap();
    let p = Permissions {
        regular_read: true,
        full_read: false,
        write: true,
    };
    let scope = Scope {
        departments: Some(vec!["A".into()]),
        task_types: None,
    };
    f.security
        .update_client(&f.credentials.client_id, p.clone(), scope.clone())
        .unwrap();
    let issued = f
        .security
        .grant(Grant {
            name: "independent MCP racer".into(),
            permissions: p,
            scope,
        })
        .unwrap();
    let credentials = Credentials {
        client_id: issued.client_id,
        token: issued.token,
    };
    host(&f, db.clone());
    let client = mcp_client(&f, &credentials).await;
    let original = version(&db, row.id);
    let mut a = body("cli-race");
    a["taskVersion"] = original.clone();
    let mut b = body("mcp-race");
    b["taskVersion"] = original;
    b["action"] = json!("setStatus");
    b["target"] = json!({"taskId":row.id});
    b["status"] = json!("processing");
    let id = row.id.to_string();
    let command = ["task", "status", "set", &id, "cancelled"];
    let ((code, local), remote) = tokio::join!(write(&f, &command, a), mcp_write(&client, b));
    if code == 0 {
        assert_eq!(remote["error"]["code"], "conflict", "{remote}");
        assert_eq!(db.get_task(row.id).unwrap().status, "cancelled");
    } else {
        assert_eq!(code, 5, "{local}");
        assert_eq!(local["error"]["code"], "conflict");
        assert_eq!(remote["status"], "ok", "{remote}");
        assert_eq!(db.get_task(row.id).unwrap().status, "processing");
    }
    let audits = db.mcp_audit_state().unwrap();
    assert_eq!(
        audits["audits"].as_array().unwrap().len(),
        1,
        "only the winner is audited and receipted"
    );
    let outside_id = outside.id.to_string();
    let mut denied = body("outside");
    denied["taskVersion"] = version(&db, outside.id);
    let (code, v) = write(&f, &["task", "trash", &outside_id], denied).await;
    assert_eq!(code, 4, "{v}");
    assert!(db.get_task(outside.id).unwrap().deleted_at.is_none());
    let (code, v) = read(&f, &["task", "parent", "candidates", &outside_id]).await;
    assert_eq!(code, 4, "{v}");
    let mut request = body("missing-profile");
    request["taskVersion"] = version(&db, row.id);
    let (code, v) = f
        .run(
            &[
                "task",
                "trash",
                &id,
                "--execute",
                "--profile",
                "missing",
                "--input",
                "-",
                "--format",
                "json",
            ],
            Some(&serde_json::to_vec(&request).unwrap()),
        )
        .await;
    assert_eq!(code, 3, "{v}");
    assert_eq!(v["error"]["code"], "unauthenticated");
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn parent_candidates_over_one_hundred_are_scoped_snapshot_pages_not_truncated() {
    let f = fixture();
    let db = Arc::new(Database::open_at(f.root.join("synthetic.db")).unwrap());
    let source = db.save_task(task("source", "A")).unwrap();
    for i in 0..105 {
        db.save_task(task(&format!("candidate {i}"), "A")).unwrap();
    }
    db.save_task(task("hidden candidate", "B")).unwrap();
    f.security
        .update_client(
            &f.credentials.client_id,
            Permissions {
                regular_read: true,
                full_read: false,
                write: true,
            },
            Scope {
                departments: Some(vec!["A".into()]),
                task_types: None,
            },
        )
        .unwrap();
    host(&f, db.clone());
    let id = source.id.to_string();
    let (code, first) = read(&f, &["task", "parent", "candidates", &id, "--limit", "100"]).await;
    assert_eq!(code, 0, "{first}");
    assert_eq!(first["data"]["total"], 105);
    assert_eq!(first["data"]["items"].as_array().unwrap().len(), 100);
    let cursor = first["data"]["nextCursor"].as_str().unwrap();
    let (code, next) = read(
        &f,
        &[
            "task",
            "parent",
            "candidates",
            &id,
            "--limit",
            "100",
            "--cursor",
            cursor,
        ],
    )
    .await;
    assert_eq!(code, 0, "{next}");
    assert_eq!(next["data"]["items"].as_array().unwrap().len(), 5);
    assert_eq!(
        first["data"]["meta"]["snapshotId"],
        next["data"]["meta"]["snapshotId"]
    );
    let (code, all) = read(&f, &["task", "parent", "candidates", &id, "--all"]).await;
    assert_eq!(code, 0, "{all}");
    assert_eq!(all["data"]["pagination"]["itemsReturned"], 105);
    assert_eq!(all["data"]["pagination"]["complete"], true);
    for value in [&first, &next, &all] {
        assert!(!value.to_string().contains("private-"));
        assert!(!value.to_string().contains("hidden candidate"));
    }
}
pub(super) fn business_snapshot(path: &Path) -> Value {
    let c = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
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
        let mut s = c
            .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
            .unwrap();
        let names: Vec<String> = s.column_names().into_iter().map(String::from).collect();
        let data = s
            .query_map([], |row| {
                let mut value = serde_json::Map::new();
                for (i, name) in names.iter().enumerate() {
                    use rusqlite::types::ValueRef;
                    let v = row.get_ref(i)?;
                    let v = if matches!(
                        name.as_str(),
                        "created_at"
                            | "updated_at"
                            | "started_at"
                            | "completed_at"
                            | "archived_at"
                            | "deleted_at"
                            | "entered_at"
                            | "enqueued_at"
                            | "closed_at"
                            | "handled_at"
                            | "voided_at"
                    ) && !matches!(v, ValueRef::Null)
                    {
                        json!("<present timestamp>")
                    } else {
                        match v {
                            ValueRef::Null => Value::Null,
                            ValueRef::Integer(n) => json!(n),
                            ValueRef::Real(n) => json!(n),
                            ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                            ValueRef::Blob(b) => json!(format!("{:x}", Sha256::digest(b))),
                        }
                    };
                    value.insert(name.clone(), v);
                }
                Ok(Value::Object(value))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        result.insert(table.into(), json!(data));
    }
    let basis: (i64, i64) = c
        .query_row(
            "SELECT data_generation,commit_sequence FROM mcp_data_basis",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    result.insert(
        "dataBasis".into(),
        json!({"dataGeneration":basis.0,"commitSequence":basis.1}),
    );
    Value::Object(result)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn gui_cli_and_actual_mcp_stdio_share_task_queue_number_event_and_status_rules() {
    let f = fixture();
    let cli_path = f.root.join("cli.db");
    let mcp_path = f.root.join("mcp.db");
    let gui_path = f.root.join("gui.db");
    let cli_db = Arc::new(Database::open_at(cli_path.clone()).unwrap());
    let mcp_db = Arc::new(Database::open_at(mcp_path.clone()).unwrap());
    let gui_db = Database::open_at(gui_path.clone()).unwrap();
    let issued = f
        .security
        .grant(Grant {
            name: "independent MCP comparison".into(),
            permissions: Permissions {
                regular_read: true,
                full_read: false,
                write: true,
            },
            scope: Scope::default(),
        })
        .unwrap();
    let credentials = Credentials {
        client_id: issued.client_id,
        token: issued.token,
    };
    let s = f.security.clone();
    let cid = credentials.client_id.clone();
    let a = cli_db.clone();
    let b = mcp_db.clone();
    in_line_lib::mcp::ipc::start(
        s.clone(),
        Arc::new(move |c, tool, args| {
            in_line_lib::mcp::service::execute(
                &s,
                if c.client_id == cid { &b } else { &a },
                c,
                tool,
                args,
            )
        }),
    )
    .unwrap();
    let client = mcp_client(&f, &credentials).await;
    let mut input = body("create");
    input["task"] = json!({"title":"three entry points","departments":["A"],"contacts":["private-contact"],"taskType":"T","details":"private-details","internalNotes":"private-notes"});
    let (code, v) = write(&f, &["task", "create"], input.clone()).await;
    assert_eq!(code, 0, "{v}");
    input["action"] = json!("create");
    let response = mcp_write(&client, input).await;
    assert_eq!(response["status"], "ok", "{response}");
    let mut gui = task("three entry points", "A");
    gui.details = "private-details".into();
    gui.internal_notes = "private-notes".into();
    gui_db.save_task(gui).unwrap();
    assert_eq!(business_snapshot(&cli_path), business_snapshot(&mcp_path));
    assert_eq!(business_snapshot(&cli_path), business_snapshot(&gui_path));
    for (key, cli, action) in [
        (
            "status",
            vec!["task", "status", "set", "1", "processing"],
            json!({"action":"setStatus","status":"processing"}),
        ),
        (
            "round",
            vec!["work", "process-round", "1"],
            json!({"action":"queue","mode":"processRound"}),
        ),
        (
            "enter",
            vec!["queue", "enter", "1"],
            json!({"action":"queue","mode":"enqueue"}),
        ),
        (
            "complete",
            vec!["work", "complete-round", "1"],
            json!({"action":"queue","mode":"completeRound"}),
        ),
    ] {
        let mut input = body(key);
        input["taskVersion"] = version(&cli_db, 1);
        let (code, v) = write(&f, &cli, input).await;
        assert_eq!(code, 0, "{v}");
        let mut input = body(key);
        input["taskVersion"] = version(&mcp_db, 1);
        input["target"] = json!({"taskId":1});
        for (k, v) in action.as_object().unwrap() {
            input[k] = v.clone();
        }
        let response = mcp_write(&client, input).await;
        assert_eq!(response["status"], "ok", "{response}");
        match key {
            "status" => gui_db.set_status(1, "processing".into()).unwrap(),
            "round" => gui_db.process_round(1).unwrap(),
            "enter" => gui_db
                .enqueue_task(
                    serde_json::from_value(
                        json!({"id":1,"inheritDeadline":false,"reason":"synthetic user request"}),
                    )
                    .unwrap(),
                )
                .unwrap(),
            _ => gui_db.complete_round(1).unwrap(),
        }
        assert_eq!(
            business_snapshot(&cli_path),
            business_snapshot(&mcp_path),
            "CLI versus actual stdio after {key}"
        );
        assert_eq!(
            business_snapshot(&cli_path),
            business_snapshot(&gui_path),
            "CLI versus GUI domain after {key}"
        );
    }
    // A normal MCP identity cannot use the private CLI routes even over IPC.
    for tool in [
        "cli_get_receipt",
        "cli_parent_candidates",
        "cli_complete_task",
        "cli_create_subtask",
    ] {
        let error =
            in_line_lib::mcp::service::execute(&f.security, &mcp_db, &credentials, tool, json!({}))
                .unwrap_err();
        assert_eq!(error.code, "forbidden");
    }
    client.cancel().await.unwrap();
}
fn fixture() -> Fixture {
    let f = Fixture::new();
    let p = Permissions {
        regular_read: true,
        full_read: false,
        write: true,
    };
    f.security.set_groups(p.clone()).unwrap();
    f.security
        .update_client(&f.credentials.client_id, p, Default::default())
        .unwrap();
    f.save("chosen", &f.profile());
    f
}
fn intent() -> Value {
    json!({"summary":"synthetic explicit user work request","explicitUserRequest":true,"confirmedRealWork":true})
}
fn body(key: &str) -> Value {
    json!({"idempotencyKey":key,"intent":intent(),"reason":"synthetic user request"})
}
fn version(db: &Database, id: i64) -> Value {
    db.mcp_task_basis(id, true).unwrap()["taskVersion"].clone()
}
async fn write(f: &Fixture, command: &[&str], input: Value) -> (i32, Value) {
    let mut args = command.to_vec();
    args.extend([
        "--execute",
        "--profile",
        "chosen",
        "--input",
        "-",
        "--format",
        "json",
    ]);
    f.run(&args, Some(&serde_json::to_vec(&input).unwrap()))
        .await
}
fn new_task(title: &str) -> Value {
    json!({"title":title,"departments":["A"],"contacts":["synthetic-contact"],"taskType":"T","details":"","internalNotes":""})
}
async fn read(f: &Fixture, cmd: &[&str]) -> (i32, Value) {
    let mut args = cmd.to_vec();
    args.extend(["--profile", "chosen", "--format", "json"]);
    f.run(&args, None).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_unknown_write_is_not_replayed_and_readonly_original_receipt_recovers_ledger() {
    let f = fixture();
    let db = Arc::new(Database::open_at(f.root.join("synthetic.db")).unwrap());
    let s = f.security.clone();
    let d = db.clone();
    let writes = Arc::new(AtomicUsize::new(0));
    let counter = writes.clone();
    in_line_lib::mcp::ipc::start(
        s.clone(),
        Arc::new(move |c, tool, args| {
            let lose = tool == "mutate_task"
                && counter.fetch_add(usize::from(tool == "mutate_task"), Ordering::SeqCst) == 0;
            let fail_verification = args["reason"] == "synthetic verification failure";
            let result = in_line_lib::mcp::service::execute(&s, &d, c, tool, args);
            let result = result.map(|mut v| {
                if fail_verification && tool == "mutate_task" {
                    v["verificationStatus"] = json!("verification_failed");
                }
                v
            });
            if lose && result.is_ok() {
                Err(McpError::new("host_unavailable"))
            } else {
                result
            }
        }),
    )
    .unwrap();
    let mut input = body("original");
    input["task"] = new_task("unknown response");
    let (code, v) = write(&f, &["task", "create"], input.clone()).await;
    assert_eq!(code, 9, "{v}");
    assert_eq!(v["error"]["code"], "result_unknown");
    assert_eq!(v["error"]["cliReceipt"]["idempotencyKey"], "original");
    assert_eq!(writes.load(Ordering::SeqCst), 1);
    let (code, v) = read(&f, &["receipt", "get", "--idempotency-key", "original"]).await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["readOnly"], true);
    assert_eq!(v["data"]["receipt"]["commitStatus"], "committed");
    let (code, v) = read(&f, &["receipt", "get", "--idempotency-key", "absent"]).await;
    assert_eq!(code, 4, "{v}");
    assert_eq!(writes.load(Ordering::SeqCst), 1);
    assert_eq!(
        db.list_tasks(in_line_lib::models::TaskView::Queue)
            .unwrap()
            .len(),
        1
    );
    // A later explicit user execution with the same key is an idempotent replay,
    // not an automatic retry by the previous invocation. No duplicate preflight blocks it.
    let (code, v) = write(&f, &["task", "create"], input.clone()).await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["replayed"], true);
    assert_eq!(
        db.list_tasks(in_line_lib::models::TaskView::Queue)
            .unwrap()
            .len(),
        1
    );
    input["task"]["title"] = json!("different payload");
    let (code, v) = write(&f, &["task", "create"], input).await;
    assert_eq!(code, 5, "{v}");
    assert_eq!(v["error"]["code"], "idempotency_conflict");
    let mut failed = body("failed-verification");
    failed["task"] = new_task("committed but unverified");
    failed["reason"] = json!("synthetic verification failure");
    let (code, v) = write(&f, &["task", "create"], failed).await;
    assert_eq!(code, 9, "{v}");
    assert_eq!(v["data"]["commitStatus"], "committed");
    assert_eq!(v["data"]["verificationStatus"], "verification_failed");
    let before = writes.load(Ordering::SeqCst);
    let (code, v) = read(
        &f,
        &["receipt", "get", "--idempotency-key", "failed-verification"],
    )
    .await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(writes.load(Ordering::SeqCst), before);
    let mut denied = body("denied");
    denied["task"] = new_task("must not create");
    f.security.set_groups(Permissions::default()).unwrap();
    let before = writes.load(Ordering::SeqCst);
    let (code, v) = write(&f, &["task", "create"], denied).await;
    assert_eq!(code, 3, "{v}");
    assert_eq!(writes.load(Ordering::SeqCst), before);
    for args in [
        vec!["task", "create"],
        vec!["task", "trash", "1", "--force"],
        vec!["undo", "approve", "1", "--execute"],
    ] {
        let mut args = args;
        args.extend(["--profile", "chosen", "--format", "json"]);
        let (code, v) = f.run(&args, None).await;
        assert_eq!(code, 2, "{v}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_real_parent_children_future_scope_completion_and_local_undo_approval() {
    let f = fixture();
    let db = Arc::new(Database::open_at(f.root.join("synthetic.db")).unwrap());
    let parent = db.save_task(task("parent", "A")).unwrap();
    host(&f, db.clone());
    let p = parent.id.to_string();
    let mut b = body("child");
    b["parentVersion"] = version(&db, parent.id);
    b["task"] = json!({"title":"child","plannedDate":"2099-01-01"});
    let (code, v) = write(&f, &["task", "child", "create", &p], b).await;
    assert_eq!(code, 0, "{v}");
    let child = v["data"]["taskId"].as_i64().unwrap();
    let child_id = child.to_string();
    assert!(db.get_task(child).unwrap().is_scheduled);
    let mut b = body("date");
    b["taskVersion"] = version(&db, child);
    b["plannedDate"] = json!(chrono::Local::now().format("%Y-%m-%d").to_string());
    let (code, v) = write(&f, &["task", "reschedule", &child_id], b).await;
    assert_eq!(code, 0, "{v}");
    let mut b = body("clear-parent");
    b["taskVersion"] = version(&db, child);
    let (code, v) = write(&f, &["task", "parent", "clear", &child_id], b).await;
    assert_eq!(code, 0, "{v}");
    let (code, v) = read(&f, &["task", "parent", "candidates", &child_id]).await;
    assert_eq!(code, 0, "{v}");
    assert!(v["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["id"] == parent.id));
    assert!(!v.to_string().contains("private-"));
    let mut b = body("set-parent");
    b["taskVersion"] = version(&db, child);
    b["parentVersion"] = version(&db, parent.id);
    let (code, v) = write(&f, &["task", "parent", "set", &child_id, &p], b).await;
    assert_eq!(code, 0, "{v}");
    let mut b = body("reorder");
    b["parentVersion"] = version(&db, parent.id);
    b["taskIds"] = json!([child]);
    b["taskVersions"] = json!({child_id.clone():version(&db,child)});
    let (code, v) = write(&f, &["task", "children", "reorder", &p], b).await;
    assert_eq!(code, 0, "{v}");
    let mut b = body("plain-parent-complete");
    b["taskVersion"] = version(&db, parent.id);
    let (code, v) = write(&f, &["task", "complete", &p], b).await;
    assert_eq!(code, 5, "{v}");
    assert_eq!(db.get_task(child).unwrap().status, "pending");
    let mut b = body("group-complete");
    b["taskVersion"] = version(&db, parent.id);
    b["taskVersions"] = json!({child_id.clone():version(&db,child)});
    b["includeEligibleSubtasks"] = json!(true);
    let (code, v) = write(&f, &["task", "complete", &p], b).await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(db.get_task(child).unwrap().status, "completed");
    let audit = v["data"]["auditId"].clone();
    let mut b = body("undo-request");
    b["auditId"] = audit;
    let (code, v) = write(&f, &["undo", "request"], b).await;
    assert_eq!(code, 8, "{v}");
    assert_eq!(v["data"]["status"], "needs_user_approval");
    assert_eq!(db.get_task(child).unwrap().status, "completed");
    let local = db
        .mcp_resolve_undo(v["data"]["undoRequestId"].as_i64().unwrap(), true)
        .unwrap();
    assert_eq!(local["verificationStatus"], "verified_after_commit");
    assert_eq!(db.get_task(child).unwrap().status, "pending");
    let mut b = body("parent-only");
    b["taskVersion"] = version(&db, parent.id);
    b["taskVersions"] = json!({child_id.clone():version(&db,child)});
    let (code, v) = write(&f, &["task", "complete", &p, "--parent-only"], b).await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(db.get_task(parent.id).unwrap().status, "completed");
    assert_eq!(db.get_task(child).unwrap().status, "pending");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_task_edit_urgency_queue_real_work_archive_trash_restore_and_reopen_use_versions() {
    let f = fixture();
    let db = Arc::new(Database::open_at(f.root.join("synthetic.db")).unwrap());
    let row = db.save_task(task("first", "A")).unwrap();
    let id = row.id.to_string();
    host(&f, db.clone());
    let mut b = body("patch");
    let basis = db.mcp_task_basis(row.id, true).unwrap();
    b["fieldBase"] = json!({"title":{"value":"first","version":basis["fieldVersions"]["title"]}});
    b["patch"] = json!({"title":"renamed"});
    let (code, v) = write(&f, &["task", "update", &id], b).await;
    assert_eq!(code, 0, "{v}");
    for (n, cmd, extra) in [
        (
            "urgent",
            vec!["task", "urgent", "set", &id],
            json!({"requester":"user","urgentReason":"explicit urgent"}),
        ),
        ("clear", vec!["task", "urgent", "clear", &id], json!({})),
        (
            "status",
            vec!["task", "status", "set", &id, "processing"],
            json!({}),
        ),
        ("round", vec!["work", "process-round", &id], json!({})),
        ("enter", vec!["queue", "enter", &id], json!({})),
        ("move", vec!["queue", "move", &id, "up"], json!({})),
        (
            "complete-round",
            vec!["work", "complete-round", &id],
            json!({}),
        ),
        ("archive", vec!["task", "archive", &id], json!({})),
        ("reopen", vec!["task", "reopen", &id], json!({})),
        ("trash", vec!["task", "trash", &id], json!({})),
        ("restore", vec!["task", "restore", &id], json!({})),
    ] {
        let mut b = body(n);
        b["taskVersion"] = version(&db, row.id);
        for (k, v) in extra.as_object().unwrap() {
            b[k] = v.clone();
        }
        let (code, v) = write(&f, &cmd, b).await;
        assert_eq!(code, 0, "{cmd:?}: {v}");
    }
    let restored = db.get_task(row.id).unwrap();
    assert!(restored.deleted_at.is_none());
    assert!(restored.has_active_queue);
    assert_eq!(restored.title, "renamed");
    let mut b = body("record");
    b["taskVersion"] = version(&db, row.id);
    b["resultStatus"] = json!("processed");
    b["handledAt"] = json!("2026-08-08T09:00:00+08:00");
    b["note"] = json!("private-real-note");
    let (code, v) = write(&f, &["work", "record", &id], b).await;
    assert_eq!(code, 0, "{v}");
    assert!(!v.to_string().contains("private-real-note"));
    let event = db
        .list_work_events(row.id)
        .unwrap()
        .into_iter()
        .find(|e| e.note == "private-real-note")
        .unwrap();
    let mut b = body("correct");
    b["taskVersion"] = version(&db, row.id);
    b["eventId"] = json!(event.id);
    b["resultStatus"] = json!("processed");
    b["handledAt"] = json!("2026-08-08T10:00:00+08:00");
    b["note"] = json!("corrected real note");
    b["confirmHistoricalImpact"] = json!(true);
    let (code, v) = write(&f, &["work", "correct", &id], b).await;
    assert_eq!(code, 0, "{v}");
    let event = db
        .list_work_events(row.id)
        .unwrap()
        .into_iter()
        .find(|e| e.note == "corrected real note")
        .unwrap();
    let mut b = body("void");
    b["taskVersion"] = version(&db, row.id);
    b["eventId"] = json!(event.id);
    b["confirmHistoricalImpact"] = json!(true);
    let (code, v) = write(&f, &["work", "void", &id], b).await;
    assert_eq!(code, 0, "{v}");
    let stale = 0;
    let mut b = body("stale");
    b["taskVersion"] = json!(stale);
    let (code, v) = write(&f, &["task", "trash", &id], b).await;
    assert_eq!(code, 5, "{v}");
    assert_eq!(v["error"]["code"], "conflict");
}
