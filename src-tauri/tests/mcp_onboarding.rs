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
