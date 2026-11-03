use in_line_lib::mcp::{
    contract::Credentials,
    security::{Grant, Permissions, Scope, Security},
};
use in_line_lib::{
    database::Database,
    models::{TaskInput, WorkEventInput},
};
use rmcp::{
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde_json::{json, Value};
use std::sync::Arc;

// P3-A evidence is restricted to explicitly selected synthetic facts. Never log
// raw protocol responses, config, credentials or credential digests.
fn p3a_fact(tool: &str, action: &str, expected: Value, actual: Value) {
    use std::io::Write;
    let passed = actual == expected;
    writeln!(
        std::io::stdout().lock(),
        "P3A_EVIDENCE {}",
        json!({
            "tool":tool,"action":action,"fixture":"one-completed-task",
            "expected":expected,"actual":actual,"passed":passed
        })
    )
    .unwrap();
    assert!(passed, "P3-A synthetic fact mismatch: {tool}/{action}");
}

#[tokio::test]
async fn stdio_server_lists_and_calls_read_only_report_tools(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "inline-mcp-protocol-test-{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap()
    ));
    std::fs::create_dir_all(&root)?;
    let path = root.join("inline.db");
    let database = Database::open_at(path.clone()).map_err(std::io::Error::other)?;
    let task = database
        .save_task(TaskInput {
            planned_date: None,
            confirm_schedule_change: false,
            id: None,
            department: "产品组".into(),
            departments: vec!["产品组".into()],
            contact: "不应暴露的联系人".into(),
            contacts: vec!["不应暴露的联系人".into()],
            task_type: "功能开发".into(),
            title: "完成 MCP 接入".into(),
            details: "不应暴露的事项详情".into(),
            status: "pending".into(),
            priority: "normal".into(),
            workload: "complex".into(),
            is_urgent: false,
            urgent_requester: String::new(),
            urgent_reason: String::new(),
            requested_deadline: None,
            requested_deadline_label: None,
            internal_notes: "不应暴露的内部备注".into(),
        })
        .map_err(std::io::Error::other)?;
    database
        .record_work_event(WorkEventInput {
            task_id: task.id,
            result_status: "completed".into(),
            handled_at: "2026-08-08T09:00:00+08:00".into(),
            note: "完成只读 MCP 协议验证".into(),
            sync_status: true,
        })
        .map_err(std::io::Error::other)?;
    let database = Arc::new(database);
    let security_root = root.join("security");
    let security = Arc::new(Security::open_at(security_root.clone())?);
    let issued = security.grant(Grant {
        name: "stdio测试".into(),
        permissions: Permissions {
            regular_read: true,
            full_read: true,
            write: false,
        },
        scope: Scope::default(),
    })?;
    security.set_groups(Permissions {
        regular_read: true,
        full_read: true,
        write: false,
    })?;
    // Keep the existing four-snapshot per-client limit. Boundary probes use
    // an independently granted read-only sibling, initialized before reads.
    let calendar_issued = security.grant(Grant {
        name: "P3-A calendar boundary reader".into(),
        permissions: Permissions {
            regular_read: true,
            full_read: true,
            write: false,
        },
        scope: Scope::default(),
    })?;
    let host_security = security.clone();
    let host_db = database.clone();
    in_line_lib::mcp::ipc::start(
        security.clone(),
        Arc::new(move |credentials, tool, args| {
            in_line_lib::mcp::service::execute(&host_security, &host_db, credentials, tool, args)
        }),
    )?;

    let transport = TokioChildProcess::new(
        tokio::process::Command::new(env!("CARGO_BIN_EXE_in-line-mcp")).configure(|command| {
            command.env("IN_LINE_MCP_TEST_SECURITY_ROOT", &security_root);
            command.env(
                "IN_LINE_MCP_TEST_HOST_EXE",
                std::env::current_exe().unwrap(),
            );
            command.env("IN_LINE_MCP_CLIENT_ID", &issued.client_id);
            command.env("IN_LINE_MCP_TOKEN", &issued.token);
        }),
    )?;
    let client = ().serve(transport).await?;
    let calendar_transport = TokioChildProcess::new(
        tokio::process::Command::new(env!("CARGO_BIN_EXE_in-line-mcp")).configure(|command| {
            command
                .env("IN_LINE_MCP_TEST_SECURITY_ROOT", &security_root)
                .env(
                    "IN_LINE_MCP_TEST_HOST_EXE",
                    std::env::current_exe().unwrap(),
                )
                .env("IN_LINE_MCP_CLIENT_ID", &calendar_issued.client_id)
                .env("IN_LINE_MCP_TOKEN", &calendar_issued.token);
        }),
    )?;
    let calendar_client = ().serve(calendar_transport).await?;
    let tools = client.list_all_tools().await?;
    assert_eq!(tools.len(), 16);
    assert!(tools.iter().any(|tool| tool.name == "get_capabilities"));
    assert!(tools.iter().any(|tool| tool.name == "get_report_summary"));
    assert!(tools.iter().any(|tool| tool.name == "list_report_items"));
    for name in [
        "query_tasks",
        "query_task_history",
        "query_work_calendar",
        "manage_saved_query",
        "mutate_task",
        "manage_preferences",
        "request_undo",
        "batch_mutate",
        "manage_business_data",
        "inspect_data",
        "get_diagnostic",
        "query_changes",
        "manage_jobs",
    ] {
        assert!(tools.iter().any(|tool| tool.name == name));
    }
    assert!(tools.iter().all(|tool| {
        tool.annotations
            .as_ref()
            .and_then(|annotations| annotations.read_only_hint)
            == Some(
                ![
                    "manage_saved_query",
                    "mutate_task",
                    "manage_preferences",
                    "request_undo",
                    "batch_mutate",
                    "manage_business_data",
                    "manage_jobs",
                ]
                .contains(&tool.name.as_ref()),
            )
    }));

    // Compare business/auth state, not whole SQLite/cache files: read tools may
    // legitimately persist derived snapshots, cursors and diagnostic metadata.
    let task_before = serde_json::to_value(database.get_task(task.id).unwrap())?;
    let events_before = serde_json::to_value(database.list_work_events(task.id).unwrap())?;
    let preferences_before = database.settings().unwrap();
    let audit_before = database.mcp_audit_state().unwrap();
    let authorization_before = std::fs::read(security_root.join("authorization.dpapi"))?;
    let arguments = serde_json::from_value(json!({
        "startDate": "2026-08-08",
        "endDate": "2026-08-08",
        "timezoneOffsetMinutes": 480
    }))?;
    let summary = client
        .call_tool(CallToolRequestParams::new("get_report_summary").with_arguments(arguments))
        .await?;
    assert_eq!(
        summary.structured_content.as_ref().unwrap()["data"]["statistics"]["summary"]["completed"],
        1
    );
    let report = &summary.structured_content.as_ref().unwrap()["data"];
    let statistics = &report["statistics"];
    // Independent fixture oracle: one completed task, one type/department/day.
    // API completionRate is a ratio (1.0), not a percentage number (100).
    p3a_fact(
        "get_report_summary",
        "summary",
        json!({"handled":1,"completed":1,"numerator":1,"denominator":1,"rate":1.0}),
        json!({"handled":statistics["summary"]["handledTasks"],"completed":statistics["summary"]["completed"],
            "numerator":statistics["summary"]["rateNumerator"],"denominator":statistics["summary"]["rateDenominator"],"rate":statistics["summary"]["completionRate"]}),
    );
    p3a_fact(
        "get_report_summary",
        "dimensions",
        json!({"types":[{"taskType":"功能开发","handledTasks":1,"completed":1,"pendingFollowUp":0}],
        "departments":[{"department":"产品组","handledTasks":1,"completed":1,"pendingFollowUp":0}],"trendDays":1,"trendDate":"2026-08-08","trendHandled":1,"trendCompleted":1}),
        json!({"types":statistics["byTaskType"],"departments":statistics["byDepartment"],"trendDays":statistics["trend"].as_array().unwrap().len(),
        "trendDate":statistics["trend"][0]["periodStart"],"trendHandled":statistics["trend"][0]["handledTasks"],"trendCompleted":statistics["trend"][0]["completed"]}),
    );

    let arguments = serde_json::from_value(json!({
        "startDate": "2026-08-08",
        "endDate": "2026-08-08",
        "timezoneOffsetMinutes": 480,
        "snapshot": report["meta"]["snapshotId"],
        "limit": 100,
        "offset": 0
    }))?;
    let details = client
        .call_tool(CallToolRequestParams::new("list_report_items").with_arguments(arguments))
        .await?;
    assert_eq!(details.is_error, Some(false));
    let details_json = details.structured_content.as_ref().unwrap().to_string();
    assert!(details_json.contains("完成 MCP 接入"));
    assert!(details_json.contains("完成只读 MCP 协议验证"));
    assert!(!details_json.contains("不应暴露的联系人"));
    assert!(!details_json.contains("不应暴露的事项详情"));
    assert!(!details_json.contains("不应暴露的内部备注"));
    let items = &details.structured_content.as_ref().unwrap()["data"];
    p3a_fact(
        "list_report_items",
        "same_snapshot",
        json!({"total":1,"workEvents":1,"sameSnapshot":true,"sameBasis":true}),
        json!({"total":items["page"]["total"],"workEvents":items["page"]["items"][0]["workEvents"].as_array().unwrap().len(),
            "sameSnapshot":items["meta"]["snapshotId"]==report["meta"]["snapshotId"],"sameBasis":items["meta"]["dataVersion"]==report["meta"]["dataVersion"]}),
    );

    let history = client
        .call_tool(
            CallToolRequestParams::new("query_task_history").with_arguments(
                serde_json::from_value(json!({"taskId":task.id,"kinds":["work"]}))?,
            ),
        )
        .await?;
    assert_eq!(history.is_error, Some(false));
    let history = &history.structured_content.as_ref().unwrap()["data"];
    p3a_fact(
        "query_task_history",
        "known_work_event",
        json!({"total":1,"taskId":task.id,"status":"completed","at":"2026-08-08T09:00:00+08:00","voided":false}),
        json!({"total":history["total"],"taskId":history["items"][0]["taskId"],"status":history["items"][0]["from"],
            "at":history["items"][0]["at"],"voided":!history["items"][0]["voidedAt"].is_null()}),
    );

    // 09:00 +08 = 01:00 UTC = previous day 23:00 at -02:00.
    // Expected calendar membership is defined from this fixture timestamp,
    // independently of report/calendar results and of the runner's local zone.
    for (day, offset, expected) in [
        ("2026-08-08", 480, 1),
        ("2026-08-07", -120, 1),
        ("2026-08-08", -120, 0),
    ] {
        let reader = if offset == 480 {
            &client
        } else {
            &calendar_client
        };
        let calendar = reader
            .call_tool(
                CallToolRequestParams::new("query_work_calendar").with_arguments(
                    serde_json::from_value(
                        json!({"startDate":day,"endDate":day,"timezoneOffsetMinutes":offset}),
                    )?,
                ),
            )
            .await?;
        assert_eq!(calendar.is_error, Some(false));
        let calendar = &calendar.structured_content.as_ref().unwrap()["data"];
        let events = calendar["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["kind"] == "workEvent")
            .collect::<Vec<_>>();
        p3a_fact(
            "query_work_calendar",
            "timezone_membership",
            json!({"day":day,"offset":offset,"rounds":expected,"completed":expected,"events":expected,"fixtureOnly":true}),
            json!({"day":day,"offset":offset,"rounds":calendar["summary"]["handlingRounds"],"completed":calendar["summary"]["completedTasks"],"events":events.len(),
                "fixtureOnly":events.iter().all(|r|r["taskId"]==task.id && r["resultStatus"]=="completed" && r["handledAt"]=="2026-08-08T09:00:00+08:00")}),
        );
    }
    let full = client
        .call_tool(CallToolRequestParams::new("query_changes").with_arguments(
            serde_json::from_value(json!({"mode":"current","projection":["id","title"]}))?,
        ))
        .await?;
    assert_eq!(full.is_error, Some(false));
    let full = &full.structured_content.as_ref().unwrap()["data"];
    p3a_fact(
        "query_changes",
        "initial_full",
        json!({"complete":true,"rows":1,"kind":"current","taskId":task.id,"title":"完成 MCP 接入"}),
        json!({"complete":full["fullSyncComplete"],"rows":full["items"].as_array().unwrap().len(),"kind":full["items"][0]["kind"],"taskId":full["items"][0]["taskId"],"title":full["items"][0]["currentTask"]["title"]}),
    );
    let incremental = client
        .call_tool(CallToolRequestParams::new("query_changes").with_arguments(
            serde_json::from_value(
                json!({"mode":"current","projection":["id","title"],"cursor":full["nextCursor"]}),
            )?,
        ))
        .await?;
    assert_eq!(incremental.is_error, Some(false));
    p3a_fact(
        "query_changes",
        "idle_incremental",
        json!([]),
        incremental.structured_content.as_ref().unwrap()["data"]["items"].clone(),
    );
    // Original reader now owns report/history/calendar/query snapshots. This
    // reproduces the initial harness failure's limit without weakening it;
    // the same calendar business query above must still succeed on its sibling.
    let limited = client.call_tool(CallToolRequestParams::new("query_work_calendar")
        .with_arguments(serde_json::from_value(json!({"startDate":"2026-08-08","endDate":"2026-08-08","timezoneOffsetMinutes":-120}))?)).await?;
    assert_eq!(limited.is_error, Some(true));
    p3a_fact(
        "query_work_calendar",
        "four_snapshot_limit",
        json!("resource_limit"),
        limited.structured_content.as_ref().unwrap()["error"]["code"].clone(),
    );

    let trace = client.call_tool(CallToolRequestParams::new("inspect_data")
        .with_arguments(serde_json::from_value(json!({"action":"trace","metric":"completed","report":{"startDate":"2026-08-08","endDate":"2026-08-08","timezoneOffsetMinutes":480,"snapshot":report["meta"]["snapshotId"]}}))?)).await?;
    assert_eq!(trace.is_error, Some(false));
    let trace = &trace.structured_content.as_ref().unwrap()["data"];
    p3a_fact(
        "inspect_data",
        "trace_completed",
        json!({"value":1,"sources":1,"complete":true,"sameSnapshot":true}),
        json!({"value":trace["value"],"sources":trace["totalSources"],"complete":trace["complete"],"sameSnapshot":trace["meta"]["snapshotId"]==report["meta"]["snapshotId"]}),
    );

    p3a_fact(
        "fixture_safety",
        "read_segment_preserves_business_and_authorization",
        json!({"task":true,"events":true,"preferences":true,"businessAudit":true,"authorization":true}),
        json!({"task":serde_json::to_value(database.get_task(task.id).unwrap())?==task_before,
            "events":serde_json::to_value(database.list_work_events(task.id).unwrap())?==events_before,
            "preferences":database.settings().unwrap()==preferences_before,
            "businessAudit":database.mcp_audit_state().unwrap()==audit_before,
            "authorization":std::fs::read(security_root.join("authorization.dpapi"))?==authorization_before}),
    );

    // Each old tool re-checks current permissions on an already-connected stdio session.
    security.set_groups(Permissions::default())?;
    let regular = client
        .call_tool(
            CallToolRequestParams::new("list_report_items").with_arguments(serde_json::from_value(
                json!({"startDate":"2026-08-08","endDate":"2026-08-08","timezoneOffsetMinutes":480}),
            )?),
        )
        .await?;
    assert!(!regular
        .structured_content
        .unwrap()
        .to_string()
        .contains("完成只读 MCP 协议验证"));
    let over_limit = client
        .call_tool(
            CallToolRequestParams::new("list_report_items").with_arguments(serde_json::from_value(
                json!({"startDate":"2026-08-08","endDate":"2026-08-08","limit":101}),
            )?),
        )
        .await?;
    assert_eq!(
        over_limit.structured_content.unwrap()["error"]["code"],
        "invalid_arguments"
    );
    security.pause(true)?;
    let paused = client
        .call_tool(
            CallToolRequestParams::new("get_capabilities")
                .with_arguments(serde_json::from_value(json!({}))?),
        )
        .await?;
    assert_eq!(
        paused.structured_content.unwrap()["error"]["code"],
        "paused"
    );
    security.pause(false)?;
    security.revoke(&issued.client_id)?;
    let revoked = client
        .call_tool(
            CallToolRequestParams::new("get_report_summary").with_arguments(
                serde_json::from_value(json!({"startDate":"2026-08-08","endDate":"2026-08-08"}))?,
            ),
        )
        .await?;
    assert_eq!(
        revoked.structured_content.unwrap()["error"]["code"],
        "revoked"
    );

    let anonymous_transport = TokioChildProcess::new(
        tokio::process::Command::new(env!("CARGO_BIN_EXE_in-line-mcp")).configure(|command| {
            command
                .env("IN_LINE_MCP_TEST_SECURITY_ROOT", &security_root)
                .env_remove("IN_LINE_MCP_CLIENT_ID")
                .env_remove("IN_LINE_MCP_TOKEN");
        }),
    )?;
    let anonymous = ().serve(anonymous_transport).await?;
    for tool in [
        "get_capabilities",
        "get_report_summary",
        "list_report_items",
    ] {
        let args = if tool == "get_capabilities" {
            json!({})
        } else {
            json!({"startDate":"2026-08-08","endDate":"2026-08-08"})
        };
        let result = anonymous
            .call_tool(
                CallToolRequestParams::new(tool).with_arguments(serde_json::from_value(args)?),
            )
            .await?;
        assert_eq!(result.is_error, Some(true));
        assert_eq!(
            result.structured_content.unwrap()["error"]["code"],
            "unauthenticated"
        );
    }
    for expected in ["unauthenticated", "unauthenticated", "rate_limited"] {
        let result = anonymous
            .call_tool(
                CallToolRequestParams::new("get_capabilities")
                    .with_arguments(serde_json::from_value(json!({}))?),
            )
            .await?;
        assert_eq!(result.is_error, Some(true));
        let envelope = result.structured_content.unwrap();
        assert_eq!(envelope["error"]["code"], expected);
        assert!(envelope["data"].is_null());
        if expected == "rate_limited" {
            assert!(envelope["error"]["retryAfterSeconds"].as_u64().unwrap() > 0);
        }
    }
    anonymous.cancel().await?;

    // Restoring a business backup never restores a revoked client or paused setting.
    let backup = database
        .create_backup("manual")
        .map_err(std::io::Error::other)?;
    security.pause(true)?;
    database
        .restore_backup(backup.path)
        .map_err(std::io::Error::other)?;
    let persisted = Security::read_at(&security_root)?;
    assert!(persisted.paused);
    assert_eq!(
        persisted
            .authorize(&Credentials {
                client_id: issued.client_id,
                token: issued.token
            })
            .err()
            .unwrap()
            .code,
        "revoked"
    );

    calendar_client.cancel().await?;
    client.cancel().await?;
    let _ = std::fs::remove_dir_all(root);
    Ok(())
}
