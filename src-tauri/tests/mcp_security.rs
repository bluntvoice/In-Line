#![cfg(windows)]
use in_line_lib::{
    database::Database,
    mcp::{
        contract::Credentials,
        security::{Grant, Permissions, Scope, Security},
        service,
    },
    models::{TaskInput, WorkEventInput},
};
use serde_json::{json, Value};
use std::sync::Arc;
fn input(title: &str, departments: Vec<&str>, kind: &str) -> TaskInput {
    serde_json::from_value(json!({"title":title,"department":departments[0],"departments":departments,"contact":"真实但私密","contacts":["真实但私密"],"taskType":kind,"details":"正文隐私","internalNotes":"内部隐私","status":"pending","priority":"normal","workload":"standard","isUrgent":false,"urgentRequester":"","urgentReason":""})).unwrap()
}
fn credentials(security: &Security, name: &str, departments: Vec<&str>, kind: &str) -> Credentials {
    let issued = security
        .grant(Grant {
            name: name.into(),
            permissions: Permissions::default(),
            scope: Scope {
                departments: Some(departments.into_iter().map(String::from).collect()),
                task_types: Some(vec![kind.into()]),
            },
        })
        .unwrap();
    Credentials {
        client_id: issued.client_id,
        token: issued.token,
    }
}
fn args() -> Value {
    json!({"startDate":"2026-08-08","endDate":"2026-08-08"})
}
#[test]
fn scope_filters_summary_denominator_trend_details_and_cross_client_calls() {
    let root = std::env::temp_dir().join(format!(
        "inline-mcp-scope-{}",
        in_line_lib::mcp::platform::random_secret().unwrap()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let db = Arc::new(Database::open_at(root.join("inline.db")).unwrap());
    let security = Arc::new(Security::open_at(root.join("security")).unwrap());
    for (title, depts, kind) in [
        ("仅A", vec!["A"], "T"),
        ("仅B", vec!["B"], "T"),
        ("跨部门", vec!["A", "B"], "T"),
        ("其他类型", vec!["A"], "U"),
    ] {
        let task = db.save_task(input(title, depts, kind)).unwrap();
        db.record_work_event(WorkEventInput {
            task_id: task.id,
            result_status: "completed".into(),
            handled_at: "2026-08-08T09:00:00+08:00".into(),
            note: "敏感办理说明".into(),
            sync_status: true,
        })
        .unwrap();
    }
    let a = credentials(&security, "A客户端", vec!["A"], "T");
    let b = credentials(&security, "B客户端", vec!["B"], "T");
    for (creds, title) in [(a.clone(), "仅A"), (b.clone(), "仅B")] {
        let summary =
            service::execute(&security, &db, &creds, "get_report_summary", args()).unwrap();
        assert_eq!(summary["statistics"]["summary"]["handledTasks"], 1);
        assert_eq!(summary["statistics"]["summary"]["rateDenominator"], 1);
        assert_eq!(summary["statistics"]["trend"][0]["handledTasks"], 1);
        let page = service::execute(&security, &db, &creds, "list_report_items", args()).unwrap();
        assert_eq!(page["page"]["total"], 1);
        assert_eq!(page["page"]["items"][0]["title"], title);
        // P2 omits unauthorized text, instead of representing it as an empty value.
        assert!(page["page"]["items"][0]["workEvents"][0]
            .get("note")
            .is_none());
        assert!(!page.to_string().contains("敏感办理说明"));
        assert!(!page.to_string().contains("跨部门"));
        assert!(!page.to_string().contains("内部隐私"));
    }
    let spoof = Credentials {
        client_id: a.client_id.clone(),
        token: b.token.clone(),
    };
    assert_eq!(
        service::execute(&security, &db, &spoof, "get_capabilities", json!({}))
            .unwrap_err()
            .code,
        "unauthenticated"
    );
    let revision = Security::read_at(&root.join("security")).unwrap().revision;
    let previously_read =
        service::execute(&security, &db, &a, "list_report_items", args()).unwrap();
    security
        .update_client(
            &a.client_id,
            Permissions::default(),
            Scope {
                departments: Some(vec!["B".into()]),
                task_types: Some(vec!["T".into()]),
            },
        )
        .unwrap();
    assert_eq!(
        service::recheck_result(&security, &a, revision, Ok(previously_read))
            .unwrap_err()
            .code,
        "authorization_changed"
    );
    security
        .update_client(
            &a.client_id,
            Permissions::default(),
            Scope {
                departments: Some(vec!["A".into()]),
                task_types: Some(vec!["T".into()]),
            },
        )
        .unwrap();
    let mut threads = vec![];
    for creds in [a.clone(), b.clone()] {
        let s = security.clone();
        let d = db.clone();
        threads.push(std::thread::spawn(move || {
            for _ in 0..8 {
                assert_eq!(
                    service::execute(&s, &d, &creds, "get_report_summary", args()).unwrap()
                        ["statistics"]["summary"]["handledTasks"],
                    1
                );
            }
        }));
    }
    for thread in threads {
        thread.join().unwrap();
    }
    let (start, end, tz) =
        in_line_lib::mcp::contract::report_range("2026-08-08", "2026-08-08").unwrap();
    assert_eq!(
        db.statistics(start, end, tz).unwrap().summary.handled_tasks,
        4
    ); // GUI remains unrestricted after MCP scope.
    security
        .update_client(
            &a.client_id,
            Permissions {
                regular_read: false,
                full_read: true,
                write: true,
            },
            Scope::default(),
        )
        .unwrap();
    assert_eq!(
        service::execute(&security, &db, &a, "get_report_summary", args())
            .unwrap_err()
            .code,
        "forbidden"
    );
    assert_eq!(
        service::execute(&security, &db, &a, "restore_database", json!({}))
            .unwrap_err()
            .code,
        "unsupported"
    );
    let caps = service::execute(&security, &db, &a, "get_capabilities", json!({})).unwrap();
    assert_eq!(caps["permissions"]["write"], false);
    assert_eq!(caps["writeTools"], json!([]));
}
#[test]
fn scope_all_departments_missing_departments_and_literal_names() {
    let scope = Scope {
        departments: Some(vec!["A".into(), "O'Brien".into()]),
        task_types: Some(vec!["T".into()]),
    };
    assert!(scope.allows(r#"["A","O'Brien"]"#, "T"));
    assert!(!scope.allows(r#"["A","B"]"#, "T"));
    assert!(!scope.allows("[]", "T"));
    assert!(!scope.allows("", "T"));
    assert!(!scope.allows("A", "U"));
    assert!(Scope::default().allows("", "U"));
    let empty = Scope {
        departments: Some(vec![]),
        task_types: None,
    };
    assert!(!empty.allows("A", "T"));
}
