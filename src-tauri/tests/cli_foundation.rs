#![cfg(windows)]
//! Actual console subprocess + protected IPC. All identities/data are synthetic.
use in_line_lib::{
    database::Database,
    mcp::{
        contract::*,
        platform,
        security::{Grant, Security},
    },
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::io::AsyncWriteExt;

struct Fixture {
    root: PathBuf,
    exe: PathBuf,
    security: Arc<Security>,
    credentials: Credentials,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "inline-cli-中文 空格-{}",
            platform::random_secret().unwrap()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let exe = root.join("in-line-cli.exe");
        std::fs::copy(env!("CARGO_BIN_EXE_in-line-cli"), &exe).unwrap();
        let security = Arc::new(Security::open_at(root.join("security")).unwrap());
        let issued = security
            .grant(Grant {
                name: "CLI隔离测试".into(),
                permissions: Default::default(),
                scope: Default::default(),
            })
            .unwrap();
        security
            .change(|store| {
                store
                    .clients
                    .iter_mut()
                    .find(|c| c.id == issued.client_id)
                    .unwrap()
                    .kind = Some("cli".into());
                Ok(())
            })
            .unwrap();
        Self {
            root,
            exe,
            security,
            credentials: Credentials {
                client_id: issued.client_id,
                token: issued.token,
            },
        }
    }
    fn security_root(&self) -> PathBuf {
        self.root.join("security")
    }
    fn profile(&self) -> Value {
        json!({"version":1,"purpose":"cli","name":"chosen","machine":platform::machine_id().unwrap(),"sid":platform::user_sid().unwrap(),"executable":std::fs::canonicalize(&self.exe).unwrap(),"binaryHash":format!("{:x}",Sha256::digest(std::fs::read(&self.exe).unwrap())),"softwareVersion":env!("CARGO_PKG_VERSION"),"credentials":self.credentials})
    }
    fn save(&self, name: &str, p: &Value) {
        let dir = self.security_root().join("cli-profiles");
        std::fs::create_dir_all(&dir).unwrap();
        platform::Descriptor::new(true)
            .unwrap()
            .apply(&dir)
            .unwrap();
        let file = dir.join(format!("{name}.dpapi"));
        std::fs::write(
            &file,
            platform::seal(&serde_json::to_vec(p).unwrap(), false).unwrap(),
        )
        .unwrap();
        platform::Descriptor::new(false)
            .unwrap()
            .apply(&file)
            .unwrap();
    }
    async fn run(&self, args: &[&str], input: Option<&[u8]>) -> (i32, Value) {
        let result = launch(
            &self.exe,
            &self.security_root(),
            args,
            input,
            Some(&self.credentials),
        )
        .await;
        let stdout = std::str::from_utf8(&result.stdout).unwrap();
        let stderr = std::str::from_utf8(&result.stderr).unwrap();
        assert!(!stdout.contains(&self.credentials.token));
        assert!(!stderr.contains(&self.credentials.token));
        assert!(
            stderr.is_empty(),
            "stderr must not enter JSON output: {stderr}"
        );
        assert_eq!(stdout.lines().count(), 1, "single JSON envelope");
        assert!(!stdout.contains('\u{1b}'));
        let value: Value = serde_json::from_str(stdout).unwrap();
        for field in ["apiVersion", "requestId", "status", "data", "error"] {
            assert!(value.get(field).is_some(), "{field}");
        }
        assert_eq!(value["apiVersion"], 1);
        (result.status.code().unwrap(), value)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
async fn launch(
    exe: &Path,
    root: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    mcp: Option<&Credentials>,
) -> std::process::Output {
    let mut command = tokio::process::Command::new(exe);
    command
        .args(args)
        .env("IN_LINE_MCP_TEST_SECURITY_ROOT", root)
        .env(
            "IN_LINE_MCP_TEST_HOST_EXE",
            std::env::current_exe().unwrap(),
        )
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if let Some(c) = mcp {
        command
            .env("IN_LINE_MCP_CLIENT_ID", &c.client_id)
            .env("IN_LINE_MCP_TOKEN", &c.token);
    }
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        child.stdin.take().unwrap().write_all(input).await.unwrap();
    } else {
        drop(child.stdin.take());
    }
    tokio::time::timeout(std::time::Duration::from_secs(15), child.wait_with_output())
        .await
        .expect("CLI subprocess deadline")
        .unwrap()
}

#[tokio::test]
async fn console_utf8_bom_paths_single_envelope_offline_and_strict_arguments() {
    let f = Fixture::new();
    let before = std::fs::read(f.security_root().join("authorization.dpapi")).unwrap();
    let (code, v) = f.run(&["--version", "--format", "json"], None).await;
    assert_eq!(code, 0);
    assert_eq!(v["data"]["name"], "in-line-cli");
    assert_eq!(
        v["data"]["build"]["softwareVersion"],
        env!("CARGO_PKG_VERSION")
    );
    let (code, v) = f.run(&["--help", "--format", "json"], None).await;
    assert_eq!(code, 0);
    assert!(v["data"]["help"].as_str().unwrap().contains("不启动主程序"));
    let (code, v) = f.run(&["doctor", "--format", "json"], None).await;
    assert_eq!(code, 6);
    assert_eq!(v["data"]["local"]["credentialsRead"], false);
    assert_eq!(v["data"]["local"]["connectionVerified"], false);
    assert_eq!(
        std::fs::read(f.security_root().join("authorization.dpapi")).unwrap(),
        before
    );
    assert!(!f.security_root().join("cli-profiles").exists());
    assert!(!f.root.join("inline.db").exists());
    // Metadata-only doctor must not execute these deliberately invalid binaries.
    std::fs::write(f.root.join("in-line.exe"), b"not an executable").unwrap();
    std::fs::write(f.root.join("in-line-mcp.exe"), b"not an executable").unwrap();
    let (code, v) = f.run(&["doctor", "--format", "json"], None).await;
    assert_eq!(code, 0);
    assert_eq!(v["data"]["compatibility"], "not_verified");
    assert_eq!(v["data"]["connectionVerified"], false);
    let input = f.root.join("中文 输入.json");
    std::fs::write(&input, b"\xef\xbb\xbf{}").unwrap();
    let (code, v) = f
        .run(
            &[
                "status",
                "--input",
                input.to_str().unwrap(),
                "--format",
                "json",
            ],
            None,
        )
        .await;
    assert_eq!(code, 3);
    assert_eq!(v["error"]["code"], "unauthenticated");
    let (code, v) = f
        .run(
            &["status", "--input", "-", "--format", "json"],
            Some(b"\xef\xbb\xbf{}"),
        )
        .await;
    assert_eq!(code, 3);
    assert_eq!(v["error"]["code"], "unauthenticated");
    for input in [
        b"\xff".as_slice(),
        b"{\"token\":\"never-echo-this\"}",
        b"{}{}",
    ] {
        let (code, v) = f
            .run(&["status", "--input", "-", "--format", "json"], Some(input))
            .await;
        assert_eq!(code, 2);
        assert!(!v.to_string().contains("never-echo-this"));
    }
    for args in [
        vec!["task", "create", "--format", "json"],
        vec!["--token", "never-echo-this", "--format", "json"],
        vec!["status", "--profile", "../escape", "--format", "json"],
        vec!["doctor", "--format", "json", "--format", "json"],
    ] {
        let (code, v) = f.run(&args, None).await;
        assert_eq!(code, 2);
        assert_eq!(v["error"]["code"], "invalid_arguments");
        assert!(!v.to_string().contains("never-echo-this"));
    }
    let out = launch(&f.exe, &f.security_root(), &["unknown"], None, None).await;
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(!out.stderr.is_empty());
}

#[tokio::test]
async fn piped_input_without_eof_returns_bounded_error_instead_of_hanging() {
    let f = Fixture::new();
    let mut child = tokio::process::Command::new(&f.exe)
        .args(["status", "--input", "-", "--format", "json"])
        .env("IN_LINE_MCP_TEST_SECURITY_ROOT", f.security_root())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    // Valid content alone is not a complete stream: keep the write end open.
    let mut pipe = child.stdin.take().unwrap();
    pipe.write_all(b"{}").await.unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(15), child.wait_with_output())
        .await
        .expect("CLI must enforce its own input deadline")
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stderr.is_empty());
    let envelope: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(envelope["status"], "error");
    assert_eq!(envelope["error"]["code"], "invalid_arguments");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_cli_profile_uses_existing_authenticated_ipc_and_preserves_permissions() {
    let f = Fixture::new();
    f.save("chosen", &f.profile());
    let database = Arc::new(Database::open_at(f.root.join("synthetic.db")).unwrap());
    let security = f.security.clone();
    let db = database.clone();
    in_line_lib::mcp::ipc::start(
        security.clone(),
        Arc::new(move |c, tool, args| {
            in_line_lib::mcp::service::execute(&security, &db, c, tool, args)
        }),
    )
    .unwrap();
    let (code, v) = f
        .run(&["status", "--profile", "chosen", "--format", "json"], None)
        .await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["transport"], "local_ipc");
    assert_eq!(v["data"]["connectionVerified"], true);
    assert_eq!(v["data"]["currentMcpSessionVerified"], false);
    assert_eq!(
        v["data"]["capabilities"]["clientId"],
        f.credentials.client_id
    );
    assert_eq!(v["data"]["capabilities"]["permissions"]["write"], false);
    for profile in [None, Some("missing")] {
        let mut args = vec!["status", "--format", "json"];
        if let Some(p) = profile {
            args.extend(["--profile", p]);
        }
        let (code, v) = f.run(&args, None).await;
        assert_eq!(code, 3);
        assert_eq!(v["error"]["code"], "unauthenticated");
    }
    f.security
        .change(|store| {
            store.paused = true;
            Ok(())
        })
        .unwrap();
    let (code, v) = f
        .run(&["status", "--profile", "chosen", "--format", "json"], None)
        .await;
    assert_eq!(code, 3);
    assert_eq!(v["error"]["code"], "paused");
    f.security
        .change(|store| {
            store.paused = false;
            Ok(())
        })
        .unwrap();
    f.security.revoke(&f.credentials.client_id).unwrap();
    let (code, v) = f
        .run(&["status", "--profile", "chosen", "--format", "json"], None)
        .await;
    assert_eq!(code, 3);
    assert_eq!(v["error"]["code"], "revoked");
    assert!(database
        .list_tasks(in_line_lib::models::TaskView::Queue)
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn profiles_reject_foreign_binding_tampering_and_mcp_identity_without_fallback() {
    let f = Fixture::new();
    let original = f.profile();
    for (field, value) in [
        ("name", json!("other")),
        ("purpose", json!("codex")),
        ("machine", json!("other-machine")),
        ("sid", json!("other-user")),
        ("binaryHash", json!("wrong-hash")),
        ("executable", json!("C:/other/in-line-cli.exe")),
        ("softwareVersion", json!("old")),
        ("version", json!(0)),
        ("unknown", json!(true)),
    ] {
        let mut profile = original.clone();
        profile[field] = value;
        f.save("chosen", &profile);
        let (code, v) = f
            .run(&["status", "--profile", "chosen", "--format", "json"], None)
            .await;
        assert_eq!(code, 6, "{field}");
        assert_eq!(v["error"]["code"], "security_unavailable");
    }
    let file = f.security_root().join("cli-profiles/chosen.dpapi");
    std::fs::write(&file, b"corrupt sealed profile").unwrap();
    let (code, v) = f
        .run(&["status", "--profile", "chosen", "--format", "json"], None)
        .await;
    assert_eq!(code, 6);
    assert_eq!(v["error"]["code"], "security_unavailable");
    assert_eq!(std::fs::read(&file).unwrap(), b"corrupt sealed profile");
    f.save("chosen", &original);
    f.security
        .change(|store| {
            store.clients[0].kind = Some("codex".into());
            Ok(())
        })
        .unwrap();
    let (code, v) = f
        .run(&["status", "--profile", "chosen", "--format", "json"], None)
        .await;
    assert_eq!(code, 3);
    assert_eq!(v["error"]["code"], "forbidden");
    // Doctor checks existence even with corrupt authorization; it must never decrypt it.
    let auth = f.security_root().join("authorization.dpapi");
    std::fs::write(&auth, b"invalid-store-kept-unchanged").unwrap();
    let (_, v) = f
        .run(&["doctor", "--profile", "chosen", "--format", "json"], None)
        .await;
    assert_eq!(v["data"]["local"]["credentialsRead"], false);
    assert_eq!(
        std::fs::read(&auth).unwrap(),
        b"invalid-store-kept-unchanged"
    );
}
