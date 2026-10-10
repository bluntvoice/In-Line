use super::*;
use crate::mcp::{
    scope::Scope,
    security::{Authorization, Permissions},
    write_types::*,
};
use serde_json::{json, Value};
use std::sync::Arc;
#[test]
fn p3_postcommit_verification_failure_keeps_one_committed_result_and_original_key_recovers() {
    let (db, auth, _) = fixture();
    db.fail_mcp_verification
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let first = create(&db, &auth, "verification-test");
    assert_eq!(first["commitStatus"], "committed");
    assert_eq!(first["verificationStatus"], "verification_failed");
    assert_eq!(db.mcp_tasks().unwrap().len(), 1);
    let repeated = create(&db, &auth, "verification-test");
    assert_eq!(first["auditId"], repeated["auditId"]);
    assert_eq!(repeated["replayed"], true);
    assert_eq!(repeated["verificationStatus"], "verified_after_commit");
    assert_eq!(db.mcp_tasks().unwrap().len(), 1);
}
fn fixture() -> (Database, Authorization, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "inline-p3-{}",
        crate::mcp::platform::random_secret().unwrap()
    ));
    fs::create_dir_all(&root).unwrap();
    (
        Database::open_at(root.join("synthetic.db")).unwrap(),
        Authorization {
            permissions: Permissions {
                regular_read: true,
                full_read: false,
                write: true,
            },
            scope: Scope {
                departments: Some(vec!["D".into()]),
                task_types: Some(vec!["T".into()]),
            },
            revision: 1,
        },
        root,
    )
}
fn input(value: Value) -> MutateArgs {
    let mut v = json!({"idempotencyKey":"create","intent":{"summary":"synthetic user request","explicitUserRequest":true,"confirmedRealWork":true},"reason":"synthetic test"});
    for (k, vv) in value.as_object().unwrap() {
        v[k] = vv.clone();
    }
    serde_json::from_value(v).unwrap()
}
fn create(db: &Database, auth: &Authorization, key: &str) -> Value {
    db.mcp_mutate("a",auth,input(json!({"idempotencyKey":key,"action":"create","task":{"title":key,"departments":["D"],"contacts":["private-contact"],"taskType":"T","details":"private-text"}}))).unwrap()
}
fn patch(db: &Database, id: i64, key: &str, field: &str, new: Value) -> MutateArgs {
    let task = serde_json::to_value(db.get_task(id).unwrap()).unwrap();
    let basis = db.mcp_task_basis(id, true).unwrap();
    input(
        json!({"idempotencyKey":key,"action":"patch","target":{"taskId":id},"fieldBase":{field:{"value":task[field],"version":basis["fieldVersions"][field]}},"patch":{field:new}}),
    )
}
fn undo(db: &Database, auth: &Authorization, audit: i64) -> Value {
    db.mcp_request_undo(
        "a",
        auth,
        UndoArgs {
            audit_id: audit,
            idempotency_key: format!("undo-{audit}"),
            intent: Intent {
                summary: "synthetic undo".into(),
                explicit_user_request: true,
                replace_whole_text: false,
                confirmed_real_work: false,
                allow_possible_duplicate: false,
            },
            reason: "test undo".into(),
        },
    )
    .unwrap()
}
#[test]
fn p3_receipts_survive_reopen_concurrent_retry_and_do_not_leak() {
    let (db, auth, root) = fixture();
    let args = input(
        json!({"action":"create","task":{"title":"one","departments":["D"],"contacts":["private"],"taskType":"T","details":"secret"}}),
    );
    let first = db.mcp_mutate("a", &auth, args.clone()).unwrap();
    assert_eq!(first["verificationStatus"], "verified_after_commit");
    assert!(!first.to_string().contains("secret"));
    drop(db);
    let db = Arc::new(Database::open_at(root.join("synthetic.db")).unwrap());
    let joins = (0..4)
        .map(|_| {
            let db = db.clone();
            let auth = auth.clone();
            let args = args.clone();
            std::thread::spawn(move || db.mcp_mutate("a", &auth, args).unwrap())
        })
        .collect::<Vec<_>>();
    for j in joins {
        assert_eq!(j.join().unwrap()["auditId"], first["auditId"]);
    }
    assert_eq!(db.mcp_tasks().unwrap().len(), 1);
    assert_eq!(
        db.mcp_audit_state().unwrap()["audits"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mut changed = args;
    changed.reason = "different".into();
    assert_eq!(
        db.mcp_mutate("a", &auth, changed).unwrap_err().code,
        "idempotency_conflict"
    );
    let read = Authorization {
        permissions: Permissions::default(),
        ..auth.clone()
    };
    assert_eq!(db.mcp_mutate("a",&read,input(json!({"action":"create","task":{"title":"x","departments":["D"],"contacts":["x"],"taskType":"T"}}))).unwrap_err().code,"forbidden");
    let out = Authorization {
        scope: Scope {
            departments: Some(vec!["other".into()]),
            task_types: None,
        },
        ..auth
    };
    assert_eq!(db.mcp_mutate("a",&out,input(json!({"action":"create","task":{"title":"x","departments":["D"],"contacts":["x"],"taskType":"T"}}))).unwrap_err().code,"forbidden");
}
#[test]
fn p3_unrelated_fields_merge_same_field_and_aba_conflict() {
    let (db, auth, _) = fixture();
    let first = create(&db, &auth, "one");
    let id = first["taskId"].as_i64().unwrap();
    let title = patch(&db, id, "title", "title", json!("new title"));
    let workload = patch(&db, id, "workload", "workload", json!("simple"));
    db.mcp_mutate("a", &auth, title.clone()).unwrap();
    db.mcp_mutate("b", &auth, workload).unwrap();
    let mut stale = title;
    stale.idempotency_key = "stale".into();
    assert_eq!(
        db.mcp_mutate("a", &auth, stale).unwrap_err().code,
        "conflict"
    );
    let stale = patch(&db, id, "aba-stale", "title", json!("future"));
    let change = patch(&db, id, "aba1", "title", json!("temp"));
    db.mcp_mutate("a", &auth, change).unwrap();
    let change = patch(&db, id, "aba2", "title", json!("new title"));
    db.mcp_mutate("a", &auth, change).unwrap();
    assert_eq!(
        db.mcp_mutate("a", &auth, stale).unwrap_err().code,
        "conflict"
    );
    assert_eq!(db.get_task(id).unwrap().workload, "simple");
}
#[test]
fn p3_atomic_failure_rolls_back_business_dictionary_versions_and_receipt() {
    let (db, auth, _) = fixture();
    let basis = db.mcp_basis().unwrap();
    db.with_conn(|c|c.execute_batch("CREATE TRIGGER fail_ai_audit BEFORE INSERT ON mcp_ai_audit BEGIN SELECT RAISE(ABORT,'injected audit failure'); END;").map_err(display_error)).unwrap();
    assert!(db.mcp_mutate("a",&auth,input(json!({"action":"create","task":{"title":"one","departments":["D"],"contacts":["new private"],"taskType":"T"}}))).is_err());
    assert!(db.mcp_tasks().unwrap().is_empty());
    assert_eq!(db.mcp_basis().unwrap(), basis);
    assert!(db.masters().unwrap().contacts.is_empty());
    assert_eq!(
        db.with_conn(|c| c
            .query_row("SELECT count(*) FROM mcp_receipts", [], |r| r
                .get::<_, i64>(0))
            .map_err(display_error))
            .unwrap(),
        0
    );
}
#[test]
fn p3_status_no_change_and_gui_share_domain_rules() {
    let (db, auth, _) = fixture();
    let first = create(&db, &auth, "one");
    let id = first["taskId"].as_i64().unwrap();
    let v = db.mcp_task_basis(id, true).unwrap()["taskVersion"]
        .as_i64()
        .unwrap();
    let basis = db.mcp_basis().unwrap();
    let skipped=db.mcp_mutate("a",&auth,input(json!({"idempotencyKey":"nochange","action":"setStatus","target":{"taskId":id},"taskVersion":v,"status":"pending"}))).unwrap();
    assert_eq!(skipped["commitStatus"], "no_change");
    assert_eq!(db.mcp_basis().unwrap(), basis);
    assert!(db.list_work_events(id).unwrap().is_empty());
    let result=db.mcp_mutate("a",&auth,input(json!({"idempotencyKey":"work","action":"setStatus","target":{"taskId":id},"taskVersion":v,"status":"processed"}))).unwrap();
    assert_eq!(db.list_work_events(id).unwrap().len(), 1);
    assert!(!db.get_task(id).unwrap().has_active_queue);
    let request = undo(&db, &auth, result["auditId"].as_i64().unwrap());
    assert_eq!(db.get_task(id).unwrap().status, "processed");
    db.mcp_resolve_undo(request["undoRequestId"].as_i64().unwrap(), true)
        .unwrap();
    assert_eq!(db.get_task(id).unwrap().status, "pending");
    assert!(db.get_task(id).unwrap().has_active_queue);
    assert!(db.list_work_events(id).unwrap().is_empty());
    let count = db
        .with_conn(|c| {
            c.query_row(
                "SELECT count(*) FROM task_work_events WHERE task_id=? AND voided_at IS NOT NULL",
                [id],
                |r| r.get::<_, i64>(0),
            )
            .map_err(display_error)
        })
        .unwrap();
    assert!(count >= 1);
}
#[test]
fn p3_patch_undo_is_gui_only_preserves_unrelated_and_rejects_conflict() {
    let (db, auth, _) = fixture();
    let first = create(&db, &auth, "one");
    let id = first["taskId"].as_i64().unwrap();
    let args = patch(&db, id, "title", "title", json!("new title"));
    let result = db.mcp_mutate("a", &auth, args).unwrap();
    let req = undo(&db, &auth, result["auditId"].as_i64().unwrap());
    let work = patch(&db, id, "workload", "workload", json!("simple"));
    db.mcp_mutate("b", &auth, work).unwrap();
    db.mcp_resolve_undo(req["undoRequestId"].as_i64().unwrap(), true)
        .unwrap();
    assert_eq!(db.get_task(id).unwrap().title, "one");
    assert_eq!(db.get_task(id).unwrap().workload, "simple");
    let args = patch(&db, id, "title2", "title", json!("second"));
    let result = db.mcp_mutate("a", &auth, args).unwrap();
    let req = undo(&db, &auth, result["auditId"].as_i64().unwrap());
    let args = patch(&db, id, "title3", "title", json!("third"));
    db.mcp_mutate("b", &auth, args).unwrap();
    let basis = db.mcp_basis().unwrap();
    assert_eq!(
        db.mcp_resolve_undo(req["undoRequestId"].as_i64().unwrap(), true)
            .unwrap_err()
            .code,
        "conflict"
    );
    assert_eq!(db.mcp_basis().unwrap(), basis);
    assert_eq!(db.get_task(id).unwrap().title, "third");
}
#[test]
fn p3_duplicates_disambiguation_whole_text_and_scope_are_explicit() {
    let (db, auth, _) = fixture();
    let first = create(&db, &auth, "one");
    let id = first["taskId"].as_i64().unwrap();
    let mut duplicate = input(
        json!({"idempotencyKey":"duplicate","action":"create","task":{"title":"one","departments":["D"],"contacts":["private"],"taskType":"T"}}),
    );
    assert_eq!(
        db.mcp_mutate("a", &auth, duplicate.clone())
            .unwrap_err()
            .code,
        "possible_duplicate"
    );
    duplicate.intent.allow_possible_duplicate = true;
    db.mcp_mutate("a", &auth, duplicate).unwrap();
    let mut args = patch(&db, id, "title", "title", json!("new"));
    if let TaskAction::Patch { target, .. } = &mut args.operation {
        target.task_id = None;
        target.title = Some("one".into());
    }
    assert_eq!(
        db.mcp_mutate("a", &auth, args).unwrap_err().code,
        "needs_disambiguation"
    );
    let replace = patch(&db, id, "text", "details", json!("replaced"));
    assert_eq!(
        db.mcp_mutate("a", &auth, replace).unwrap_err().code,
        "explicit_intent_required"
    );
    let mut args = patch(&db, id, "fragment", "details", json!("unused"));
    if let TaskAction::Patch {
        patch, text_edits, ..
    } = &mut args.operation
    {
        patch.clear();
        text_edits.push(TextEdit {
            field: "details".into(),
            find: "private".into(),
            replace: "updated".into(),
        });
    }
    db.mcp_mutate("a", &auth, args).unwrap();
    assert_eq!(db.get_task(id).unwrap().details, "updated-text");
    let args = patch(&db, id, "scope", "departments", json!(["outside"]));
    assert_eq!(
        db.mcp_mutate("a", &auth, args).unwrap_err().code,
        "forbidden"
    );
    assert!(serde_json::from_value::<MutateArgs>(
        json!({"action":"permanentlyDelete","idempotencyKey":"x"})
    )
    .is_err());
}
#[test]
fn p3_preference_whitelist_atomic_and_user_approved_undo() {
    let (db, auth, _) = fixture();
    let args:PreferenceArgs=serde_json::from_value(json!({"action":"patch","patch":{"ui_scale":"120"},"fieldBase":{"ui_scale":null},"fieldVersions":{"ui_scale":0},"idempotencyKey":"pref","intent":{"summary":"user changes scale","explicitUserRequest":true},"reason":"synthetic"})).unwrap();
    let result = db.mcp_preferences("a", &auth, args).unwrap();
    let req = undo(&db, &auth, result["auditId"].as_i64().unwrap());
    db.mcp_resolve_undo(req["undoRequestId"].as_i64().unwrap(), true)
        .unwrap();
    assert!(!db.settings().unwrap().contains_key("ui_scale"));
    let args:PreferenceArgs=serde_json::from_value(json!({"action":"patch","patch":{"launch_at_login":"true","ui_scale":"140"},"fieldBase":{"launch_at_login":null,"ui_scale":null},"fieldVersions":{"launch_at_login":0,"ui_scale":2},"idempotencyKey":"bad","intent":{"summary":"bad","explicitUserRequest":true},"reason":"synthetic"})).unwrap();
    assert_eq!(
        db.mcp_preferences("a", &auth, args).unwrap_err().code,
        "invalid_arguments"
    );
    assert!(!db.settings().unwrap().contains_key("ui_scale"));
}

#[test]
fn p3_gui_changes_invalidate_versions_urgent_neighbour_scope_and_undo_are_checked() {
    let (db, auth, _) = fixture();
    let first = create(&db, &auth, "first");
    let second = create(&db, &auth, "second");
    let id = second["taskId"].as_i64().unwrap();
    let first_id = first["taskId"].as_i64().unwrap();
    let mut other = db.get_task(first_id).unwrap();
    other.departments = vec!["outside".into()];
    other.department = "outside".into();
    db.save_task(serde_json::from_value(serde_json::to_value(other).unwrap()).unwrap())
        .unwrap();
    let v = db.mcp_task_basis(id, true).unwrap()["taskVersion"]
        .as_i64()
        .unwrap();
    let args = input(
        json!({"idempotencyKey":"urgent","action":"setUrgent","target":{"taskId":id},"taskVersion":v,"isUrgent":true,"requester":"real user","urgentReason":"actual deadline"}),
    );
    let basis = db.mcp_basis().unwrap();
    assert_eq!(
        db.mcp_mutate("a", &auth, args.clone()).unwrap_err().code,
        "forbidden"
    );
    assert_eq!(db.mcp_basis().unwrap(), basis);
    assert!(!db.get_task(id).unwrap().is_urgent);
    let broad = Authorization {
        scope: Scope::default(),
        ..auth.clone()
    };
    let old_order = db.get_task(id).unwrap().custom_sort_order;
    let result = db.mcp_mutate("a", &broad, args).unwrap();
    let request = undo(&db, &broad, result["auditId"].as_i64().unwrap());
    db.mcp_resolve_undo(request["undoRequestId"].as_i64().unwrap(), true)
        .unwrap();
    assert!(!db.get_task(id).unwrap().is_urgent);
    assert_eq!(db.get_task(id).unwrap().custom_sort_order, old_order);
    let stale = patch(&db, id, "gui-stale", "title", json!("MCP new"));
    let mut task = db.get_task(id).unwrap();
    task.title = "GUI change".into();
    db.save_task(serde_json::from_value(serde_json::to_value(task).unwrap()).unwrap())
        .unwrap();
    assert_eq!(
        db.mcp_mutate("a", &auth, stale).unwrap_err().code,
        "conflict"
    );
}
#[test]
fn p3_explicit_real_work_and_future_undo_preserve_history_and_reservations() {
    let (db, auth, _) = fixture();
    let future = (Local::now().date_naive() + chrono::Duration::days(4))
        .format("%Y-%m-%d")
        .to_string();
    let task:TaskInput=serde_json::from_value(json!({"title":"future","department":"D","departments":["D"],"contacts":["C"],"taskType":"T","details":"","status":"pending","priority":"normal","workload":"standard","isUrgent":false,"plannedDate":future})).unwrap();
    let task = db.save_task(task).unwrap();
    let v = db.mcp_task_basis(task.id, true).unwrap()["taskVersion"]
        .as_i64()
        .unwrap();
    let mut args = input(
        json!({"idempotencyKey":"future-work","action":"recordWorkEvent","target":{"taskId":task.id},"taskVersion":v,"resultStatus":"processed","handledAt":Local::now().to_rfc3339(),"note":"real work","syncStatus":true}),
    );
    args.intent.confirmed_real_work = false;
    assert_eq!(
        db.mcp_mutate("a", &auth, args.clone()).unwrap_err().code,
        "explicit_intent_required"
    );
    args.intent.confirmed_real_work = true;
    let result = db.mcp_mutate("a", &auth, args).unwrap();
    assert!(!db.get_task(task.id).unwrap().is_scheduled);
    let request = undo(&db, &auth, result["auditId"].as_i64().unwrap());
    db.mcp_resolve_undo(request["undoRequestId"].as_i64().unwrap(), true)
        .unwrap();
    let restored = db.get_task(task.id).unwrap();
    assert!(restored.is_scheduled);
    assert_eq!(restored.planned_date, future);
    assert_eq!(restored.permanent_number, task.permanent_number);
    assert!(restored.daily_sequence > task.daily_sequence);
    assert_eq!(restored.processing_rounds, 0);
}
#[test]
fn p3_preference_aba_and_cross_client_undo_do_not_bypass_permissions() {
    let (db, auth, _) = fixture();
    db.set_setting("ui_scale".into(), "100".into()).unwrap();
    let v = db
        .with_conn(|c| {
            c.query_row(
                "SELECT version FROM mcp_preference_versions WHERE key='ui_scale'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map_err(display_error)
        })
        .unwrap();
    let args:PreferenceArgs=serde_json::from_value(json!({"action":"patch","patch":{"ui_scale":"120"},"fieldBase":{"ui_scale":"100"},"fieldVersions":{"ui_scale":v},"idempotencyKey":"aba-pref","intent":{"summary":"explicit","explicitUserRequest":true},"reason":"synthetic"})).unwrap();
    db.set_setting("ui_scale".into(), "140".into()).unwrap();
    db.set_setting("ui_scale".into(), "100".into()).unwrap();
    assert_eq!(
        db.mcp_preferences("a", &auth, args).unwrap_err().code,
        "conflict"
    );
    let result = create(&db, &auth, "one");
    let args = UndoArgs {
        audit_id: result["auditId"].as_i64().unwrap(),
        idempotency_key: "foreign".into(),
        intent: Intent {
            summary: "explicit".into(),
            explicit_user_request: true,
            replace_whole_text: false,
            confirmed_real_work: false,
            allow_possible_duplicate: false,
        },
        reason: "synthetic".into(),
    };
    assert_eq!(
        db.mcp_request_undo("b", &auth, args).unwrap_err().code,
        "not_found"
    );
    let mut invalid=serde_json::to_value(input(json!({"action":"create","task":{"title":"one","departments":["D"],"contacts":["C"],"taskType":"T"}}))).unwrap();
    invalid["arbitrarySql"] = json!("DELETE FROM tasks");
    assert!(serde_json::from_value::<MutateArgs>(invalid).is_err());
}
#[test]
fn p3_schema11_upgrade_preserves_uuid_tasks_and_backs_up_before_mutation() {
    let root = std::env::temp_dir().join(format!(
        "inline-p3-migration-{}",
        crate::mcp::platform::random_secret().unwrap()
    ));
    let db = Database::open_root(root.clone()).unwrap();
    let auth = Authorization {
        permissions: Permissions {
            regular_read: true,
            full_read: true,
            write: true,
        },
        scope: Scope::default(),
        revision: 1,
    };
    let result = create(&db, &auth, "old");
    let task = db.get_task(result["taskId"].as_i64().unwrap()).unwrap();
    let uuid = db.mcp_basis().unwrap().database_uuid;
    db.with_conn(|c| {
        c.execute_batch("UPDATE schema_meta SET version=11")
            .map_err(display_error)
    })
    .unwrap();
    drop(db);
    let reopened = Database::open_root(root).unwrap();
    assert_eq!(reopened.mcp_basis().unwrap().schema_version, 12);
    assert_eq!(reopened.mcp_basis().unwrap().database_uuid, uuid);
    assert_eq!(
        serde_json::to_value(reopened.get_task(task.id).unwrap()).unwrap(),
        serde_json::to_value(task).unwrap()
    );
    assert!(reopened
        .list_backups()
        .unwrap()
        .iter()
        .any(|b| b.path.contains("before-migration")));
}
