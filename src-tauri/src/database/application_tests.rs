//! Conformance of the GUI service path and the authenticated MCP facade.
//! All databases and authorizations are synthetic and independent of production.
use super::*;
use crate::mcp::{
    contract::{report_range, Credentials},
    security::{Grant, Permissions, Scope, Security},
    service,
};
use serde_json::{json, Value as Json};

struct Fixture {
    db: Database,
    security: Security,
    credentials: Credentials,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "inline-application-{}",
            crate::mcp::platform::random_secret().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        let db = Database::open_at(root.join("synthetic.db")).unwrap();
        let security = Security::open_at(root.join("security")).unwrap();
        security.acknowledge_sensitive_notice().unwrap();
        security.set_groups(Self::permissions(true)).unwrap();
        let credentials = Self::grant(&security, true, Scope::default());
        Self {
            db,
            security,
            credentials,
        }
    }
    fn permissions(write: bool) -> Permissions {
        Permissions {
            regular_read: true,
            full_read: false,
            write,
        }
    }
    fn grant(security: &Security, write: bool, scope: Scope) -> Credentials {
        let issued = security
            .grant(Grant {
                name: "synthetic application conformance".into(),
                permissions: Self::permissions(write),
                scope,
            })
            .unwrap();
        Credentials {
            client_id: issued.client_id,
            token: issued.token,
        }
    }
    fn task(&self, title: &str, department: &str, future: bool) -> LegalTask {
        let date = (Local::now().date_naive() + chrono::Duration::days(3)).to_string();
        let input = json!({"title":title,"department":department,"departments":[department],"contact":"synthetic-private-contact","contacts":["synthetic-private-contact"],"taskType":"T","details":"synthetic-private-details","internalNotes":"","status":"pending","priority":"normal","workload":"standard","isUrgent":false,"urgentRequester":"","urgentReason":"","plannedDate":if future{Some(date)}else{None}});
        self.db
            .save_task(serde_json::from_value(input).unwrap())
            .unwrap()
    }
    fn status_request(&self, id: i64, status: &str, key: &str) -> Json {
        json!({"action":"setStatus","target":{"taskId":id},"status":status,"taskVersion":self.db.mcp_task_basis(id,false).unwrap()["taskVersion"],"idempotencyKey":key,"reason":"synthetic explicit status change","intent":{"summary":"synthetic user instruction","explicitUserRequest":true,"confirmedRealWork":true}})
    }
    fn execute(&self, tool: &str, args: Json) -> Result<Json, crate::mcp::contract::McpError> {
        service::execute(&self.security, &self.db, &self.credentials, tool, args)
    }
}

