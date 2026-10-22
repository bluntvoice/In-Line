use super::*;
use crate::{
    application::cli::*,
    mcp::{
        scope::Scope,
        security::{Authorization, Permissions},
        write_types::*,
    },
};
fn fixture() -> (Database, Authorization) {
    let path = std::env::temp_dir().join(format!(
        "inline-cli-phase4-{}",
        crate::mcp::platform::random_secret().unwrap()
    ));
    fs::create_dir_all(&path).unwrap();
    (
        Database::open_at(path.join("synthetic.db")).unwrap(),
        Authorization {
            permissions: Permissions {
                regular_read: true,
                full_read: false,
                write: true,
            },
            scope: Scope {
                departments: Some(vec!["A".into()]),
                task_types: None,
            },
            revision: 1,
        },
    )
}
fn task(title: &str, department: &str) -> TaskInput {
    serde_json::from_value(json!({"title":title,"department":department,"departments":[department],"contacts":["private-contact"],"taskType":"T","details":"private-body","internalNotes":"private-notes","status":"pending","priority":"normal","workload":"standard","isUrgent":false,"urgentRequester":"","urgentReason":""})).unwrap()
}
fn child(db: &Database, parent: i64, title: &str, dept: Option<&str>) -> LegalTask {
    db.create_subtask(serde_json::from_value(json!({"parentTaskId":parent,"title":title,"departments":dept.map(|s|vec![s]),"enqueueToday":true})).unwrap()).unwrap()
}
fn intent() -> Intent {
    serde_json::from_value(json!({"summary":"synthetic user requested real work","explicitUserRequest":true,"confirmedRealWork":true})).unwrap()
}
fn complete(db: &Database, parent: i64, key: &str) -> CompleteArgs {
    let ids = db.with_conn(|c| child_ids_on(c, parent)).unwrap();
    CompleteArgs {
        task_id: parent,
        task_version: db.mcp_task_basis(parent, true).unwrap()["taskVersion"]
            .as_i64()
            .unwrap(),
        task_versions: ids
            .into_iter()
            .map(|id| {
                (
                    id.to_string(),
                    db.mcp_task_basis(id, true).unwrap()["taskVersion"]
                        .as_i64()
                        .unwrap(),
                )
            })
            .collect(),
        include_eligible_subtasks: true,
        idempotency_key: key.into(),
        intent: intent(),
        reason: "synthetic scope completion".into(),
    }
}
fn rows(db: &Database) -> Json {
    db.with_conn(|c|{
        let mut s=c.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").map_err(display_error)?;
        let names=s.query_map([],|r|r.get::<_,String>(0)).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;
        let mut result=serde_json::Map::new();
        for name in names {assert!(name.chars().all(|c|c.is_ascii_alphanumeric()||c=='_'));let mut q=c.prepare(&format!("SELECT * FROM {name} ORDER BY rowid")).map_err(display_error)?;let count=q.column_count();let values=q.query_map([],|r|{(0..count).map(|i|Ok(format!("{:?}",r.get_ref(i)?))).collect::<Result<Vec<_>,rusqlite::Error>>()}).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;result.insert(name,json!(values));}
        Ok(json!(result))
    }).unwrap()
}
#[test]
fn group_completion_scope_aba_failure_after_child_work_and_local_undo_are_atomic() {
    let (db, auth) = fixture();
    let parent = db.save_task(task("parent", "A")).unwrap();
    let a = child(&db, parent.id, "child A", None);
    let b = child(&db, parent.id, "child B", Some("B"));
    let before = rows(&db);
    assert_eq!(
        db.cli_complete("cli", &auth, complete(&db, parent.id, "scope"))
            .unwrap_err()
            .code,
        "forbidden"
    );
    assert_eq!(rows(&db), before);
    let mut all = auth.clone();
    all.scope = Scope::default();
    let stale = complete(&db, parent.id, "aba");
    db.set_status(a.id, "processing".into()).unwrap();
    db.set_status(a.id, "pending".into()).unwrap();
    let before = rows(&db);
    assert_eq!(
        db.cli_complete("cli", &all, stale).unwrap_err().code,
        "conflict"
    );
    assert_eq!(rows(&db), before);
    let args = complete(&db, parent.id, "complete");
    let again: CompleteArgs = serde_json::from_value(serde_json::to_value(&args).unwrap()).unwrap();
    let result = db.cli_complete("cli", &all, args).unwrap();
    assert_eq!(result["verificationStatus"], "verified_after_commit");
    for id in [parent.id, a.id, b.id] {
        assert_eq!(db.get_task(id).unwrap().status, "completed");
    }
    let repeated = db.cli_complete("cli", &all, again).unwrap();
    assert_eq!(repeated["auditId"], result["auditId"]);
    assert_eq!(repeated["replayed"], true);
    let undo = db
        .mcp_request_undo(
            "cli",
            &all,
            UndoArgs {
                audit_id: result["auditId"].as_i64().unwrap(),
                idempotency_key: "undo".into(),
                intent: intent(),
                reason: "synthetic local approval needed".into(),
            },
        )
        .unwrap();
    assert_eq!(undo["status"], "needs_user_approval");
    assert_eq!(db.get_task(parent.id).unwrap().status, "completed");
    let result = db
        .mcp_resolve_undo(undo["undoRequestId"].as_i64().unwrap(), true)
        .unwrap();
    assert_eq!(result["verificationStatus"], "verified_after_commit");
    for id in [parent.id, a.id, b.id] {
        let t = db.get_task(id).unwrap();
        assert_eq!(t.status, "pending");
        let (all,valid)=db.with_conn(|c|c.query_row("SELECT COUNT(*),COALESCE(SUM(CASE WHEN voided_at IS NULL THEN 1 ELSE 0 END),0) FROM task_work_events WHERE task_id=? AND result_status='completed'",[id],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?))).map_err(display_error)).unwrap();
        assert!(all >= 1, "original completion rows must remain");
        assert_eq!(
            valid, 0,
            "compensated completion must not remain valid work"
        );
    }
    // A later parent failure after child completion must roll back all numbers,
    // work events, versions, audits and receipts in the same transaction.
    db.complete_task(CompleteTaskInput {
        task_id: parent.id,
        include_eligible_subtasks: false,
    })
    .unwrap();
    let before = rows(&db);
    assert_eq!(
        db.cli_complete("cli", &all, complete(&db, parent.id, "late-failure"))
            .unwrap_err()
            .code,
        "business_rule"
    );
    assert_eq!(rows(&db), before);
}
#[test]
fn child_creation_inherits_in_host_and_future_numbering_without_exposing_or_recording_fake_work() {
    let (db, auth) = fixture();
    let mut input = task("future parent", "A");
    input.planned_date = Some("2099-01-01".into());
    let parent = db.save_task(input).unwrap();
    let args:ChildArgs=serde_json::from_value(json!({"parentTaskId":parent.id,"parentVersion":db.mcp_task_basis(parent.id,true).unwrap()["taskVersion"],"task":{"title":"future child"},"idempotencyKey":"child","intent":intent(),"reason":"user requested child"})).unwrap();
    let result = db.cli_create_subtask("cli", &auth, args).unwrap();
    assert!(!result.to_string().contains("private-"));
    let task = db.get_task(result["taskId"].as_i64().unwrap()).unwrap();
    assert_eq!(task.contacts, parent.contacts);
    assert_eq!(task.departments, parent.departments);
    assert_eq!(task.task_type, parent.task_type);
    assert_eq!(task.planned_date, parent.planned_date);
    assert!(task.is_scheduled);
    assert!(task.daily_sequence > 0);
    assert!(db.list_work_events(task.id).unwrap().is_empty());
    let bad:ChildArgs=serde_json::from_value(json!({"parentTaskId":parent.id,"parentVersion":db.mcp_task_basis(parent.id,true).unwrap()["taskVersion"],"task":{"title":"outside child","departments":["B"]},"idempotencyKey":"bad-child","intent":intent(),"reason":"scope test"})).unwrap();
    let before = rows(&db);
    assert_eq!(
        db.cli_create_subtask("cli", &auth, bad).unwrap_err().code,
        "forbidden"
    );
    assert_eq!(rows(&db), before);
}
#[test]
fn receipt_lookup_is_scoped_ledger_read_only_and_missing_foreign_keys_do_not_execute() {
    let (db, auth) = fixture();
    let args:MutateArgs=serde_json::from_value(json!({"action":"create","task":{"title":"receipt","departments":["A"],"contacts":["synthetic-contact"],"taskType":"T"},"idempotencyKey":"original","intent":intent(),"reason":"synthetic request"})).unwrap();
    db.fail_mcp_verification
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let written = db.mcp_mutate("cli", &auth, args).unwrap();
    assert_eq!(written["verificationStatus"], "verification_failed");
    let before = rows(&db);
    let args = || ReceiptArgs {
        kind: None,
        idempotency_key: "original".into(),
    };
    let receipt = db.cli_receipt("cli", &auth, args()).unwrap();
    assert_eq!(receipt["readOnly"], true);
    assert_eq!(receipt["ledgerStatus"], "recorded");
    assert_eq!(receipt["receipt"]["auditId"], written["auditId"]);
    assert_eq!(
        db.cli_receipt("other", &auth, args()).unwrap_err().code,
        "not_found"
    );
    assert_eq!(
        db.cli_receipt(
            "cli",
            &auth,
            ReceiptArgs {
                kind: None,
                idempotency_key: "never-written".into()
            }
        )
        .unwrap_err()
        .code,
        "not_found"
    );
    let mut limited = auth.clone();
    limited.scope.departments = Some(vec!["B".into()]);
    assert_eq!(
        db.cli_receipt("cli", &limited, args()).unwrap_err().code,
        "forbidden"
    );
    assert_eq!(rows(&db), before);
}
