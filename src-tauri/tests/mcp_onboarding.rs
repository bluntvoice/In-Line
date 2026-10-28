#![cfg(windows)]
use in_line_lib::{
    database::Database,
    mcp::{
        contract::Credentials,
        ipc,
        onboarding::{self, PrepareArgs},
        platform,
        security::{Permissions, Scope, Security},
        service,
    },
};
use serde_json::{json, Value};
use std::sync::Arc;

#[tokio::test]
async fn existing_stdio_connection_refreshes_authorized_identity_without_restarting() {
    use rmcp::{model::CallToolRequestParams, transport::TokioChildProcess, ServiceExt};
    let root = std::env::temp_dir().join(format!(
        "inline-live-credentials-{}",
        platform::random_secret().unwrap()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let security_root = root.join("security");
    let home = root.join("codex");
    std::fs::create_dir_all(&home).unwrap();
    let security = Arc::new(Security::open_at(security_root.clone()).unwrap());
    let db = Arc::new(Database::open_at(root.join("synthetic.db")).unwrap());
    let sec = security.clone();
    let database = db.clone();
    ipc::start(
        security.clone(),
        Arc::new(move |c, t, a| service::execute(&sec, &database, c, t, a)),
    )
    .unwrap();
    let exe = std::path::Path::new(env!("CARGO_BIN_EXE_in-line-mcp"));
    let test_exe = std::env::current_exe().unwrap();
    let mut bare = tokio::process::Command::new(exe);
    bare.env("IN_LINE_MCP_TEST_SECURITY_ROOT", &security_root)
        .env("IN_LINE_MCP_TEST_HOST_EXE", &test_exe)
        .env("IN_LINE_MCP_TEST_CLIENT_EXE", &test_exe)
        .env_remove("IN_LINE_MCP_CLIENT_ID")
        .env_remove("IN_LINE_MCP_TOKEN")
        .creation_flags(0x08000000);
    let client = ().serve(TokioChildProcess::new(bare).unwrap()).await.unwrap();
    let request =
        || CallToolRequestParams::new("get_capabilities").with_arguments(serde_json::Map::new());
    assert_eq!(
        client
            .call_tool(request())
            .await
            .unwrap()
            .structured_content
            .unwrap()["error"]["code"],
        "unauthenticated"
    );
    let original=format!("# keep original settings\nmodel='keep-model'\n[mcp_servers.in_line]\ncommand={}\nargs=[]\n",toml_edit::Value::from(exe.to_str().unwrap()));
    std::fs::write(home.join("config.toml"), original).unwrap();
    let now = chrono::Utc::now().timestamp();
    let receipt = onboarding::prepare_at(
        &security,
        &security_root,
        exe,
        PrepareArgs {
            client: "codex".into(),
            permissions: Permissions::default(),
            scope: Scope::default(),
        },
        now,
    )
    .unwrap();
    let run = |id: String| {
        let mut command = tokio::process::Command::new(exe);
        command
            .args(["--import-client", &id])
            .env("IN_LINE_MCP_TEST_SECURITY_ROOT", &security_root)
            .env("IN_LINE_MCP_TEST_CODEX_HOME", &home)
            .env("IN_LINE_MCP_TEST_HOST_EXE", &test_exe)
            .env("IN_LINE_MCP_TEST_CLIENT_EXE", &test_exe)
            .creation_flags(0x08000000);
        command
    };
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        run(receipt.package_id).output(),
    )
    .await
    .unwrap()
    .unwrap();
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        output.status.success(),
        "redacted importer response: {response}"
    );
    assert!(output.stderr.is_empty());
    assert_eq!(response["data"]["configurationVerified"], true);
    assert_eq!(response["data"]["currentSessionVerified"], false);
    assert_eq!(response["data"]["sessionCredentialRefreshPrepared"], true);
    // Reuse the original client/stdio transport, never replace it with the verifier child.
    let active = client
        .call_tool(request())
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(active["status"], "ok");
    assert_eq!(active["data"]["clientId"], receipt.client_id);
    assert_eq!(
        active["data"]["compatibility"]["sessionCredentialsRefreshed"],
        true
    );
    assert_eq!(
        active["data"]["compatibility"]["currentSessionAutoImportVerified"],
        true
    );
    assert_eq!(
        active["data"]["credentialResolution"],
        json!({"source":"runtimeBinding","changed":true})
    );
    assert_eq!(active["data"]["verificationScope"], "thisToolCall");
    let repeated = client
        .call_tool(request())
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(
        repeated["data"]["credentialResolution"],
        json!({"source":"runtimeBinding","changed":false})
    );
    assert_eq!(
        repeated["data"]["compatibility"]["sessionCredentialsRefreshed"],
        true
    );
    assert_eq!(
        active["data"]["permissions"],
        json!({"regularRead":true,"fullRead":false,"write":false})
    );
    security.pause(true).unwrap();
    assert_eq!(
        client
            .call_tool(request())
            .await
            .unwrap()
            .structured_content
            .unwrap()["error"]["code"],
        "paused"
    );
    security.pause(false).unwrap();
    let rotated = onboarding::rotate_at(
        &security,
        &security_root,
        exe,
        &receipt.client_id,
        chrono::Utc::now().timestamp(),
    )
    .unwrap();
    assert_eq!(
        client
            .call_tool(request())
            .await
            .unwrap()
            .structured_content
            .unwrap()["error"]["code"],
        "unauthenticated"
    );
    let rotated_output = run(rotated.package_id).output().await.unwrap();
    assert!(
        rotated_output.status.success(),
        "redacted rotation result: {}",
        String::from_utf8_lossy(&rotated_output.stdout)
    );
    assert_eq!(
        client
            .call_tool(request())
            .await
            .unwrap()
            .structured_content
            .unwrap()["status"],
        "ok"
    );
    security.revoke(&receipt.client_id).unwrap();
    assert_eq!(
        client
            .call_tool(request())
            .await
            .unwrap()
            .structured_content
            .unwrap()["error"]["code"],
        "revoked"
    );
    client.cancel().await.unwrap();
    // Keep synthetic fixtures if the asynchronous host still retains a handle.
    let _ = db;
    let _ = root;
}

