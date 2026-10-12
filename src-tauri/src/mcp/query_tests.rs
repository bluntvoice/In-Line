use super::super::{
    contract::Credentials,
    security::{Grant, Permissions, Scope, Security},
    service,
};
use super::*;
use crate::models::{TaskInput, WorkEventInput};
use serde_json::{json, Value};

fn fixture() -> (Database, Security, Credentials, Credentials) {
    let root =
        std::env::temp_dir().join(format!("inline-p2-{}", platform::random_secret().unwrap()));
    std::fs::create_dir_all(&root).unwrap();
    let db = Database::open_at(root.join("inline.db")).unwrap();
    let security = Security::open_at(root.join("security")).unwrap();
    let permissions = Permissions {
        regular_read: true,
        full_read: true,
        write: true,
    };
    security.set_groups(permissions.clone()).unwrap();
    let make = |name: &str, full: bool| {
        let issued = security
            .grant(Grant {
                name: name.into(),
                permissions: Permissions {
                    full_read: full,
                    ..permissions.clone()
                },
                scope: Scope {
                    departments: Some(vec!["A".into()]),
                    task_types: Some(vec!["T".into()]),
                },
            })
            .unwrap();
        Credentials {
            client_id: issued.client_id,
            token: issued.token,
        }
    };
    let a = make("regular", false);
    let b = make("full", true);
    (db, security, a, b)
}
fn input(title: &str, dept: &str) -> TaskInput {
    serde_json::from_value(json!({"title":title,"department":dept,"departments":[dept],"contact":"secret-contact","contacts":["secret-contact"],"taskType":"T","details":"secret-detail","internalNotes":"secret-internal","status":"pending","priority":"normal","workload":"standard","isUrgent":false,"urgentRequester":"","urgentReason":""})).unwrap()
}
fn call(s: &Security, d: &Database, c: &Credentials, tool: &str, args: Value) -> Value {
    service::execute(s, d, c, tool, args).unwrap()
}
fn err(s: &Security, d: &Database, c: &Credentials, tool: &str, args: Value) -> String {
    service::execute(s, d, c, tool, args).unwrap_err().code
}
fn event(db: &Database, id: i64, at: &str, status: &str) {
    db.record_work_event(WorkEventInput {
        task_id: id,
        result_status: status.into(),
        handled_at: at.into(),
        note: "secret-work-note".into(),
        sync_status: true,
    })
    .unwrap();
}

