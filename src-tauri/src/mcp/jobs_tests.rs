use super::super::{
    contract::Credentials,
    security::{Grant, Permissions, Scope, Security},
    service,
};
use super::*;
use crate::models::TaskInput;
fn fixture() -> (Database, Security, Credentials, Credentials) {
    let root =
        std::env::temp_dir().join(format!("inline-p5-{}", platform::random_secret().unwrap()));
    let db = Database::open_at(root.join("inline.db")).unwrap();
    let s = Security::open_at(root.join("security")).unwrap();
    let p = Permissions {
        regular_read: true,
        full_read: true,
        write: true,
    };
    s.set_groups(p.clone()).unwrap();
    let grant = |name: &str| {
        let c = s
            .grant(Grant {
                name: name.into(),
                permissions: p.clone(),
                scope: Scope {
                    departments: Some(vec!["A".into()]),
                    task_types: None,
                },
            })
            .unwrap();
        Credentials {
            client_id: c.client_id,
            token: c.token,
        }
    };
    let a = grant("A");
    let b = grant("B");
    (db, s, a, b)
}
fn input(title: &str, department: &str) -> TaskInput {
    serde_json::from_value(json!({"title":title,"department":department,"departments":[department],"contact":"C","contacts":["C"],"taskType":"T","status":"pending","priority":"normal","workload":"standard","isUrgent":false,"details":"secret"})).unwrap()
}
fn args(v: Value) -> JobsArgs {
    serde_json::from_value(v).unwrap()
}
fn auth(s: &Security, c: &Credentials) -> Authorization {
    s.store.lock().unwrap().authorize(c).unwrap()
}
fn wait(s: &Security, id: &str) -> Job {
    for _ in 0..300 {
        let j = s.jobs.state.lock().unwrap().jobs[id].clone();
        if !active(&j.state) {
            return j;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("job did not reach terminal checkpoint")
}
fn call(s: &Security, db: &Database, c: &Credentials, v: Value) -> Value {
    service::execute(s, db, c, "manage_jobs", v).unwrap()
}
#[test]
fn first_anchors_cannot_be_renewed_by_restart_or_repeated_limits() {
    assert_eq!(expired(Some(100), 100, false, 1899), None);
    assert_eq!(expired(Some(100), 1900, false, 1900), Some("pause_expired"));
    assert_eq!(expired(None, 100, true, 86499), None);
    assert_eq!(expired(None, 100, true, 86500), Some("recovery_expired"));
    assert_eq!(expired(None, 100, true, 99), Some("clock_invalid"));
}
fn seed_interrupted(s: &Security, mut j: Job) -> String {
    j.id = platform::random_secret().unwrap();
    j.state = "running".into();
    j.finished = None;
    j.error = None;
    let r = j.rounds.last_mut().unwrap();
    r.ended = None;
    r.outcome = None;
    save(&s.jobs.root, &j).unwrap();
    j.id
}
fn reopen(s: &Security, db: &Database) -> Security {
    let reopened = Security::open_at(s.jobs.root.parent().unwrap().to_owned()).unwrap();
    reopened.jobs.initialize(db, None).unwrap();
    reopened
}
#[test]
fn persisted_partial_pages_recover_same_basis_without_duplicate_or_mixed_rows() {
    let (db, s, a, _) = fixture();
    for n in 0..205 {
        db.save_task(input(&format!("item-{n}"), "A")).unwrap();
    }
    let start = call(
        &s,
        &db,
        &a,
        json!({"action":"start","kind":"query_tasks","arguments":{"projection":["id","title"]}}),
    );
    let complete = wait(&s, start["job"]["jobId"].as_str().unwrap());
    assert_eq!(complete.rows.len(), 205);
    let expected = complete.rows.clone();
    let mut partial = complete;
    partial.rows.truncate(100);
    let id = seed_interrupted(&s, partial);
    let recovered = reopen(&s, &db);
    let result = wait(&recovered, &id);
    assert_eq!(result.state, "completed");
    assert_eq!(result.rows, expected);
    assert_eq!(result.rounds.len(), 1);
}
#[test]
fn original_intent_permits_only_one_independent_full_requery() {
    let (db, s, a, _) = fixture();
    db.save_task(input("old", "A")).unwrap();
    let start = call(
        &s,
        &db,
        &a,
        json!({"action":"start","kind":"query_tasks","arguments":{},"allowLatestAfterInvalidation":true}),
    );
    let first = wait(&s, start["job"]["jobId"].as_str().unwrap());
    db.save_task(input("new", "A")).unwrap();
    let id = seed_interrupted(&s, first);
    let recovered = reopen(&s, &db);
    let second = wait(&recovered, &id);
    assert_eq!(second.state, "completed");
    assert_eq!(second.rounds.len(), 2);
    assert_eq!(
        second.rounds[0].outcome.as_deref(),
        Some("baseline_invalid")
    );
    assert_eq!(second.rows.len(), 2);
    db.save_task(input("another", "A")).unwrap();
    let id = seed_interrupted(&recovered, second);
    let third = reopen(&recovered, &db);
    let failed = wait(&third, &id);
    assert_eq!(failed.state, "failed");
    assert_eq!(failed.error.as_deref(), Some("snapshot_invalid"));
    assert!(failed.rows.is_empty());
    assert_eq!(failed.rounds.len(), 2);
}
#[test]
fn interrupted_deadlines_pause_and_global_revocation_do_not_renew() {
    let (db, s, a, _) = fixture();
    let start = call(
        &s,
        &db,
        &a,
        json!({"action":"start","kind":"query_tasks","arguments":{}}),
    );
    let complete = wait(&s, start["job"]["jobId"].as_str().unwrap());
    let mut old = complete.clone();
    old.checkpoint = Utc::now().timestamp() - 86401;
    let old_id = seed_interrupted(&s, old);
    let mut paused = complete.clone();
    paused.first_paused = Some(Utc::now().timestamp() - 1801);
    let paused_id = seed_interrupted(&s, paused);
    let third_id = seed_interrupted(&s, complete);
    s.pause(true).unwrap();
    let reopened = reopen(&s, &db);
    assert_eq!(
        wait(&reopened, &old_id).error.as_deref(),
        Some("recovery_expired")
    );
    assert_eq!(
        wait(&reopened, &paused_id).error.as_deref(),
        Some("pause_expired")
    );
    assert_eq!(wait(&reopened, &third_id).error.as_deref(), Some("paused"));
    assert!(wait(&reopened, &old_id).checkpoint < Utc::now().timestamp() - 86400);
}
#[test]
fn completed_result_live_scope_and_retention_are_rechecked() {
    let (db, s, a, _) = fixture();
    let t = db.save_task(input("authorized", "A")).unwrap();
    let start = call(
        &s,
        &db,
        &a,
        json!({"action":"start","kind":"query_tasks","arguments":{}}),
    );
    let id = start["job"]["jobId"].as_str().unwrap();
    wait(&s, id);
    let mut edit = input("outside-secret", "B");
    edit.id = Some(t.id);
    db.save_task(edit).unwrap();
    assert_eq!(
        service::execute(
            &s,
            &db,
            &a,
            "manage_jobs",
            json!({"action":"result","jobId":id})
        )
        .unwrap_err()
        .code,
        "snapshot_invalid"
    );
    let now = Utc::now().timestamp();
    s.jobs.local_cleanup(now + 7 * 86400 + 1).unwrap();
    let j = s.jobs.state.lock().unwrap().jobs[id].clone();
    assert!(j.rows.is_empty());
    assert!(j.arguments.is_null());
    s.jobs.local_cleanup(now + 30 * 86400 + 1).unwrap();
    assert!(!s.jobs.state.lock().unwrap().jobs.contains_key(id));
}
#[test]
fn asynchronous_export_requires_intent_and_publishing_recovery_never_generates_again() {
    let (db, s, a, _) = fixture();
    db.save_task(input("=unsafe-formula", "A")).unwrap();
    let arguments = json!({"action":"export","idempotencyKey":"p5-test-export","intent":{"summary":"用户明确导出","explicitUserRequest":true},"reason":"合成验收导出","format":"csv","projection":["id","title"]});
    let mut bad = arguments.clone();
    bad["intent"]["explicitUserRequest"] = json!(false);
    assert!(service::execute(
        &s,
        &db,
        &a,
        "manage_jobs",
        json!({"action":"start","kind":"export","arguments":bad})
    )
    .is_err());
    let start = call(
        &s,
        &db,
        &a,
        json!({"action":"start","kind":"export","arguments":arguments}),
    );
    let complete = wait(&s, start["job"]["jobId"].as_str().unwrap());
    assert_eq!(complete.state, "completed");
    assert!(complete.publish_started);
    assert!(complete.rows.is_empty());
    let receipt = complete.summary.clone().unwrap();
    let id = seed_interrupted(&s, complete);
    let recovered = reopen(&s, &db);
    let finished = wait(&recovered, &id);
    assert_eq!(finished.state, "completed");
    assert_eq!(
        finished.summary.as_ref().unwrap()["artifactName"],
        receipt["artifactName"]
    );
    assert_eq!(finished.summary.as_ref().unwrap()["replayed"], true);
    let synthetic_export = s
        .jobs
        .root
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("backups/mcp-created")
        .join(receipt["artifactName"].as_str().unwrap());
    let bytes = std::fs::read_to_string(synthetic_export).unwrap();
    assert!(bytes.contains("'=unsafe-formula"));
}
#[test]
fn cursor_survives_restart_metadata_only_and_detects_index_loss() {
    let (db, s, a, _) = fixture();
    let t = db.save_task(input("secret-one", "A")).unwrap();
    let first = service::execute(
        &s,
        &db,
        &a,
        "query_changes",
        json!({"projection":["id","title"]}),
    )
    .unwrap();
    let root = s.jobs.root.parent().unwrap();
    let token = first["nextCursor"].as_str().unwrap();
    let metadata =
        std::fs::read_to_string(root.join("changes").join(format!("{token}.json"))).unwrap();
    assert!(!metadata.contains("secret-one"));
    assert!(!metadata.contains(&a.token));
    let reopened = Security::open_at(root.to_owned()).unwrap();
    let mut edit = input("edited", "A");
    edit.id = Some(t.id);
    db.save_task(edit).unwrap();
    let delta = service::execute(
        &reopened,
        &db,
        &a,
        "query_changes",
        json!({"projection":["id","title"],"cursor":token}),
    )
    .unwrap();
    assert!(!delta["items"].as_array().unwrap().is_empty());
    let path = root.parent().unwrap().join("inline.db");
    rusqlite::Connection::open(path).unwrap().execute("DELETE FROM mcp_change_events WHERE sequence=(SELECT MIN(sequence) FROM mcp_change_events)",[]).unwrap();
    assert_eq!(
        service::execute(
            &reopened,
            &db,
            &a,
            "query_changes",
            json!({"projection":["id","title"],"cursor":delta["nextCursor"]})
        )
        .unwrap_err()
        .code,
        "resync_required"
    );
}
#[test]
fn private_jobs_dedup_waiter_cancel_result_and_revocation() {
    let (db, s, a, b) = fixture();
    db.save_task(input("one", "A")).unwrap();
    db.save_task(input("outside", "B")).unwrap();
    let start = call(
        &s,
        &db,
        &a,
        json!({"action":"start","kind":"query_tasks","arguments":{}}),
    );
    let id = start["job"]["jobId"].as_str().unwrap();
    let j = wait(&s, id);
    assert_eq!(j.state, "completed");
    assert_eq!(j.rows.len(), 1);
    let shared = call(
        &s,
        &db,
        &a,
        json!({"action":"start","kind":"query_tasks","arguments":{}}),
    );
    assert_eq!(shared["job"]["jobId"], start["job"]["jobId"]);
    assert_eq!(shared["reused"], true);
    call(
        &s,
        &db,
        &a,
        json!({"action":"cancelWaiter","jobId":id,"waiterId":start["waiterId"]}),
    );
    assert_eq!(
        service::execute(
            &s,
            &db,
            &a,
            "manage_jobs",
            json!({"action":"result","jobId":id,"waiterId":start["waiterId"]})
        )
        .unwrap_err()
        .code,
        "waiter_cancelled"
    );
    let result = call(
        &s,
        &db,
        &a,
        json!({"action":"result","jobId":id,"waiterId":shared["waiterId"]}),
    );
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        service::execute(
            &s,
            &db,
            &b,
            "manage_jobs",
            json!({"action":"get","jobId":id})
        )
        .unwrap_err()
        .code,
        "not_found"
    );
    let separate = call(
        &s,
        &db,
        &b,
        json!({"action":"start","kind":"query_tasks","arguments":{}}),
    );
    assert_ne!(separate["job"]["jobId"], start["job"]["jobId"]);
    wait(&s, separate["job"]["jobId"].as_str().unwrap());
    s.update_client(&a.client_id, Permissions::default(), Scope::default())
        .unwrap();
    assert_eq!(
        service::execute(
            &s,
            &db,
            &a,
            "manage_jobs",
            json!({"action":"result","jobId":id})
        )
        .unwrap_err()
        .code,
        "authorization_changed"
    );
}
#[test]
fn long_incremental_chain_releases_consumed_cursor_ancestors() {
    let (db, s, a, _) = fixture();
    let t = db.save_task(input("one", "A")).unwrap();
    let first = service::execute(
        &s,
        &db,
        &a,
        "query_changes",
        json!({"projection":["id","title"],"limit":100}),
    )
    .unwrap();
    let mut token = first["nextCursor"].clone();
    for n in 0..40 {
        let mut edit = input(&format!("edit-{n}"), "A");
        edit.id = Some(t.id);
        db.save_task(edit).unwrap();
        loop {
            let page = service::execute(
                &s,
                &db,
                &a,
                "query_changes",
                json!({"projection":["id","title"],"limit":100,"cursor":token}),
            )
            .unwrap();
            token = page["nextCursor"].clone();
            if page["hasMore"] == false {
                break;
            }
        }
    }
    let root = s.jobs.root.parent().unwrap().join("changes");
    assert!(durable::ids(&root).unwrap().len() <= 2);
}
#[test]
fn background_report_matches_existing_statistical_definition() {
    let (db, s, a, _) = fixture();
    let task = db.save_task(input("report", "A")).unwrap();
    db.record_work_event(serde_json::from_value(json!({"taskId":task.id,"resultStatus":"completed","handledAt":"2026-08-08T10:00:00+08:00","note":"sensitive-work-note","syncStatus":true})).unwrap()).unwrap();
    let args = json!({"startDate":"2026-08-08","endDate":"2026-08-08","timezoneOffsetMinutes":480});
    let synchronous = service::execute(&s, &db, &a, "get_report_summary", args.clone()).unwrap();
    let start = call(
        &s,
        &db,
        &a,
        json!({"action":"start","kind":"get_report_summary","arguments":args}),
    );
    let job = wait(&s, start["job"]["jobId"].as_str().unwrap());
    assert_eq!(job.state, "completed");
    assert_eq!(
        job.summary.as_ref().unwrap()["statistics"],
        synchronous["statistics"]
    );
    assert_eq!(
        job.summary.as_ref().unwrap()["definition"],
        synchronous["definition"]
    );
}
#[test]
fn publishing_recovery_without_receipt_does_not_reserve_or_write_a_file() {
    let (db, s, a, _) = fixture();
    let start = call(
        &s,
        &db,
        &a,
        json!({"action":"start","kind":"query_tasks","arguments":{}}),
    );
    let mut completed = wait(&s, start["job"]["jobId"].as_str().unwrap());
    completed.kind = "export".into();
    completed.publish_started = true;
    completed.arguments = json!({"action":"export","idempotencyKey":"must-not-reserve","intent":{"summary":"合成导出指令","explicitUserRequest":true},"reason":"合成中断","format":"json"});
    let id = seed_interrupted(&s, completed);
    let reopened = reopen(&s, &db);
    let failed = wait(&reopened, &id);
    assert_eq!(failed.error.as_deref(), Some("result_unknown"));
    let path = s
        .jobs
        .root
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("inline.db");
    let conn = rusqlite::Connection::open(path).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM mcp_artifact_jobs WHERE request_key='must-not-reserve'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}
#[test]
fn writes_cannot_be_scheduled_and_explicit_requery_is_independent() {
    let (db, s, a, _) = fixture();
    for kind in ["mutate_task", "batch_mutate", "backup", "restore_database"] {
        assert_eq!(
            service::execute(
                &s,
                &db,
                &a,
                "manage_jobs",
                json!({"action":"start","kind":kind,"arguments":{}})
            )
            .unwrap_err()
            .code,
            "unsupported"
        );
    }
    let first = call(
        &s,
        &db,
        &a,
        json!({"action":"start","kind":"query_tasks","arguments":{}}),
    );
    wait(&s, first["job"]["jobId"].as_str().unwrap());
    let second = call(
        &s,
        &db,
        &a,
        json!({"action":"start","kind":"query_tasks","arguments":{},"explicitRequery":true}),
    );
    assert_ne!(first["job"]["jobId"], second["job"]["jobId"]);
    wait(&s, second["job"]["jobId"].as_str().unwrap());
}
#[test]
fn limited_jobs_cancel_and_cleanup_preserves_running_checkpoints() {
    let (db, s, a, _) = fixture();
    let authorization = auth(&s, &a);
    {
        let mut limits = s.limits.lock().unwrap();
        for _ in 0..61 {
            let _ = limits.check(format!("valid:{}", a.client_id), true, Instant::now());
        }
    }
    let first = s
        .jobs
        .execute(
            &db,
            &a.client_id,
            &authorization,
            args(json!({"action":"start","kind":"query_tasks","arguments":{}})),
        )
        .unwrap();
    let id = first["job"]["jobId"].as_str().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let j = s.jobs.state.lock().unwrap().jobs[id].clone();
    assert_eq!(j.state, "paused");
    assert!(j.first_paused.is_some());
    assert_eq!(
        s.jobs
            .execute(
                &db,
                &a.client_id,
                &authorization,
                args(json!({"action":"cancelJob","jobId":id}))
            )
            .unwrap_err()
            .code,
        "explicit_intent_required"
    );
    s.jobs
        .local_cleanup(Utc::now().timestamp() + 40 * 86400)
        .unwrap();
    assert!(s.jobs.state.lock().unwrap().jobs.contains_key(id));
    s.jobs.local_cancel(id).unwrap();
    assert_eq!(wait(&s, id).state, "cancelled");
    let out = s
        .jobs
        .local_cleanup(Utc::now().timestamp() + 31 * 86400)
        .unwrap();
    assert_eq!(out["summariesRemoved"], 1);
}
#[test]
fn changes_capture_gui_events_leave_scope_without_leaking_body_and_bind_client() {
    let (db, s, a, b) = fixture();
    let t = db.save_task(input("one", "A")).unwrap();
    let call = |c: &Credentials, v: Value| service::execute(&s, &db, c, "query_changes", v);
    let full = call(&a, json!({"projection":["id","title"]})).unwrap();
    assert_eq!(full["fullSyncComplete"], true);
    assert_eq!(full["items"].as_array().unwrap().len(), 1);
    let token = full["nextCursor"].clone();
    assert_eq!(
        call(&b, json!({"projection":["id","title"],"cursor":token}))
            .unwrap_err()
            .code,
        "cursor_invalid"
    );
    let mut edit = input("secret moved title", "B");
    edit.id = Some(t.id);
    db.save_task(edit).unwrap();
    let delta = call(&a, json!({"projection":["id","title"],"cursor":token})).unwrap();
    assert_eq!(delta["items"].as_array().unwrap().len(), 1);
    assert_eq!(delta["items"][0], json!({"kind":"removed","taskId":t.id}));
    assert!(!delta.to_string().contains("secret moved title"));
    let replay = call(&a, json!({"projection":["id","title"],"cursor":token})).unwrap();
    assert_eq!(replay["items"], delta["items"]);
    let next = call(
        &a,
        json!({"projection":["id","title"],"cursor":delta["nextCursor"]}),
    )
    .unwrap();
    assert!(next["items"].as_array().unwrap().is_empty());
}
#[test]
fn full_sync_pagination_generation_and_event_modes() {
    let (db, s, a, _) = fixture();
    let t = db.save_task(input("one", "A")).unwrap();
    db.save_task(input("two", "A")).unwrap();
    let first = service::execute(
        &s,
        &db,
        &a,
        "query_changes",
        json!({"limit":1,"mode":"current"}),
    )
    .unwrap();
    assert_eq!(first["hasMore"], true);
    let second = service::execute(
        &s,
        &db,
        &a,
        "query_changes",
        json!({"limit":1,"mode":"current","cursor":first["nextCursor"]}),
    )
    .unwrap();
    assert_eq!(second["fullSyncComplete"], true);
    assert_ne!(first["items"][0]["taskId"], second["items"][0]["taskId"]);
    let mut edit = input("edited", "A");
    edit.id = Some(t.id);
    db.save_task(edit).unwrap();
    let delta = service::execute(
        &s,
        &db,
        &a,
        "query_changes",
        json!({"limit":1,"mode":"current","cursor":second["nextCursor"]}),
    )
    .unwrap();
    assert_eq!(delta["items"].as_array().unwrap().len(), 1);
    assert_eq!(delta["items"][0]["currentTask"]["title"], "edited");
    assert!(delta["items"][0]["currentTask"]["taskVersion"]
        .as_i64()
        .is_some());
    let synthetic_path = s
        .jobs
        .root
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("inline.db");
    rusqlite::Connection::open(synthetic_path)
        .unwrap()
        .execute(
            "UPDATE mcp_data_basis SET data_generation=data_generation+1",
            [],
        )
        .unwrap();
    assert_eq!(
        service::execute(
            &s,
            &db,
            &a,
            "query_changes",
            json!({"limit":1,"mode":"current","cursor":delta["nextCursor"]})
        )
        .unwrap_err()
        .code,
        "resync_required"
    );
}