// Exact before/after dump for rollback: includes business rows, data basis,
// change indexes, receipts and audit, rather than checking only tasks.status.
fn database_rows(db: &Database) -> BTreeMap<String, Vec<Vec<String>>> {
    db.with_conn(|conn| {
        let mut table_query = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        let names = table_query
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let mut result = BTreeMap::new();
        for name in names {
            let mut q = conn
                .prepare(&format!("SELECT * FROM \"{}\"", name.replace('"', "\"\"")))
                .unwrap();
            let columns = q.column_count();
            let mut rows = q
                .query_map([], |r| {
                    (0..columns)
                        .map(|i| Ok(format!("{:?}", r.get_ref(i)?)))
                        .collect::<rusqlite::Result<Vec<_>>>()
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            rows.sort();
            result.insert(name, rows);
        }
        Ok(result)
    })
    .unwrap()
}

#[test]
fn application_phase1_summary_matches_gui_and_preserves_scope_and_future_exclusion() {
    let f = Fixture::new();
    let first = f.task("handled A", "A", false);
    let other = f.task("handled B", "B", false);
    f.task("future A", "A", true);
    f.db.set_status(first.id, "processed".into()).unwrap();
    f.db.set_status(other.id, "completed".into()).unwrap();
    let date = today();
    let (start, end, tz) = report_range(&date, &date).unwrap();
    let gui = f.db.statistics(start, end, tz).unwrap();
    let args = json!({"startDate":date,"endDate":date,"timezoneOffsetMinutes":tz});
    let mcp = f.execute("get_report_summary", args.clone()).unwrap();
    assert_eq!(mcp["statistics"], serde_json::to_value(&gui).unwrap());
    assert_eq!(gui.summary.handled_tasks, 2);
    assert_eq!(gui.summary.completed, 1);
    let restricted = Fixture::grant(
        &f.security,
        false,
        Scope {
            departments: Some(vec!["A".into()]),
            task_types: Some(vec!["T".into()]),
        },
    );
    let limited = service::execute(
        &f.security,
        &f.db,
        &restricted,
        "get_report_summary",
        args.clone(),
    )
    .unwrap();
    assert_eq!(limited["statistics"]["summary"]["handledTasks"], 1);
    assert_eq!(limited["statistics"]["summary"]["completed"], 0);
    assert!(!limited.to_string().contains("synthetic-private"));
    // Refresh after granting another client so only the following GUI write,
    // rather than an authorization revision, can invalidate this cached query.
    let cached = f.execute("get_report_summary", args.clone()).unwrap();
    let old_snapshot = cached["meta"]["snapshotId"].clone();
    f.db.set_status(first.id, "completed".into()).unwrap();
    let fresh = f.execute("get_report_summary", args).unwrap();
    assert_ne!(fresh["meta"]["snapshotId"], old_snapshot);
    assert_eq!(fresh["statistics"]["summary"]["completed"], 2);
}

#[test]
fn application_phase1_status_preserves_future_numbers_work_events_and_idempotence() {
    let gui = Fixture::new();
    let mcp = Fixture::new();
    let gui_task = gui.task("future", "A", true);
    let mcp_task = mcp.task("future", "A", true);
    for (status, key) in [
        ("processing", "start-future"),
        ("processed", "handle-future"),
    ] {
        gui.db.set_status(gui_task.id, status.into()).unwrap();
        let args = mcp.status_request(mcp_task.id, status, key);
        let result = mcp.execute("mutate_task", args.clone()).unwrap();
        assert_eq!(result["commitStatus"], "committed");
        assert_eq!(result["verificationStatus"], "verified_after_commit");
        let before_replay = database_rows(&mcp.db);
        let replay = mcp.execute("mutate_task", args).unwrap();
        assert_eq!(replay["replayed"], true);
        assert_eq!(replay["auditId"], result["auditId"]);
        assert_eq!(database_rows(&mcp.db), before_replay);
    }
    let mut a = serde_json::to_value(gui.db.get_task(gui_task.id).unwrap()).unwrap();
    let mut b = serde_json::to_value(mcp.db.get_task(mcp_task.id).unwrap()).unwrap();
    // These timestamps are generated independently by the two invocations;
    // plan dates, deadlines, queue dates, IDs and all other task fields remain.
    for field in [
        "createdAt",
        "updatedAt",
        "startedAt",
        "completedAt",
        "deferredEnteredAt",
        "scheduleActionAt",
    ] {
        a.as_object_mut().unwrap().remove(field);
        b.as_object_mut().unwrap().remove(field);
    }
    assert_eq!(a, b);
    assert_eq!(a["permanentNumber"], gui_task.permanent_number);
    assert_eq!(a["ticketDate"], today());
    assert_eq!(a["isScheduled"], false);
    assert_eq!(a["processingRounds"], 1);
    let events = mcp.db.list_work_events(mcp_task.id).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].result_status, "processed");
    assert!(events[0].handled_at.starts_with(&today()));
    for table in ["daily_sequences", "task_queue_entries"] {
        let rows = |db: &Database| {
            db.with_conn(|conn| {
                let sql = if table == "daily_sequences" {
                    "SELECT ticket_date,last_sequence FROM daily_sequences ORDER BY ticket_date"
                } else {
                    "SELECT queue_date,daily_sequence,close_reason,closed_at IS NULL FROM task_queue_entries ORDER BY id"
                };
                let mut q = conn.prepare(sql).unwrap();
                let count = q.column_count();
                Ok(q.query_map([], |r| (0..count).map(|i| Ok(format!("{:?}",r.get_ref(i)?))).collect::<rusqlite::Result<Vec<_>>>()).unwrap().collect::<Result<Vec<_>,_>>().unwrap())
            }).unwrap()
        };
        assert_eq!(rows(&gui.db), rows(&mcp.db));
    }
}