#[test]
fn metric_trace_all_counts_rates_dimensions_trend_and_scope_match_gui() {
    let (db, s, a, b) = fixture();
    let first = db.save_task(input("first", "A")).unwrap();
    let second = db.save_task(input("second", "A")).unwrap();
    let outside = db.save_task(input("outside", "B")).unwrap();
    event(&db, first.id, "2026-08-08T09:00:00+08:00", "processed");
    event(&db, first.id, "2026-08-08T10:00:00+08:00", "completed");
    event(
        &db,
        second.id,
        "2026-08-08T10:00:00+08:00",
        "waiting_materials",
    );
    event(&db, outside.id, "2026-08-08T10:00:00+08:00", "completed");
    db.record_work_event(WorkEventInput {
        task_id: first.id,
        result_status: "processed".into(),
        handled_at: "2026-08-08T11:00:00+08:00".into(),
        note: "secret-voided".into(),
        sync_status: false,
    })
    .unwrap();
    let voided = db
        .list_work_events(first.id)
        .unwrap()
        .into_iter()
        .find(|e| e.note == "secret-voided")
        .unwrap();
    db.void_work_event(voided.id, true).unwrap();
    let trash = db.save_task(input("excluded-trash", "A")).unwrap();
    event(&db, trash.id, "2026-08-08T11:00:00+08:00", "completed");
    db.soft_delete(trash.id).unwrap();
    for mode in ["processing", "closure"] {
        db.set_setting("statistics_rate_mode".into(), mode.into())
            .unwrap();
        let report =
            json!({"startDate":"2026-08-08","endDate":"2026-08-08","timezoneOffsetMinutes":480});
        let summary = call(&s, &db, &a, "get_report_summary", report.clone());
        let native = db
            .statistics_scoped(
                "2026-08-08T00:00:00+08:00".into(),
                "2026-08-09T00:00:00+08:00".into(),
                480,
                Scope {
                    departments: Some(vec!["A".into()]),
                    task_types: Some(vec!["T".into()]),
                },
            )
            .unwrap();
        assert_eq!(summary["statistics"], serde_json::to_value(native).unwrap());
        let mut r = report;
        r["snapshot"] = summary["meta"]["snapshotId"].clone();
        for metric in [
            "handledTasks",
            "topLevelTasks",
            "subtasks",
            "processed",
            "completed",
            "waitingMaterials",
            "waitingConfirmation",
            "waitingCounterpartyConfirmation",
            "rateNumerator",
            "rateDenominator",
            "completionRate",
            "byTaskType.handledTasks",
            "byTaskType.completed",
            "byTaskType.pendingFollowUp",
            "byDepartment.handledTasks",
            "byDepartment.completed",
            "byDepartment.pendingFollowUp",
            "trend.handledTasks",
            "trend.processed",
            "trend.completed",
        ] {
            let mut args = json!({"action":"trace","report":r,"metric":metric});
            if metric.starts_with("byTaskType.") {
                args["dimension"] = json!("T");
            }
            if metric.starts_with("byDepartment.") {
                args["dimension"] = json!("A");
            }
            if metric.starts_with("trend.") {
                args["periodStart"] = json!("2026-08-08");
            }
            let trace = call(&s, &db, &a, "inspect_data", args);
            assert_eq!(trace["complete"], true);
            assert!(!trace.to_string().contains("secret-"));
            assert!(!trace.to_string().contains("outside"));
            assert!(!trace.to_string().contains("excluded-trash"));
            for row in trace["items"].as_array().unwrap() {
                assert!(!row["validEvents"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["id"] == voided.id));
                if !row["winningEventId"].is_null() {
                    assert!(row["validEvents"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|e| e["id"] == row["winningEventId"]));
                }
            }
        }
        assert_eq!(
            err(
                &s,
                &db,
                &b,
                "inspect_data",
                json!({"action":"trace","report":r,"metric":"handledTasks"})
            ),
            "cursor_invalid"
        );
    }
}

#[test]
fn trace_pagination_is_bound_to_metric_client_and_original_snapshot() {
    let (db, s, a, b) = fixture();
    for n in 0..101 {
        let t = db.save_task(input(&format!("source-{n}"), "A")).unwrap();
        event(&db, t.id, "2026-08-08T10:00:00+08:00", "completed");
    }
    let mut r = json!({"startDate":"2026-08-08","endDate":"2026-08-08","timezoneOffsetMinutes":480,"limit":100});
    let sum = call(
        &s,
        &db,
        &a,
        "get_report_summary",
        json!({"startDate":"2026-08-08","endDate":"2026-08-08","timezoneOffsetMinutes":480}),
    );
    r["snapshot"] = sum["meta"]["snapshotId"].clone();
    let args = json!({"action":"trace","report":r,"metric":"handledTasks"});
    let first = call(&s, &db, &a, "inspect_data", args.clone());
    assert_eq!(first["items"].as_array().unwrap().len(), 100);
    let cursor = first["nextCursor"].clone();
    s.cache.lock().unwrap().clear(None);
    let mut update = input("changed-later", "A");
    update.id = Some(101);
    db.save_task(update).unwrap();
    let mut next = args.clone();
    next["cursor"] = cursor.clone();
    let second = call(&s, &db, &a, "inspect_data", next.clone());
    assert_eq!(second["items"].as_array().unwrap().len(), 1);
    assert_eq!(second["items"][0]["title"], "source-100");
    assert!(second["nextCursor"].is_null());
    let mut mismatch = next.clone();
    mismatch["metric"] = json!("completed");
    assert_eq!(err(&s, &db, &a, "inspect_data", mismatch), "cursor_invalid");
    assert_eq!(
        err(&s, &db, &b, "inspect_data", next.clone()),
        "cursor_invalid"
    );
    let before = db.mcp_basis().unwrap();
    let compare = call(
        &s,
        &db,
        &a,
        "inspect_data",
        json!({"action":"compare","report":r,"right":{"startDate":"2026-08-08","endDate":"2026-08-08","timezoneOffsetMinutes":480}}),
    );
    assert!(compare["differences"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["field"] == "dataVersion"));
    assert_eq!(before, db.mcp_basis().unwrap());
    let auth = s.store.lock().unwrap().authorize(&a).unwrap();
    s.update_client(
        &a.client_id,
        auth.permissions,
        Scope {
            departments: Some(vec!["B".into()]),
            task_types: None,
        },
    )
    .unwrap();
    assert_eq!(
        err(&s, &db, &a, "inspect_data", next),
        "authorization_changed"
    );
}

