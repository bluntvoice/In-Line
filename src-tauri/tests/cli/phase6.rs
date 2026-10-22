//! Installed programs are not used: probe only the just-built synthetic package.
use super::*;
#[tokio::test]
async fn phase6_offline_three_binary_identity_never_opens_store_gui_or_ipc() {
    let root = std::env::temp_dir().join(format!(
        "inline-phase6-offline-{}",
        in_line_lib::mcp::platform::random_secret().unwrap()
    ));
    assert!(!root.exists());
    let mut builds = vec![];
    for (name, path, args) in [
        (
            "in-line",
            env!("CARGO_BIN_EXE_in-line"),
            vec!["--build-info"],
        ),
        (
            "in-line-mcp",
            env!("CARGO_BIN_EXE_in-line-mcp"),
            vec!["--build-info"],
        ),
        (
            "in-line-cli",
            env!("CARGO_BIN_EXE_in-line-cli"),
            vec!["--version", "--format", "json"],
        ),
    ] {
        let output = launch(Path::new(path), &root, &args, None, None).await;
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_eq!(stdout.lines().count(), 1);
        let value: Value = serde_json::from_str(&stdout).unwrap();
        assert_eq!(value["status"], "ok");
        assert_eq!(value["data"]["name"], name);
        if name != "in-line-cli" {
            assert_eq!(value["data"]["offline"], true);
        }
        builds.push(value["data"]["build"].clone());
        assert!(
            !root.exists(),
            "offline probe must not create authorization/endpoint files"
        );
    }
    assert!(builds.windows(2).all(|p| p[0] == p[1]));
}
