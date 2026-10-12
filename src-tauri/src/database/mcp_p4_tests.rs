use super::*;
use crate::mcp::{
    contract::McpError,
    scope::Scope,
    security::{Authorization, Client, Permissions, Store},
    write_types::*,
};
use serde_json::{json, Value as Json};
fn fixture() -> (Database, Authorization, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "inline-p4-{}",
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
fn intent() -> Intent {
    Intent {
        summary: "明确的合成用户指令".into(),
        explicit_user_request: true,
        replace_whole_text: true,
        confirmed_real_work: true,
        allow_possible_duplicate: false,
    }
}
fn task(title: &str) -> Json {
    json!({"title":title,"departments":["D"],"contacts":["private-contact"],"taskType":"T","details":"private-text"})
}
fn mutation(key: &str, op: Json) -> MutateArgs {
    let mut v = json!({"idempotencyKey":key,"intent":intent(),"reason":"synthetic user request"});
    for (k, val) in op.as_object().unwrap() {
        v[k] = val.clone();
    }
    serde_json::from_value(v).unwrap()
}
fn create(db: &Database, a: &Authorization, key: &str) -> i64 {
    db.mcp_mutate(
        "a",
        a,
        mutation(key, json!({"action":"create","task":task(key)})),
    )
    .unwrap()["taskId"]
        .as_i64()
        .unwrap()
}
fn ver(db: &Database, id: i64) -> i64 {
    db.mcp_task_basis(id, true).unwrap()["taskVersion"]
        .as_i64()
        .unwrap()
}
fn batch(key: &str, action: &str, ops: Vec<Json>) -> BatchArgs {
    serde_json::from_value(json!({"action":action,"operations":ops,"idempotencyKey":key,"intent":intent(),"reason":"synthetic batch"})).unwrap()
}
fn item(op: Json) -> Json {
    json!({"operation":op})
}
fn rows(db: &Database) -> Json {
    db.with_conn(|conn|{let mut s=conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").map_err(display_error)?;let tables=s.query_map([],|r|r.get::<_,String>(0)).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;
        let mut result=serde_json::Map::new();for name in tables{assert!(name.chars().all(|c|c.is_ascii_alphanumeric()||c=='_'));let mut q=conn.prepare(&format!("SELECT * FROM {name} ORDER BY rowid")).map_err(display_error)?;let count=q.column_count();let data=q.query_map([],|r|{let mut row=vec![];for i in 0..count{row.push(format!("{:?}",r.get_ref(i)?));}Ok(row)}).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;result.insert(name,json!(data));}Ok(json!(result))}).unwrap()
}
fn state(_db: &Database) -> Store {
    Store {
        sensitive_notice_acknowledged: false,
        version: 1,
        machine: "synthetic".into(),
        sid: "synthetic".into(),
        ipc_secret: "not-production".into(),
        revision: 1,
        paused: false,
        groups: Permissions {
            regular_read: true,
            full_read: false,
            write: true,
        },
        clients: vec![Client {
            id: "a".into(),
            name: "synthetic".into(),
            kind: None,
            token_hash: "not-a-credential".into(),
            revoked: false,
            permissions: Permissions {
                regular_read: true,
                full_read: false,
                write: true,
            },
            scope: Scope {
                departments: Some(vec!["D".into()]),
                task_types: Some(vec!["T".into()]),
            },
        }],
    }
}
fn approve_undo(db: &Database, a: &Authorization, aid: i64) -> Result<Json, McpError> {
    let r = db.mcp_request_undo(
        "a",
        a,
        UndoArgs {
            audit_id: aid,
            idempotency_key: format!("undo-{aid}"),
            intent: intent(),
            reason: "explicit synthetic undo".into(),
        },
    )?;
    db.mcp_resolve_undo(r["undoRequestId"].as_i64().unwrap(), true)
}

#[test]
fn p4_dry_run_and_mixed_failure_preserve_every_business_number_version_dictionary_and_receipt() {
    let (db, a, _) = fixture();
    let before = rows(&db);
    let request = batch(
        "dry",
        "dryRun",
        vec![item(json!({"action":"create","task":task("new")}))],
    );
    let r = db.mcp_batch("a", &a, request).unwrap();
    assert_eq!(r["commitStatus"], "dry_run");
    assert_eq!(rows(&db), before);
    assert!(!r.to_string().contains("private-contact"));
    assert!(!r.to_string().contains("private-text"));
    let r=db.mcp_batch("a",&a,batch("bad","apply",vec![item(json!({"action":"create","task":task("rolled-back")})),item(json!({"action":"setStatus","target":{"taskId":9999},"taskVersion":0,"status":"pending"}))]));
    assert_eq!(r.unwrap_err().code, "not_found");
    assert_eq!(rows(&db), before);
}
#[test]
fn p4_batch_refs_two_level_structure_and_group_undo_are_atomic() {
    let (db, a, _) = fixture();
    let ops = vec![
        json!({"refKey":"p","operation":{"action":"create","task":task("parent")}}),
        json!({"refKey":"c","operation":{"action":"createSubtask","parent":{"batchRef":"p"},"parentVersion":0,"task":task("child"),"enqueueToday":false}}),
    ];
    let before = rows(&db);
    let mut three = ops.clone();
    three.push(item(json!({"action":"createSubtask","parent":{"batchRef":"c"},"parentVersion":0,"task":task("third")})));
    assert!(db
        .mcp_batch("a", &a, batch("three", "apply", three))
        .is_err());
    assert_eq!(rows(&db), before);
    let result = db
        .mcp_batch("a", &a, batch("two", "apply", ops.clone()))
        .unwrap();
    let parent = result["refs"]["p"][0].as_i64().unwrap();
    let child = result["refs"]["c"][0].as_i64().unwrap();
    assert_eq!(db.get_task(child).unwrap().parent_task_id, Some(parent));
    assert!(!db.get_task(child).unwrap().has_active_queue);
    let repeat = db.mcp_batch("a", &a, batch("two", "apply", ops)).unwrap();
    assert_eq!(repeat["replayed"], true);
    assert_eq!(db.mcp_tasks().unwrap().len(), 2);
    approve_undo(&db, &a, result["auditId"].as_i64().unwrap()).unwrap();
    assert!(db.get_task(parent).unwrap().deleted_at.is_some());
    assert!(db.get_task(child).unwrap().deleted_at.is_some());
}
#[test]
fn p4_limit_configuration_cumulative_budget_and_ambiguous_batch_are_safe() {
    let (db, a, _) = fixture();
    let too_many = (0..21)
        .map(|i| item(json!({"action":"create","task":task(&format!("t-{i}"))})))
        .collect();
    assert_eq!(
        db.mcp_batch("a", &a, batch("21", "apply", too_many))
            .unwrap_err()
            .code,
        "resource_limit"
    );
    assert!(db.mcp_set_batch_limit(0).is_err());
    assert!(db.mcp_set_batch_limit(101).is_err());
    db.mcp_set_batch_limit(1).unwrap();
    assert_eq!(db.mcp_batch_limit().unwrap(), 1);
    db.with_conn(|c| {
        c.execute(
            "INSERT INTO mcp_batch_usage VALUES('a',?,200)",
            [Utc::now().timestamp()],
        )
        .map_err(display_error)?;
        Ok(())
    })
    .unwrap();
    let before = rows(&db);
    assert_eq!(
        db.mcp_batch(
            "a",
            &a,
            batch(
                "over-budget",
                "apply",
                vec![item(json!({"action":"create","task":task("x")}))]
            )
        )
        .unwrap_err()
        .code,
        "rate_limited"
    );
    assert_eq!(rows(&db), before);
    db.with_conn(|c| {
        c.execute("DELETE FROM mcp_batch_usage", [])
            .map_err(display_error)?;
        Ok(())
    })
    .unwrap();
    db.mcp_set_batch_limit(20).unwrap();
    let one = create(&db, &a, "same");
    let mut duplicate = mutation("same2", json!({"action":"create","task":task("same")}));
    duplicate.intent.allow_possible_duplicate = true;
    db.mcp_mutate("a", &a, duplicate).unwrap();
    let before = rows(&db);
    assert_eq!(db.mcp_batch("a",&a,batch("ambiguous","apply",vec![item(json!({"action":"patch","target":{"title":"same"},"patch":{"title":"wrong"},"fieldBase":{"title":{"value":"same","version":0}}})),item(json!({"action":"setStatus","target":{"taskId":one},"taskVersion":ver(&db,one),"status":"processing"}))])).unwrap_err().code,"needs_disambiguation");
    assert_eq!(rows(&db), before);
}
fn reschedules(db: &Database, ids: &[i64], date: &str) -> Vec<Json> {
    ids.iter().map(|id|item(json!({"action":"reschedule","target":{"taskId":id},"taskVersion":ver(db,*id),"plannedDate":date,"requestedDeadline":null}))).collect()
}
#[test]
fn p4_bulk_date_approval_is_once_rechecks_revocation_conflicts_and_preserves_number_history() {
    let (db, a, _) = fixture();
    let ids = [create(&db, &a, "one"), create(&db, &a, "two")];
    let original = ids.map(|id| db.get_task(id).unwrap());
    let date = (Local::now() + chrono::Duration::days(3))
        .format("%Y-%m-%d")
        .to_string();
    let args = batch("dates", "apply", reschedules(&db, &ids, &date));
    let basis = db.mcp_basis().unwrap();
    let result = db.mcp_batch("a", &a, args.clone()).unwrap();
    assert_eq!(result["commitStatus"], "needs_user_approval");
    assert_eq!(db.mcp_basis().unwrap(), basis);
    assert!(!db.get_task(ids[0]).unwrap().is_scheduled);
    let rid = result["approvalId"].as_i64().unwrap();
    let mut revoked = state(&db);
    revoked.clients[0].revoked = true;
    assert_eq!(
        db.mcp_resolve_batch(rid, true, &revoked).unwrap_err().code,
        "revoked"
    );
    assert_eq!(db.mcp_basis().unwrap(), basis);
    let accepted = db.mcp_resolve_batch(rid, true, &state(&db)).unwrap();
    assert_eq!(accepted["verificationStatus"], "verified_after_commit");
    for (id, old) in ids.iter().zip(original) {
        let t = db.get_task(*id).unwrap();
        assert!(t.is_scheduled);
        assert_eq!(t.planned_date, date);
        assert_eq!(t.permanent_number, old.permanent_number);
        assert!(!t.has_active_queue);
    }
    let same = db.mcp_batch("a", &a, args).unwrap();
    assert_eq!(same["auditId"], accepted["auditId"]);
    assert_eq!(same["replayed"], true);
    assert_eq!(
        db.mcp_resolve_batch(rid, true, &state(&db)).unwrap()["status"],
        "approved"
    );
    let third = create(&db, &a, "three");
    let fourth = create(&db, &a, "four");
    let pending = db
        .mcp_batch(
            "a",
            &a,
            batch(
                "conflict",
                "apply",
                reschedules(&db, &[third, fourth], &date),
            ),
        )
        .unwrap();
    let mut input: TaskInput = serde_json::from_value(json!(db.get_task(third).unwrap())).unwrap();
    input.title = "GUI edit".into();
    db.save_task(input).unwrap();
    let before = rows(&db);
    assert_eq!(
        db.mcp_resolve_batch(pending["approvalId"].as_i64().unwrap(), true, &state(&db))
            .unwrap_err()
            .code,
        "conflict"
    );
    assert_eq!(rows(&db), before);
}
#[test]
fn p4_future_creation_queue_real_work_and_correction_keep_linked_history() {
    let (db, a, _) = fixture();
    let date = (Local::now() + chrono::Duration::days(2))
        .format("%Y-%m-%d")
        .to_string();
    let mut future = task("future");
    future["plannedDate"] = json!(date);
    let result = db
        .mcp_mutate(
            "a",
            &a,
            mutation("future", json!({"action":"create","task":future})),
        )
        .unwrap();
    let id = result["taskId"].as_i64().unwrap();
    let t = db.get_task(id).unwrap();
    assert!(t.is_scheduled);
    assert!(!t.has_active_queue);
    let mut not_real = mutation(
        "not-real",
        json!({"action":"queue","target":{"taskId":id},"taskVersion":ver(&db,id),"mode":"processRound"}),
    );
    not_real.intent.confirmed_real_work = false;
    let before = rows(&db);
    assert_eq!(
        db.mcp_mutate("a", &a, not_real).unwrap_err().code,
        "explicit_intent_required"
    );
    assert_eq!(rows(&db), before);
    db.mcp_mutate("a",&a,mutation("work",json!({"action":"recordWorkEvent","target":{"taskId":id},"taskVersion":ver(&db,id),"resultStatus":"processed","handledAt":Utc::now().to_rfc3339(),"note":"real synthetic work","syncStatus":true}))).unwrap();
    let event = db.list_work_events(id).unwrap()[0].id;
    let correction=db.mcp_mutate("a",&a,mutation("correction",json!({"action":"correctWorkEvent","target":{"taskId":id},"taskVersion":ver(&db,id),"eventId":event,"resultStatus":"waiting_materials","handledAt":Utc::now().to_rfc3339(),"note":"corrected fact","confirmHistoricalImpact":true}))).unwrap();
    assert!(!correction.to_string().contains("corrected fact"));
    db.with_conn(|c|{let rows:(i64,i64)=c.query_row("SELECT (SELECT COUNT(*) FROM task_work_events WHERE id=? AND voided_at IS NOT NULL),(SELECT COUNT(*) FROM task_work_events WHERE correction_of=? AND voided_at IS NULL)",params![event,event],|r|Ok((r.get(0)?,r.get(1)?))).map_err(display_error)?;assert_eq!(rows,(1,1));Ok(())}).unwrap();
    approve_undo(&db, &a, correction["auditId"].as_i64().unwrap()).unwrap();
    db.with_conn(|c|{let count:i64=c.query_row("SELECT COUNT(*) FROM task_work_events WHERE task_id=? AND voided_at IS NULL AND result_status='processed'",[id],|r|r.get(0)).map_err(display_error)?;assert_eq!(count,1);Ok(())}).unwrap();
}
#[test]
fn p4_scope_all_affected_batch_receipts_and_permission_downgrade_are_enforced() {
    let (db, mut a, _) = fixture();
    a.permissions.full_read = true;
    let id = create(&db, &a, "private");
    let args = batch(
        "details",
        "apply",
        vec![item(
            json!({"action":"patch","target":{"taskId":id},"fieldBase":{"details":{"value":"private-text","version":0}},"patch":{"details":"changed-private"}}),
        )],
    );
    let first = db.mcp_batch("a", &a, args.clone()).unwrap();
    assert!(first.to_string().contains("private-text"));
    a.permissions.full_read = false;
    let replay = db.mcp_batch("a", &a, args.clone()).unwrap();
    assert!(!replay.to_string().contains("private-text"));
    assert!(!replay.to_string().contains("changed-private"));
    a.scope.departments = Some(vec!["Other".into()]);
    assert_eq!(db.mcp_batch("a", &a, args).unwrap_err().code, "forbidden");
}
#[test]
fn p4_backup_is_local_full_business_only_durable_idempotent_and_unreadable_via_api() {
    let (db, a, root) = fixture();
    let id = create(&db, &a, "backed");
    let args:BusinessDataArgs=serde_json::from_value(json!({"action":"backup","idempotencyKey":"backup","intent":intent(),"reason":"explicit local backup"})).unwrap();
    let result = db
        .mcp_artifact("a", &a, args.clone(), || {
            panic!("backup must not query export")
        })
        .unwrap();
    let repeated = db
        .mcp_artifact("a", &a, args, || panic!("replay must not export"))
        .unwrap();
    assert_eq!(result["auditId"], repeated["auditId"]);
    assert_eq!(repeated["replayed"], true);
    assert!(db
        .list_backups()
        .unwrap()
        .iter()
        .any(|v| v.name == result["artifactName"].as_str().unwrap()));
    let path = db
        .backup_dir
        .join("mcp-created")
        .join(result["artifactName"].as_str().unwrap());
    assert!(path.starts_with(root));
    let backup = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let title: String = backup
        .query_row("SELECT title FROM tasks WHERE id=?", [id], |r| r.get(0))
        .unwrap();
    assert_eq!(title, "backed");
    let auth_tables:i64=backup.query_row("SELECT COUNT(*) FROM sqlite_master WHERE name LIKE '%token%' OR name LIKE '%credential%'",[],|r|r.get(0)).unwrap();
    assert_eq!(auth_tables, 0);
    assert_eq!(
        fs::read_dir(db.backup_dir.join("mcp-created"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        approve_undo(&db, &a, result["auditId"].as_i64().unwrap())
            .unwrap_err()
            .code,
        "unsupported"
    );
}
#[test]
fn p4_business_export_filters_scope_fields_and_csv_formulas_without_backup_access() {
    let (db, a, root) = fixture();
    create(&db, &a, "=SUM(1,2)");
    let mut unrestricted = a.clone();
    unrestricted.scope = Scope::default();
    let mut outside = task("outside");
    outside["departments"] = json!(["Other"]);
    db.mcp_mutate(
        "a",
        &unrestricted,
        mutation("outside", json!({"action":"create","task":outside})),
    )
    .unwrap();
    let args:BusinessDataArgs=serde_json::from_value(json!({"action":"export","format":"json","idempotencyKey":"export","intent":intent(),"reason":"explicit business export"})).unwrap();
    let mut queries = crate::mcp::query::QueryState::new(root.join("queries"));
    let result =
        crate::mcp::business_data::execute(&mut queries, &db, "a", &a, args.clone()).unwrap();
    let bytes = fs::read(
        db.backup_dir
            .join("mcp-created")
            .join(result["artifactName"].as_str().unwrap()),
    )
    .unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert!(!text.contains("private-contact"));
    assert!(!text.contains("private-text"));
    assert!(!text.contains("outside"));
    let data: Json = serde_json::from_str(&text).unwrap();
    assert_eq!(data["items"].as_array().unwrap().len(), 1);
    assert_eq!(data["meta"]["complete"], true);
    let mut denied = args.clone();
    denied.idempotency_key = "denied".into();
    denied.projection = Some(vec!["contacts".into()]);
    let before = rows(&db);
    assert_eq!(
        crate::mcp::business_data::execute(&mut queries, &db, "a", &a, denied)
            .unwrap_err()
            .code,
        "forbidden"
    );
    assert_eq!(rows(&db), before);
    let mut csv = args;
    csv.format = Some("csv".into());
    csv.idempotency_key = "csv".into();
    let result = crate::mcp::business_data::execute(&mut queries, &db, "a", &a, csv).unwrap();
    let csv = fs::read_to_string(
        db.backup_dir
            .join("mcp-created")
            .join(result["artifactName"].as_str().unwrap()),
    )
    .unwrap();
    assert!(csv.contains("'=SUM(1,2)"));
}
#[test]
fn p4_postcommit_batch_verification_failure_does_not_replay_business_writes() {
    let (db, a, _) = fixture();
    db.fail_mcp_verification
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let args = batch(
        "verify",
        "apply",
        vec![item(json!({"action":"create","task":task("once")}))],
    );
    let first = db.mcp_batch("a", &a, args.clone()).unwrap();
    assert_eq!(first["verificationStatus"], "verification_failed");
    let repeat = db.mcp_batch("a", &a, args).unwrap();
    assert_eq!(repeat["verificationStatus"], "verified_after_commit");
    assert_eq!(repeat["replayed"], true);
    assert_eq!(first["auditId"], repeat["auditId"]);
    assert_eq!(db.mcp_tasks().unwrap().len(), 1);
}

#[test]
fn p4_deadline_batch_requires_single_approval_and_cannot_be_smuggled_as_patch() {
    let (db, a, _) = fixture();
    let ids = [create(&db, &a, "deadline1"), create(&db, &a, "deadline2")];
    let ops=ids.iter().map(|id|item(json!({"action":"patch","target":{"taskId":id},"fieldBase":{"requestedDeadline":{"value":null,"version":0}},"patch":{"requestedDeadline":(Local::now()+chrono::Duration::days(2)).to_rfc3339()}}))).collect();
    let basis = db.mcp_basis().unwrap();
    let proposal = db
        .mcp_batch("a", &a, batch("deadlines", "apply", ops))
        .unwrap();
    assert_eq!(proposal["commitStatus"], "needs_user_approval");
    assert_eq!(db.mcp_basis().unwrap(), basis);
    assert!(ids
        .iter()
        .all(|id| db.get_task(*id).unwrap().requested_deadline.is_none()));
    db.mcp_resolve_batch(proposal["approvalId"].as_i64().unwrap(), true, &state(&db))
        .unwrap();
    assert!(ids
        .iter()
        .all(|id| db.get_task(*id).unwrap().requested_deadline.is_some()));
}

#[test]
fn p4_artifact_unknown_result_never_regenerates_and_permission_change_rejects_old_receipt() {
    let (db, a, _) = fixture();
    create(&db, &a, "artifact");
    let args:BusinessDataArgs=serde_json::from_value(json!({"action":"export","format":"json","idempotencyKey":"lost","intent":intent(),"reason":"synthetic export"})).unwrap();
    assert_eq!(
        db.mcp_artifact("a", &a, args.clone(), || Err(
            crate::mcp::contract::McpError::new("resource_limit")
        ))
        .unwrap_err()
        .code,
        "resource_limit"
    );
    let before = rows(&db);
    assert_eq!(
        db.mcp_artifact("a", &a, args, || panic!("must not rerun reserved export"))
            .unwrap_err()
            .code,
        "result_unknown"
    );
    assert_eq!(rows(&db), before);
    let args:BusinessDataArgs=serde_json::from_value(json!({"action":"backup","idempotencyKey":"old","intent":intent(),"reason":"synthetic backup"})).unwrap();
    let result = db.mcp_artifact("a", &a, args.clone(), || panic!()).unwrap();
    let mut narrowed = a.clone();
    narrowed.scope.departments = Some(vec!["Other".into()]);
    assert_eq!(
        db.mcp_artifact("a", &narrowed, args, || panic!())
            .unwrap_err()
            .code,
        "idempotency_conflict"
    );
    db.delete_backup(
        db.backup_dir
            .join("mcp-created")
            .join(result["artifactName"].as_str().unwrap())
            .to_string_lossy()
            .into(),
    )
    .unwrap();
    assert!(db.list_backups().unwrap().is_empty());
}

#[test]
fn p4_backup_merge_translates_correction_ids_and_preserves_original_voided_history() {
    let (source, a, _) = fixture();
    let id = create(&source, &a, "corrected");
    source.mcp_mutate("a",&a,mutation("work-original",json!({"action":"recordWorkEvent","target":{"taskId":id},"taskVersion":ver(&source,id),"resultStatus":"processed","handledAt":Utc::now().to_rfc3339(),"note":"original fact","syncStatus":false}))).unwrap();
    let original = source.list_work_events(id).unwrap()[0].id;
    source.mcp_mutate("a",&a,mutation("work-correct",json!({"action":"correctWorkEvent","target":{"taskId":id},"taskVersion":ver(&source,id),"eventId":original,"resultStatus":"waiting_materials","handledAt":Utc::now().to_rfc3339(),"note":"corrected fact","confirmHistoricalImpact":true}))).unwrap();
    let backup = source.create_backup("manual").unwrap();
    let (target, ta, _) = fixture();
    let filler = create(&target, &ta, "unrelated");
    target
        .record_work_event(WorkEventInput {
            task_id: filler,
            result_status: "processed".into(),
            handled_at: Utc::now().to_rfc3339(),
            note: "unrelated".into(),
            sync_status: false,
        })
        .unwrap();
    let imported = target.backup_dir.join("correction-source.db");
    fs::copy(backup.path, &imported).unwrap();
    target
        .restore_backup(imported.to_string_lossy().into())
        .unwrap();
    let merged = target
        .mcp_tasks()
        .unwrap()
        .into_iter()
        .find(|t| t.title == "corrected")
        .unwrap();
    let verify = || {
        target.with_conn(|c|{let count:i64=c.query_row("SELECT COUNT(*) FROM task_work_events corrected JOIN task_work_events original ON original.id=corrected.correction_of WHERE corrected.task_id=? AND original.task_id=? AND corrected.voided_at IS NULL AND original.voided_at IS NOT NULL AND corrected.note='corrected fact' AND original.note='original fact'",params![merged.id,merged.id],|r|r.get(0)).map_err(display_error)?;assert_eq!(count,1);let total:i64=c.query_row("SELECT COUNT(*) FROM task_work_events WHERE task_id=?",[merged.id],|r|r.get(0)).map_err(display_error)?;assert_eq!(total,2);Ok(())}).unwrap()
    };
    verify();
    target
        .restore_backup(imported.to_string_lossy().into())
        .unwrap();
    verify();
    let timeline = target.mcp_history(merged.id, true, true, false).unwrap();
    assert!(timeline.iter().any(|v| v["correctionOf"].is_i64()));
}

#[test]
fn p4_link_sort_trash_restore_and_indirect_scope_roll_back_as_a_group() {
    let (db, a, _) = fixture();
    let parent = create(&db, &a, "parent-scope");
    let first = create(&db, &a, "child-one");
    let second = create(&db, &a, "child-two");
    for (index, id) in [first, second].into_iter().enumerate() {
        db.mcp_mutate("a",&a,mutation(&format!("link-{index}"),json!({"action":"setParent","target":{"taskId":id},"taskVersion":ver(&db,id),"parent":{"taskId":parent},"parentVersion":ver(&db,parent)}))).unwrap();
    }
    db.mcp_mutate("a",&a,mutation("sort",json!({"action":"reorderSubtasks","parent":{"taskId":parent},"parentVersion":ver(&db,parent),"taskIds":[second,first],"taskVersions":{second.to_string():ver(&db,second),first.to_string():ver(&db,first)}}))).unwrap();
    assert!(
        db.get_task(second).unwrap().subtask_sort_order
            < db.get_task(first).unwrap().subtask_sort_order
    );
    let mut child: TaskInput = serde_json::from_value(json!(db.get_task(first).unwrap())).unwrap();
    child.departments = vec!["Other".into()];
    child.department = "Other".into();
    db.save_task(child).unwrap();
    let before = rows(&db);
    assert_eq!(db.mcp_batch("a",&a,batch("scope-delete","apply",vec![item(json!({"action":"trash","target":{"taskId":parent},"taskVersion":ver(&db,parent),"includeSubtasks":true}))])).unwrap_err().code,"forbidden");
    assert_eq!(rows(&db), before);
    let mut full = a.clone();
    full.scope = Scope::default();
    db.mcp_mutate("a",&full,mutation("trash-all",json!({"action":"trash","target":{"taskId":parent},"taskVersion":ver(&db,parent),"includeSubtasks":true}))).unwrap();
    assert!([parent, first, second].iter().all(|id| db
        .get_task(*id)
        .unwrap()
        .deleted_at
        .is_some()));
    db.mcp_mutate(
        "a",
        &full,
        mutation(
            "restore-parent",
            json!({"action":"restore","target":{"taskId":parent},"taskVersion":ver(&db,parent)}),
        ),
    )
    .unwrap();
    assert!(db.get_task(parent).unwrap().deleted_at.is_none());
    assert!(db.get_task(first).unwrap().deleted_at.is_some());
}

#[test]
fn p4_batch_limit_counts_indirect_children_and_rolls_back_their_histories() {
    let (db, a, _) = fixture();
    let parent = create(&db, &a, "limit-parent");
    let child = create(&db, &a, "limit-child");
    db.mcp_mutate("a",&a,mutation("limit-link",json!({"action":"setParent","target":{"taskId":child},"taskVersion":ver(&db,child),"parent":{"taskId":parent},"parentVersion":ver(&db,parent)}))).unwrap();
    db.mcp_set_batch_limit(1).unwrap();
    let before = rows(&db);
    let result=db.mcp_batch("a",&a,batch("limit-group","apply",vec![item(json!({"action":"trash","target":{"taskId":parent},"taskVersion":ver(&db,parent),"includeSubtasks":true}))]));
    assert_eq!(result.unwrap_err().code, "resource_limit");
    assert_eq!(rows(&db), before);
}

#[test]
fn p4_work_preview_redacts_notes_after_downgrade_and_gui_merge_keeps_correction_chain() {
    let (db, mut a, _) = fixture();
    let original = create(&db, &a, "preview-work");
    let target = create(&db, &a, "merge-target");
    a.permissions.full_read = true;
    let handled = Utc::now().to_rfc3339();
    let args = batch(
        "preview-event",
        "apply",
        vec![item(
            json!({"action":"recordWorkEvent","target":{"taskId":original},"taskVersion":ver(&db,original),"resultStatus":"processed","handledAt":handled,"note":"private-event-note","syncStatus":false}),
        )],
    );
    let first = db.mcp_batch("a", &a, args.clone()).unwrap();
    assert!(first.to_string().contains("private-event-note"));
    a.permissions.full_read = false;
    let replay = db.mcp_batch("a", &a, args).unwrap();
    assert!(!replay.to_string().contains("private-event-note"));
    let event = db.list_work_events(original).unwrap()[0].id;
    db.mcp_mutate("a",&a,mutation("merge-correct",json!({"action":"correctWorkEvent","target":{"taskId":original},"taskVersion":ver(&db,original),"eventId":event,"resultStatus":"waiting_materials","handledAt":handled,"note":"new fact","confirmHistoricalImpact":true}))).unwrap();
    // An exact voided duplicate on the destination used to allow deletion of the linked original.
    db.with_transaction(|c|{c.execute("INSERT INTO task_work_events(task_id,result_status,handled_at,task_type_snapshot,source,note,created_at,updated_at,voided_at) SELECT ?,result_status,handled_at,task_type_snapshot,source,note,created_at,updated_at,voided_at FROM task_work_events WHERE id=?",params![target,event]).map_err(display_error)?;Ok(())}).unwrap();
    db.merge_tasks(MergeTaskInput {
        target_task_id: target,
        source_task_id: original,
        deduplicate_records: true,
        trash_source: true,
    })
    .unwrap();
    db.with_conn(|c|{let n:i64=c.query_row("SELECT COUNT(*) FROM task_work_events newer JOIN task_work_events old ON old.id=newer.correction_of WHERE newer.task_id=? AND old.task_id=? AND old.voided_at IS NOT NULL AND newer.voided_at IS NULL",params![target,target],|r|r.get(0)).map_err(display_error)?;assert_eq!(n,1);Ok(())}).unwrap();
}