#[test]
fn report_cache_is_short_client_owned_invalidates_on_basis_auth_and_explicit_requery() {
    let (db, s, a, b) = fixture();
    let t = db.save_task(input("first", "A")).unwrap();
    event(&db, t.id, "2026-08-08T10:00:00+08:00", "completed");
    let report =
        json!({"startDate":"2026-08-08","endDate":"2026-08-08","timezoneOffsetMinutes":480});
    let first = call(&s, &db, &a, "get_report_summary", report.clone());
    assert_eq!(first["meta"]["cache"]["reused"], false);
    let reused = call(&s, &db, &a, "get_report_summary", report.clone());
    assert_eq!(reused["meta"]["cache"]["reused"], true);
    assert_eq!(
        reused["meta"]["cache"]["computedAt"],
        first["meta"]["cache"]["computedAt"]
    );
    let separate = call(&s, &db, &b, "get_report_summary", report.clone());
    assert_eq!(separate["meta"]["cache"]["reused"], false);
    let mut explicit = report.clone();
    explicit["explicitRequery"] = json!(true);
    assert_eq!(
        call(&s, &db, &a, "get_report_summary", explicit)["meta"]["cache"]["reused"],
        false
    );
    db.set_setting("statistics_rate_mode".into(), "closure".into())
        .unwrap();
    assert_eq!(
        call(&s, &db, &a, "get_report_summary", report.clone())["meta"]["cache"]["reused"],
        false
    );
    let health = call(
        &s,
        &db,
        &a,
        "inspect_data",
        json!({"action":"health","snapshot":first["meta"]["snapshotId"]}),
    );
    assert_eq!(health["health"]["healthy"], true);
    assert!(!health.to_string().contains("secret-"));
    s.pause(true).unwrap();
    assert_eq!(
        err(&s, &db, &a, "inspect_data", json!({"action":"cache"})),
        "paused"
    );
    s.pause(false).unwrap();
    assert_eq!(
        call(&s, &db, &a, "get_report_summary", report.clone())["meta"]["cache"]["reused"],
        false
    );
    s.revoke(&a.client_id).unwrap();
    assert_eq!(err(&s, &db, &a, "get_report_summary", report), "revoked");
}
#[test]
fn cache_default_timezone_is_resolved_before_identity_and_scalar_jobs_fail_without_panicking() {
    let (db, s, a, _) = fixture();
    let t = db.save_task(input("timezone", "A")).unwrap();
    event(&db, t.id, "2026-08-08T10:00:00+08:00", "completed");
    let raw = json!({"startDate":"2026-08-08","endDate":"2026-08-08"});
    let first = call(&s, &db, &a, "get_report_summary", raw.clone());
    let mut explicit = raw;
    explicit["timezoneOffsetMinutes"] = first["timezoneOffsetMinutes"].clone();
    assert_eq!(
        call(&s, &db, &a, "get_report_summary", explicit)["meta"]["cache"]["reused"],
        true
    );
    for arguments in [json!(42), json!(null), json!([]), json!("bad")] {
        assert_eq!(
            err(
                &s,
                &db,
                &a,
                "manage_jobs",
                json!({"action":"start","kind":"get_report_summary","arguments":arguments})
            ),
            "invalid_arguments"
        );
    }
    assert_eq!(
        call(&s, &db, &a, "inspect_data", json!({"action":"health"}))["health"]["healthy"],
        true
    );
    assert_eq!(
        bounded(json!({"oversized":"x".repeat(7*1024*1024)}))
            .unwrap_err()
            .code,
        "resource_limit"
    );
}

