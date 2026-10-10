#![cfg(windows)]
use in_line_lib::{
    database::Database,
    mcp::{
        ipc, platform,
        security::{Grant, Permissions, Scope, Security},
        service,
    },
    models::TaskInput,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn p2_real_stdio_and_authenticated_pipe_use_bound_snapshot_and_redaction() {
    let root = std::env::temp_dir().join(format!(
        "inline-p2-stdio-{}",
        platform::random_secret().unwrap()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let db = Arc::new(Database::open_at(root.join("synthetic.db")).unwrap());
    let security_root = root.join("security");
    let security = Arc::new(Security::open_at(security_root.clone()).unwrap());
    for title in ["original-first", "original-second"] {
        let input:TaskInput=serde_json::from_value(json!({"title":title,"department":"synthetic A","departments":["synthetic A"],"contact":"secret-contact","contacts":["secret-contact"],"taskType":"synthetic T","details":"secret-details","internalNotes":"secret-note","status":"pending","priority":"normal","workload":"standard","isUrgent":false,"urgentRequester":"","urgentReason":""})).unwrap();
        db.save_task(input).unwrap();
    }
    let issued = security
        .grant(Grant {
            name: "synthetic-stdio".into(),
            permissions: Permissions::default(),
            scope: Scope {
                departments: Some(vec!["synthetic A".into()]),
                task_types: Some(vec!["synthetic T".into()]),
            },
        })
        .unwrap();
    let sec = security.clone();
    let data = db.clone();
    ipc::start(
        security.clone(),
        Arc::new(move |creds, tool, args| service::execute(&sec, &data, creds, tool, args)),
    )
    .unwrap();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_in-line-mcp"))
        .env("IN_LINE_MCP_TEST_SECURITY_ROOT", &security_root)
        .env(
            "IN_LINE_MCP_TEST_HOST_EXE",
            std::env::current_exe().unwrap(),
        )
        .env("IN_LINE_MCP_CLIENT_ID", &issued.client_id)
        .env("IN_LINE_MCP_TOKEN", &issued.token)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000)
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
    let execute = async {
        async fn request(
            input: &mut tokio::process::ChildStdin,
            output: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
            id: i64,
            method: &str,
            params: Value,
        ) -> Value {
            input
                .write_all(
                    format!(
                        "{}\n",
                        json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            let line = output.next_line().await.unwrap().unwrap();
            let response: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(response["id"], id);
            response
        }
        let init=request(&mut input,&mut output,1,"initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"p2-isolation","version":"1"}})).await;
        assert!(init["result"].is_object());
        input
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
            .await
            .unwrap();
        let list = request(&mut input, &mut output, 2, "tools/list", json!({})).await;
        assert_eq!(list["result"]["tools"].as_array().unwrap().len(), 7);
        let first = request(
            &mut input,
            &mut output,
            3,
            "tools/call",
            json!({"name":"query_tasks","arguments":{"limit":1,"projection":["id","title"]}}),
        )
        .await;
        assert_eq!(first["result"]["structuredContent"]["status"], "ok");
        let page = &first["result"]["structuredContent"]["data"];
        assert_eq!(page["total"], 2);
        assert!(!first.to_string().contains("secret-"));
        db.set_task_ticket_color(2, Some("#008800".into())).unwrap();
        let second=request(&mut input,&mut output,4,"tools/call",json!({"name":"query_tasks","arguments":{"limit":1,"projection":["id","title"],"cursor":page["nextCursor"]}})).await;
        assert_eq!(
            second["result"]["structuredContent"]["data"]["items"][0]["title"],
            "original-second"
        );
        assert_eq!(
            second["result"]["structuredContent"]["data"]["meta"]["dataVersion"],
            page["meta"]["dataVersion"]
        );
        let history = request(
            &mut input,
            &mut output,
            5,
            "tools/call",
            json!({"name":"query_task_history","arguments":{"taskId":1}}),
        )
        .await;
        assert_eq!(history["result"]["structuredContent"]["status"], "ok");
        assert!(!history.to_string().contains("secret-"));
        let day = chrono::Local::now().format("%Y-%m-%d").to_string();
        let calendar = request(
            &mut input,
            &mut output,
            6,
            "tools/call",
            json!({"name":"query_work_calendar","arguments":{"startDate":day,"endDate":day}}),
        )
        .await;
        assert_eq!(calendar["result"]["structuredContent"]["status"], "ok");
        assert!(calendar["result"]["structuredContent"]["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["kind"] == "queueTask"));
        let saved=request(&mut input,&mut output,7,"tools/call",json!({"name":"manage_saved_query","arguments":{"action":"save","name":"denied","explicitIntent":true}})).await;
        assert_eq!(
            saved["result"]["structuredContent"]["error"]["code"],
            "explicit_intent_required"
        );
        assert_eq!(saved["result"]["isError"], true);
    };
    tokio::time::timeout(std::time::Duration::from_secs(30), execute)
        .await
        .unwrap();
    child.kill().await.unwrap();
    child.wait().await.unwrap();
}
