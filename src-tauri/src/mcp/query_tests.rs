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