#[test]
fn cache_clear_preserves_actual_audit_snapshot_and_index_repair_invalidates_job_and_cursor() {
    let (db, s, a, _) = fixture();
    call(
        &s,
        &db,
        &a,
        "mutate_task",
        json!({"action":"create","task":{"title":"audited","departments":["A"],"contacts":["secret-contact"],"taskType":"T"},"idempotencyKey":"p6-audit","intent":{"summary":"synthetic explicit create","explicitUserRequest":true},"reason":"synthetic"}),
    );
    let audit = db.mcp_audit_state().unwrap();
    assert!(!audit["audits"].as_array().unwrap().is_empty());
    let full = call(&s, &db, &a, "query_changes", json!({"projection":["id"]}));
    let args = json!({"startDate":"2026-08-08","endDate":"2026-08-08","timezoneOffsetMinutes":480});
    let report = call(&s, &db, &a, "get_report_summary", args.clone());
    let before = db.mcp_basis().unwrap();
    s.cache.lock().unwrap().clear(None);
    assert_eq!(audit, db.mcp_audit_state().unwrap());
    assert_eq!(before, db.mcp_basis().unwrap());
    let health = call(
        &s,
        &db,
        &a,
        "inspect_data",
        json!({"action":"health","snapshot":report["meta"]["snapshotId"],"cursor":full["nextCursor"]}),
    );
    assert_eq!(health["snapshot"]["valid"], true);
    assert_eq!(health["cursor"]["valid"], true);
    let job = call(
        &s,
        &db,
        &a,
        "manage_jobs",
        json!({"action":"start","kind":"query_tasks","arguments":{"projection":["id"]}}),
    );
    let id = job["job"]["jobId"].clone();
    let mut complete = false;
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(25));
        if call(
            &s,
            &db,
            &a,
            "manage_jobs",
            json!({"action":"get","jobId":id}),
        )["state"]
            == "completed"
        {
            complete = true;
            break;
        }
    }
    assert!(complete);
    let path = s
        .queries
        .lock()
        .unwrap()
        .root
        .parent()
        .unwrap()
        .join("inline.db");
    rusqlite::Connection::open(path).unwrap().execute("DELETE FROM mcp_change_events WHERE sequence=(SELECT MIN(sequence) FROM mcp_change_events)",[]).unwrap();
    let corrupt = call(
        &s,
        &db,
        &a,
        "inspect_data",
        json!({"action":"health","cursor":full["nextCursor"]}),
    );
    assert_eq!(corrupt["health"]["healthy"], false);
    assert_eq!(corrupt["cursor"]["valid"], false);
    db.mcp_repair_derived().unwrap();
    assert_eq!(audit, db.mcp_audit_state().unwrap());
    let mut old = args;
    old["snapshot"] = report["meta"]["snapshotId"].clone();
    assert_eq!(
        err(&s, &db, &a, "get_report_summary", old),
        "snapshot_invalid"
    );
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "query_changes",
            json!({"projection":["id"],"cursor":full["nextCursor"]})
        ),
        "resync_required"
    );
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "manage_jobs",
            json!({"action":"result","jobId":id})
        ),
        "snapshot_invalid"
    );
}

#[test]
fn pages_freeze_content_and_order_over_100_items_and_concurrent_writes() {
    let (db, s, a, b) = fixture();
    for i in 0..101 {
        db.save_task(input(&format!("original-{i}"), "A")).unwrap();
    }
    db.save_task(input("outside", "B")).unwrap();
    let first = call(&s, &db, &a, "query_tasks", json!({"limit":100}));
    assert_eq!(first["total"], 101);
    assert_eq!(first["items"].as_array().unwrap().len(), 100);
    assert!(!first.to_string().contains("secret-"));
    let cursor = first["nextCursor"].clone();
    let snapshot = first["meta"]["snapshotId"].clone();
    db.save_task(input("new-after-page", "A")).unwrap();
    let mut changed = input("changed-after-page", "A");
    changed.id = Some(101);
    db.save_task(changed).unwrap();
    let second = call(
        &s,
        &db,
        &a,
        "query_tasks",
        json!({"limit":100,"cursor":cursor}),
    );
    assert_eq!(second["total"], 101);
    assert_eq!(second["items"][0]["title"], "original-100");
    assert!(second["nextCursor"].is_null());
    assert_eq!(second["meta"]["dataVersion"], first["meta"]["dataVersion"]);
    assert_eq!(
        err(
            &s,
            &db,
            &b,
            "query_tasks",
            json!({"limit":100,"cursor":cursor})
        ),
        "cursor_invalid"
    );
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "query_tasks",
            json!({"limit":50,"cursor":cursor})
        ),
        "cursor_invalid"
    );
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "query_tasks",
            json!({"limit":100,"cursor":cursor,"filters":{"text":"changed"}})
        ),
        "cursor_invalid"
    );
    assert_eq!(
        err(&s, &db, &a, "query_tasks", json!({"cursor":"tampered"})),
        "cursor_invalid"
    );
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "query_tasks",
            json!({"snapshot":snapshot,"projection":["details"]})
        ),
        "forbidden"
    );
    let latest = call(&s, &db, &a, "query_tasks", json!({}));
    assert_eq!(latest["total"], 102);
    assert!(
        latest["meta"]["dataVersion"]["commitSequence"]
            .as_u64()
            .unwrap()
            > first["meta"]["dataVersion"]["commitSequence"]
                .as_u64()
                .unwrap()
    );
}