#[tokio::test]
async fn actual_importer_config_and_sidecar_verify_identity_intersection_scope_without_claiming_current_session(
) {
    let root = std::env::temp_dir().join(format!(
        "inline-onboarding-stdio-{}",
        platform::random_secret().unwrap()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let security_root = root.join("security");
    let home = root.join("codex");
    std::fs::create_dir_all(&home).unwrap();
    let db = Arc::new(Database::open_at(root.join("synthetic.db")).unwrap());
    let security = Arc::new(Security::open_at(security_root.clone()).unwrap());
    let sec = security.clone();
    let database = db.clone();
    ipc::start(
        security.clone(),
        Arc::new(move |creds, tool, args| service::execute(&sec, &database, creds, tool, args)),
    )
    .unwrap();
    let now = chrono::Utc::now().timestamp();
    let receipt = onboarding::prepare_at(
        &security,
        &security_root,
        std::path::Path::new(env!("CARGO_BIN_EXE_in-line-mcp")),
        PrepareArgs {
            client: "codex".into(),
            permissions: Permissions {
                regular_read: true,
                full_read: true,
                write: false,
            },
            scope: Scope {
                departments: Some(vec!["synthetic A".into()]),
                task_types: Some(vec!["synthetic T".into()]),
            },
        },
        now,
    )
    .unwrap();
    let ticket_file = security_root
        .join("onboarding")
        .join(&receipt.package_id)
        .join("ticket.dpapi");
    let ticket: Value = serde_json::from_slice(
        &platform::seal(&std::fs::read(&ticket_file).unwrap(), true).unwrap(),
    )
    .unwrap();
    let token = ticket["credentials"]["token"].as_str().unwrap();
    let original="# synthetic configuration\nmodel = 'keep-model'\n[mcp_servers.other]\ncommand = 'keep-program'\nargs = []\n";
    std::fs::write(home.join("config.toml"), original).unwrap();
    let run = || {
        let mut cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_in-line-mcp"));
        cmd.args(["--import-client", &receipt.package_id])
            .env("IN_LINE_MCP_TEST_SECURITY_ROOT", &security_root)
            .env("IN_LINE_MCP_TEST_CODEX_HOME", &home)
            .env(
                "IN_LINE_MCP_TEST_HOST_EXE",
                std::env::current_exe().unwrap(),
            )
            .creation_flags(0x08000000);
        cmd
    };
    let output = tokio::time::timeout(std::time::Duration::from_secs(30), run().output())
        .await
        .unwrap()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        output.status.success(),
        "redacted importer result: {stdout}"
    );
    assert!(stderr.is_empty());
    assert_eq!(stdout.lines().count(), 1);
    assert!(!stdout.contains(token));
    assert!(!stderr.contains(token));
    let response: Value = serde_json::from_str(&stdout).unwrap();
    let data = &response["data"];
    assert_eq!(response["status"], "ok");
    assert_eq!(data["clientId"], receipt.client_id);
    assert_eq!(data["configurationVerified"], true);
    assert_eq!(data["currentSessionVerified"], false);
    assert_eq!(
        data["permissions"],
        json!({"regularRead":true,"fullRead":false,"write":false})
    );
    assert_eq!(
        data["scope"],
        json!({"departments":["synthetic A"],"taskTypes":["synthetic T"]})
    );
    let config = std::fs::read_to_string(home.join("config.toml")).unwrap();
    let parsed = config.parse::<toml_edit::DocumentMut>().unwrap();
    assert!(config.contains("# synthetic configuration"));
    assert_eq!(parsed["model"].as_str(), Some("keep-model"));
    assert_eq!(
        parsed["mcp_servers"]["other"]["command"].as_str(),
        Some("keep-program")
    );
    assert_eq!(
        parsed["mcp_servers"]["in_line"]["env"]["IN_LINE_MCP_TOKEN"].as_str(),
        Some(token)
    );
    assert!(parsed["mcp_servers"]["in_line"]["args"]
        .as_array()
        .unwrap()
        .is_empty());
    platform::Descriptor::new(false)
        .unwrap()
        .verify(&home.join("config.toml"))
        .unwrap();
    assert!(!ticket_file.exists());
    let replay = run().output().await.unwrap();
    assert!(!replay.status.success());
    let error: Value = serde_json::from_slice(&replay.stdout).unwrap();
    assert_eq!(error["error"]["code"], "import_used");
    assert_eq!(
        config,
        std::fs::read_to_string(home.join("config.toml")).unwrap()
    );
    assert!(!String::from_utf8_lossy(&replay.stdout).contains(token));
    // New short-lived handoffs reuse the same identity and exact config bytes.
    // They are distinct synthetic tickets, never replayed production commands.
    let auth_before = std::fs::read(security_root.join("authorization.dpapi")).unwrap();
    let file_before = platform::file_location(&home.join("config.toml"))
        .unwrap()
        .1;
    for _ in 0..2 {
        let reused = onboarding::reuse_at(
            &security,
            &security_root,
            std::path::Path::new(env!("CARGO_BIN_EXE_in-line-mcp")),
            &home,
            &receipt.client_id,
            chrono::Utc::now().timestamp(),
        )
        .unwrap();
        let mut cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_in-line-mcp"));
        cmd.args(["--import-client", &reused.package_id])
            .env("IN_LINE_MCP_TEST_SECURITY_ROOT", &security_root)
            .env("IN_LINE_MCP_TEST_CODEX_HOME", &home)
            .env(
                "IN_LINE_MCP_TEST_HOST_EXE",
                std::env::current_exe().unwrap(),
            )
            .creation_flags(0x08000000);
        let output = cmd.output().await.unwrap();
        assert!(output.status.success(), "synthetic reuse import failed");
        assert!(output.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&output.stdout).contains(token));
        let result: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["data"]["configurationChanged"], false);
        assert_eq!(result["data"]["configurationVerified"], true);
        assert_eq!(result["data"]["currentSessionVerified"], false);
        assert_eq!(result["data"]["clientId"], receipt.client_id);
        assert_eq!(result["data"]["scope"], data["scope"]);
        assert_eq!(result["data"]["permissions"], data["permissions"]);
    }
    assert_eq!(security.view().unwrap().clients.len(), 1);
    assert_eq!(
        std::fs::read(security_root.join("authorization.dpapi")).unwrap(),
        auth_before
    );
    assert_eq!(
        platform::file_location(&home.join("config.toml"))
            .unwrap()
            .1,
        file_before
    );
    assert_eq!(
        std::fs::read_to_string(home.join("config.toml")).unwrap(),
        config
    );
    let credentials = Credentials {
        client_id: receipt.client_id,
        token: token.into(),
    };
    security.revoke(&credentials.client_id).unwrap();
    assert_eq!(
        Security::read_at(&security_root)
            .unwrap()
            .authorize(&credentials)
            .err()
            .unwrap()
            .code,
        "revoked"
    );
    drop(db);
    drop(security);
    let resolved = std::fs::canonicalize(&root).unwrap();
    let temp = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    assert!(resolved.starts_with(temp));
    // Host tasks may retain the synthetic database handle until the test runtime exits.
    let _ = std::fs::remove_dir_all(root);
}
