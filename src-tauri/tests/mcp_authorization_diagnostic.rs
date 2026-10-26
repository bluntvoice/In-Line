#![cfg(windows)]
use in_line_lib::mcp::{
    onboarding::{self, PrepareArgs},
    platform,
    security::{Permissions, Scope, Security},
};
use serde_json::Value;
use std::{path::Path, process::Command};

fn run(root: &Path, id: Option<&str>) -> std::process::Output {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new(env!("CARGO_BIN_EXE_in-line-mcp"));
    command.arg("--diagnose-authorization");
    if let Some(id) = id {
        command.arg(id);
    }
    command
        .env("IN_LINE_MCP_TEST_SECURITY_ROOT", root)
        .env("CODEX_HOME", root.join("must-not-create-client-home"))
        .env("IN_LINE_MCP_TOKEN", "synthetic-token-must-not-appear")
        .env(
            "IN_LINE_MCP_CLIENT_ID",
            "synthetic-credential-must-not-appear",
        )
        .creation_flags(0x08000000)
        .output()
        .unwrap()
}
#[test]
fn actual_diagnostic_mode_does_not_import_consume_or_echo_credentials() {
    let root = std::env::temp_dir().join(format!(
        "inline-diagnostic-process-{}",
        platform::random_secret().unwrap()
    ));
    let security = Security::open_at(root.clone()).unwrap();
    let exe = Path::new(env!("CARGO_BIN_EXE_in-line-mcp"));
    let receipt = onboarding::prepare_at(
        &security,
        &root,
        exe,
        PrepareArgs {
            client: "codex".into(),
            permissions: Permissions::default(),
            scope: Scope::default(),
        },
        chrono::Utc::now().timestamp(),
    )
    .unwrap();
    let ticket = root
        .join("onboarding")
        .join(&receipt.package_id)
        .join("ticket.dpapi");
    let ticket_before = std::fs::read(&ticket).unwrap();
    let store_before = std::fs::read(root.join("authorization.dpapi")).unwrap();
    let entries = || {
        let mut names: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        names
    };
    let entries_before = entries();
    let modified = std::fs::metadata(root.join("authorization.dpapi"))
        .unwrap()
        .modified()
        .unwrap();
    let output = run(&root, Some(&receipt.client_id));
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let json = String::from_utf8(output.stdout).unwrap();
    for forbidden in [
        "synthetic-token-must-not-appear",
        "synthetic-credential-must-not-appear",
        &receipt.client_id,
        &receipt.package_id,
        "tokenHash",
        "ipcSecret",
    ] {
        assert!(!json.contains(forbidden));
    }
    let report: Value = serde_json::from_str(&json).unwrap();
    assert_eq!(report["status"], "ok");
    assert_eq!(report["data"]["diagnosticOnly"], true);
    assert_eq!(report["data"]["configurationVerified"], false);
    assert_eq!(report["data"]["currentSessionVerified"], false);
    assert_eq!(report["data"]["disk"]["selectedClient"]["found"], true);
    assert_eq!(report["data"]["disk"]["selectedClient"]["kind"], "codex");
    assert!(report["data"]["memory"].is_null());
    assert_eq!(std::fs::read(&ticket).unwrap(), ticket_before);
    assert_eq!(
        std::fs::read(root.join("authorization.dpapi")).unwrap(),
        store_before
    );
    assert_eq!(
        std::fs::metadata(root.join("authorization.dpapi"))
            .unwrap()
            .modified()
            .unwrap(),
        modified
    );
    assert!(!ticket.with_file_name("attempt.lock").exists());
    assert!(!ticket.with_file_name("result.json").exists());
    assert!(!root.join("must-not-create-client-home").exists());
    assert_eq!(entries(), entries_before);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn missing_storage_and_invalid_id_never_create_authorizations() {
    let root = std::env::temp_dir().join(format!(
        "inline-diagnostic-absent-{}",
        platform::random_secret().unwrap()
    ));
    let output = run(&root, None);
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["data"]["storage"]["authorizationFileExists"], false);
    assert_eq!(report["data"]["diskErrorCode"], "security_unavailable");
    assert!(report["data"]["disk"].is_null());
    assert!(!root.exists());
    let output = run(&root, Some("invalid"));
    assert!(!output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["error"]["code"], "invalid_arguments");
    assert!(!root.exists());
}

#[test]
fn profile_canary_and_actual_sidecar_resolve_the_same_unredirected_file() {
    // A synthetic sibling under Profile exercises the real Codex launch context,
    // without opening or migrating the installed application's authorization.
    let root = dirs::home_dir().unwrap().join(format!(
        ".in-line-storage-test-{}",
        platform::random_secret().unwrap()
    ));
    let security = Security::open_at(root.clone()).unwrap();
    let path = root.join("authorization.dpapi");
    let (physical, identity) = platform::file_location(&path).unwrap();
    assert!(platform::same_location_name(&path, &physical));
    let before = std::fs::read(&path).unwrap();
    let output = run(&root, None);
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["data"]["storage"]["redirected"], false);
    assert_eq!(report["data"]["storage"]["fileIdentity"], identity);
    assert_eq!(report["data"]["disk"]["clientCount"], 0);
    assert_eq!(report["data"]["configurationVerified"], false);
    assert_eq!(report["data"]["currentSessionVerified"], false);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!root.join("must-not-create-client-home").exists());
    drop(security);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn appdata_canary_guard_agrees_with_the_actual_physical_file() {
    // Public synthetic bytes only. A packaged launcher may redirect this canary;
    // an unpackaged CI runner may leave it in place. Both must be recognized.
    let root = dirs::data_local_dir().unwrap().join("Temp").join(format!(
        "inline-location-test-{}",
        platform::random_secret().unwrap()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("metadata-canary.txt");
    std::fs::write(&file, b"public synthetic canary").unwrap();
    let (physical, _) = platform::file_location(&file).unwrap();
    let redirected = !platform::same_location_name(&file, &physical);
    if redirected {
        assert_eq!(
            platform::require_unredirected(&file).unwrap_err().code,
            "storage_redirected"
        );
    } else {
        platform::require_unredirected(&file).unwrap();
    }
    println!("synthetic_appdata_canary_redirected={redirected}");
    assert!(root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("inline-location-test-"));
    std::fs::remove_dir_all(root).unwrap();
}