#[test]
fn application_phase1_parent_completion_rolls_back_every_table_for_both_entries() {
    for external in [false, true] {
        let f = Fixture::new();
        let parent = f.task("future parent", "A", true);
        let child = f.task("unfinished child", "A", false);
        f.db.set_parent_task(child.id, Some(parent.id)).unwrap();
        let before = database_rows(&f.db);
        if external {
            let error = f
                .execute(
                    "mutate_task",
                    f.status_request(parent.id, "completed", "parent-failure"),
                )
                .unwrap_err();
            assert_eq!(error.code, "business_rule");
            assert!(error.message.contains("未完成子任务"));
        } else {
            assert!(f
                .db
                .set_status(parent.id, "completed".into())
                .unwrap_err()
                .contains("未完成子任务"));
        }
        assert_eq!(database_rows(&f.db), before);
        assert!(f.db.get_task(parent.id).unwrap().is_scheduled);
        assert_eq!(f.db.get_task(child.id).unwrap().status, "pending");
    }
}

#[test]
fn application_phase1_external_guards_remain_after_gui_changes() {
    let f = Fixture::new();
    let a = f.task("A", "A", false);
    let b = f.task("B", "B", false);
    let stale = f.status_request(a.id, "completed", "stale-version");
    f.db.set_status(a.id, "processed".into()).unwrap();
    let before = database_rows(&f.db);
    assert_eq!(
        f.execute("mutate_task", stale).unwrap_err().code,
        "conflict"
    );
    let read_only = Fixture::grant(&f.security, false, Scope::default());
    let scoped = Fixture::grant(
        &f.security,
        true,
        Scope {
            departments: Some(vec!["A".into()]),
            task_types: Some(vec!["T".into()]),
        },
    );
    for (creds, id, expected) in [
        (&read_only, a.id, "forbidden"),
        (&scoped, b.id, "not_found"),
    ] {
        let error = service::execute(
            &f.security,
            &f.db,
            creds,
            "mutate_task",
            f.status_request(id, "completed", "guarded-status"),
        )
        .unwrap_err();
        // Out-of-scope identity is hidden, not disclosed by a permission error.
        assert_eq!(error.code, expected);
    }
    assert_eq!(database_rows(&f.db), before);
    f.security.pause(true).unwrap();
    assert_eq!(
        f.execute("mutate_task", f.status_request(a.id, "pending", "paused"))
            .unwrap_err()
            .code,
        "paused"
    );
    // Pausing external clients must not disable the user's local GUI workflow.
    f.db.set_status(a.id, "pending".into()).unwrap();
    assert_eq!(f.db.get_task(a.id).unwrap().status, "pending");
    f.security.pause(false).unwrap();
    f.security.revoke(&f.credentials.client_id).unwrap();
    assert_eq!(
        f.execute("get_capabilities", json!({})).unwrap_err().code,
        "revoked"
    );
}

#[test]
fn application_phase1_status_rejects_autocommit_before_any_domain_side_effect() {
    let f = Fixture::new();
    let task = f.task("future without transaction", "A", true);
    let before = database_rows(&f.db);
    let error =
        f.db.with_conn(|connection| {
            assert!(connection.is_autocommit());
            crate::application::set_status_in_transaction(connection, task.id, "processed".into())
        })
        .unwrap_err();
    assert_eq!(error, "事项状态修改必须在事务内执行");
    assert_eq!(database_rows(&f.db), before);
}