#[test]
fn history_projection_voided_and_trash_require_full_read() {
    let (db, s, a, b) = fixture();
    let task = db.save_task(input("visible", "A")).unwrap();
    event(&db, task.id, "2025-01-02T12:00:00+08:00", "processed");
    event(&db, task.id, "2026-10-09T12:00:00+08:00", "completed");
    db.add_log(task.id, "secret-log".into()).unwrap();
    let events = db.list_work_events(task.id).unwrap();
    db.void_work_event(events[0].id, true).unwrap();
    let regular = call(&s, &db, &a, "query_task_history", json!({"taskId":task.id}));
    assert!(!regular.to_string().contains("secret-"));
    assert!(regular["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|x| x["text"].is_null() && x["kind"] != "log"));
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "query_task_history",
            json!({"taskId":task.id,"includeVoided":true})
        ),
        "forbidden"
    );
    let full = call(
        &s,
        &db,
        &b,
        "query_task_history",
        json!({"taskId":task.id,"includeVoided":true}),
    );
    assert!(full.to_string().contains("secret-log"));
    assert_eq!(
        full["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "work")
            .count(),
        2
    );
    db.soft_delete(task.id).unwrap();
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "query_tasks",
            json!({"filters":{"includeTrash":true}})
        ),
        "forbidden"
    );
    let trash = call(
        &s,
        &db,
        &b,
        "query_tasks",
        json!({"filters":{"includeTrash":true}}),
    );
    assert_eq!(trash["items"][0]["details"], "secret-detail");
    assert_eq!(
        err(&s, &db, &a, "query_task_history", json!({"taskId":9999})),
        "not_found"
    );
}

#[test]
fn report_and_calendar_share_ui_calculations_and_snapshot_with_long_history() {
    let (db, s, a, _) = fixture();
    let t = db.save_task(input("archive-included", "A")).unwrap();
    event(&db, t.id, "2024-01-02T23:30:00+08:00", "processed");
    event(&db, t.id, "2026-10-09T23:30:00+08:00", "completed");
    db.archive(t.id).unwrap();
    let outside = db.save_task(input("outside", "B")).unwrap();
    event(&db, outside.id, "2026-10-09T12:00:00+08:00", "completed");
    let args = json!({"startDate":"2024-01-01","endDate":"2026-10-09","timezoneOffsetMinutes":480});
    let summary = call(&s, &db, &a, "get_report_summary", args.clone());
    assert_eq!(summary["statistics"]["summary"]["handledTasks"], 1);
    let mut items_args = args.clone();
    items_args["snapshot"] = summary["meta"]["snapshotId"].clone();
    let items = call(&s, &db, &a, "list_report_items", items_args);
    assert_eq!(items["page"]["total"], 1);
    assert_eq!(
        items["page"]["items"][0]["workEvents"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(!items.to_string().contains("secret-work-note"));
    assert_eq!(items["meta"]["dataVersion"], summary["meta"]["dataVersion"]);
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "list_report_items",
            json!({"startDate":"2026-10-09","endDate":"2026-10-09","offset":1})
        ),
        "invalid_arguments"
    );
    let calendar = call(&s, &db, &a, "query_work_calendar", args);
    assert_eq!(calendar["summary"]["handlingRounds"], 2);
    assert_eq!(calendar["summary"]["completedTasks"], 1);
    assert!(!calendar.to_string().contains("outside"));
    let day = call(
        &s,
        &db,
        &a,
        "get_report_summary",
        json!({"startDate":"2026-10-09","endDate":"2026-10-09","timezoneOffsetMinutes":0}),
    );
    assert_eq!(day["statistics"]["summary"]["handledTasks"], 1);
}

