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
use serde_json::json;
use std::sync::Arc;

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
    let tools = client.list_all_tools().await?;
    assert_eq!(tools.len(), 3);
    assert!(tools.iter().any(|tool| tool.name == "get_capabilities"));
    assert!(tools.iter().any(|tool| tool.name == "get_report_summary"));
    assert!(tools.iter().any(|tool| tool.name == "list_report_items"));
    assert!(tools.iter().all(|tool| {
        tool.annotations
            .as_ref()
            .and_then(|annotations| annotations.read_only_hint)
            == Some(true)
    }));

    let arguments = serde_json::from_value(json!({
        "startDate": "2026-08-08",
        "endDate": "2026-08-08"
    }))?;
    let summary = client
        .call_tool(CallToolRequestParams::new("get_report_summary").with_arguments(arguments))
        .await?;
    assert_eq!(
        summary.structured_content.as_ref().unwrap()["data"]["statistics"]["summary"]["completed"],
        1
    );

    let arguments = serde_json::from_value(json!({
        "startDate": "2026-08-08",
        "endDate": "2026-08-08",
        "limit": 100,
        "offset": 0
    }))?;
    let details = client
        .call_tool(CallToolRequestParams::new("list_report_items").with_arguments(arguments))
        .await?;
    assert_eq!(details.is_error, Some(false));
    let details_json = details.structured_content.unwrap().to_string();
    assert!(details_json.contains("完成 MCP 接入"));
    assert!(details_json.contains("完成只读 MCP 协议验证"));
    assert!(!details_json.contains("不应暴露的联系人"));
    assert!(!details_json.contains("不应暴露的事项详情"));
    assert!(!details_json.contains("不应暴露的内部备注"));

    // Each old tool re-checks current permissions on an already-connected stdio session.
    security.set_groups(Permissions::default())?;
    let regular = client
        .call_tool(
            CallToolRequestParams::new("list_report_items").with_arguments(serde_json::from_value(
                json!({"startDate":"2026-08-08","endDate":"2026-08-08"}),
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

    client.cancel().await?;
    let _ = std::fs::remove_dir_all(root);
    Ok(())
}
