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
    drop(database);

    let transport = TokioChildProcess::new(
        tokio::process::Command::new(env!("CARGO_BIN_EXE_in-line-mcp")).configure(|command| {
            command.env("IN_LINE_MCP_DATABASE_PATH", &path);
        }),
    )?;
    let client = ().serve(transport).await?;
    let tools = client.list_all_tools().await?;
    assert_eq!(tools.len(), 2);
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
        summary.structured_content.as_ref().unwrap()["statistics"]["summary"]["completed"],
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
    let details_json = details.structured_content.unwrap().to_string();
    assert!(details_json.contains("完成 MCP 接入"));
    assert!(details_json.contains("完成只读 MCP 协议验证"));
    assert!(!details_json.contains("不应暴露的联系人"));
    assert!(!details_json.contains("不应暴露的事项详情"));
    assert!(!details_json.contains("不应暴露的内部备注"));

    client.cancel().await?;
    let _ = std::fs::remove_dir_all(root);
    Ok(())
}