#[test]
fn query_filters_are_bound_and_injection_has_no_sql_access() {
    let (db, s, a, _) = fixture();
    db.save_task(input("literal ' OR 1=1 --", "A")).unwrap();
    db.save_task(input("ordinary", "A")).unwrap();
    db.save_task(input("outside", "B")).unwrap();
    let q = call(
        &s,
        &db,
        &a,
        "query_tasks",
        json!({"filters":{"text":"' OR 1=1 --","departments":["A"],"taskTypes":["T"],"archive":"active","structure":"topLevel","activeQueue":true},"projection":["id","title"]}),
    );
    assert_eq!(q["total"], 1);
    assert!(q["items"][0]["details"].is_null());
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "query_tasks",
            json!({"sql":"SELECT * FROM tasks"})
        ),
        "invalid_arguments"
    );
    assert_eq!(
        err(&s, &db, &a, "query_tasks", json!({"limit":101})),
        "invalid_arguments"
    );
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "query_tasks",
            json!({"filters":{"createdFrom":"bad-date"}})
        ),
        "invalid_arguments"
    );
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "query_tasks",
            json!({"filters":{"statuses":["invented"]}})
        ),
        "invalid_arguments"
    );
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "query_tasks",
            json!({"projection":["internalNotes"]})
        ),
        "forbidden"
    );
}

#[test]
fn saved_queries_are_client_isolated_persistent_and_reauthorized() {
    let (db, s, a, b) = fixture();
    let saved = call(
        &s,
        &db,
        &b,
        "manage_saved_query",
        json!({"action":"save","name":"my-query","explicitIntent":true,"filters":{"departments":["A"]},"projection":["title","details"]}),
    );
    assert_eq!(saved["saved"], true);
    assert_eq!(
        call(&s, &db, &a, "manage_saved_query", json!({"action":"list"}))["names"],
        json!([])
    );
    assert_eq!(
        err(
            &s,
            &db,
            &b,
            "manage_saved_query",
            json!({"action":"save","name":"silent"})
        ),
        "explicit_intent_required"
    );
    let root = s.queries.lock().unwrap().root.clone();
    let restarted = Security::open_at(root.clone()).unwrap();
    assert_eq!(
        call(
            &restarted,
            &db,
            &b,
            "manage_saved_query",
            json!({"action":"get","name":"my-query"})
        )["definition"]["projection"],
        json!(["title", "details"])
    );
    restarted
        .update_client(
            &b.client_id,
            Permissions {
                regular_read: true,
                full_read: false,
                write: true,
            },
            Scope::default(),
        )
        .unwrap();
    assert_eq!(
        err(
            &restarted,
            &db,
            &b,
            "manage_saved_query",
            json!({"action":"get","name":"my-query"})
        ),
        "forbidden"
    );
    call(
        &restarted,
        &db,
        &b,
        "manage_saved_query",
        json!({"action":"delete","name":"my-query","explicitIntent":true}),
    );
    assert_eq!(
        call(
            &restarted,
            &db,
            &b,
            "manage_saved_query",
            json!({"action":"list"})
        )["names"],
        json!([])
    );
    assert!(!std::fs::read_to_string(root.join("saved-queries.json"))
        .unwrap()
        .contains(&b.token));
}

#[test]
fn live_scope_change_expiry_revocation_and_generation_invalidate_results() {
    let (db, s, a, _) = fixture();
    let t = db.save_task(input("visible", "A")).unwrap();
    let first = call(&s, &db, &a, "query_tasks", json!({}));
    let id = first["meta"]["snapshotId"].as_str().unwrap().to_owned();
    let mut changed = input("visible", "B");
    changed.id = Some(t.id);
    db.save_task(changed).unwrap();
    assert_eq!(
        err(&s, &db, &a, "query_tasks", json!({"snapshot":id})),
        "snapshot_invalid"
    );
    let mut changed = input("visible", "A");
    changed.id = Some(t.id);
    db.save_task(changed).unwrap();
    s.queries
        .lock()
        .unwrap()
        .snapshots
        .get_mut(&id)
        .unwrap()
        .created = Instant::now() - TTL;
    assert_eq!(
        err(&s, &db, &a, "query_tasks", json!({"snapshot":id})),
        "snapshot_expired"
    );
    let first = call(&s, &db, &a, "query_tasks", json!({}));
    s.pause(true).unwrap();
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "query_tasks",
            json!({"snapshot":first["meta"]["snapshotId"]})
        ),
        "paused"
    );
    s.pause(false).unwrap();
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "query_tasks",
            json!({"snapshot":first["meta"]["snapshotId"]})
        ),
        "authorization_changed"
    );
    s.revoke(&a.client_id).unwrap();
    assert_eq!(err(&s, &db, &a, "query_tasks", json!({})), "revoked");
}

#[test]
fn parent_scope_and_filtered_statistics_preserve_subtask_classification() {
    let (db, s, a, b) = fixture();
    let parent = db.save_task(input("private-parent", "B")).unwrap();
    let child=db.create_subtask(serde_json::from_value(json!({"parentTaskId":parent.id,"title":"visible-child","departments":["A"],"taskType":"T"})).unwrap()).unwrap();
    event(&db, child.id, "2026-10-09T10:00:00+08:00", "completed");
    let q = call(
        &s,
        &db,
        &a,
        "query_tasks",
        json!({"filters":{"structure":"subtask"}}),
    );
    assert_eq!(q["total"], 1);
    assert!(q["items"][0]["parentTaskId"].is_null());
    assert!(!q.to_string().contains("private-parent"));
    let full_history = call(
        &s,
        &db,
        &b,
        "query_task_history",
        json!({"taskId":child.id}),
    );
    assert!(!full_history.to_string().contains("private-parent"));
    assert!(full_history["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["kind"] == "log" && row["from"] == "relation" && row["text"].is_null()));
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "query_tasks",
            json!({"filters":{"parentTaskId":parent.id}})
        ),
        "not_found"
    );
    let stats = call(
        &s,
        &db,
        &a,
        "get_report_summary",
        json!({"startDate":"2026-10-09","endDate":"2026-10-09","timezoneOffsetMinutes":480,"filters":{"structure":"subtask"}}),
    );
    assert_eq!(stats["statistics"]["summary"]["subtasks"], 1);
    assert_eq!(stats["statistics"]["summary"]["topLevelTasks"], 0);
}

#[test]
fn resource_caps_and_database_generation_changes_fail_closed() {
    let (db, s, a, _) = fixture();
    db.save_task(input("visible", "A")).unwrap();
    let first = call(&s, &db, &a, "query_tasks", json!({}));
    let repeated = call(&s, &db, &a, "query_tasks", json!({}));
    assert_eq!(repeated["meta"]["snapshotId"], first["meta"]["snapshotId"]);
    for args in [
        json!({"projection":["id"]}),
        json!({"filters":{"text":"visible"}}),
        json!({"filters":{"statuses":["pending"]}}),
    ] {
        call(&s, &db, &a, "query_tasks", args);
    }
    assert_eq!(
        err(&s, &db, &a, "query_tasks", json!({"projection":["title"]})),
        "resource_limit"
    );
    let root = s
        .queries
        .lock()
        .unwrap()
        .root
        .parent()
        .unwrap()
        .to_path_buf();
    let conn = rusqlite::Connection::open(root.join("inline.db")).unwrap();
    conn.execute(
        "UPDATE mcp_data_basis SET data_generation=data_generation+1 WHERE singleton=1",
        [],
    )
    .unwrap();
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "query_tasks",
            json!({"snapshot":first["meta"]["snapshotId"]})
        ),
        "snapshot_invalid"
    );
}

#[test]
fn saved_definition_write_requires_effective_write_and_current_scope() {
    let (db, s, a, b) = fixture();
    s.update_client(&a.client_id, Permissions::default(), Scope::default())
        .unwrap();
    assert_eq!(
        err(
            &s,
            &db,
            &a,
            "manage_saved_query",
            json!({"action":"save","name":"denied","explicitIntent":true})
        ),
        "explicit_intent_required"
    );
    call(
        &s,
        &db,
        &b,
        "manage_saved_query",
        json!({"action":"save","name":"scope-safe","explicitIntent":true,"filters":{"departments":["B"]},"projection":["id","title"]}),
    );
    db.save_task(input("outside", "B")).unwrap();
    let def = call(
        &s,
        &db,
        &b,
        "manage_saved_query",
        json!({"action":"get","name":"scope-safe"}),
    );
    let q = call(&s, &db, &b, "query_tasks", def["definition"].clone());
    assert_eq!(q["total"], 0);
}
