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
#[path = "cli/phase4.rs"]
mod phase4;

fn task(title: &str, dept: &str) -> in_line_lib::models::TaskInput {
    serde_json::from_value(json!({"title":title,"department":dept,"departments":[dept],"contact":"private-contact","contacts":["private-contact"],"taskType":"T","details":"private-details","internalNotes":"private-notes","status":"pending","priority":"normal","workload":"standard","isUrgent":false,"urgentRequester":"","urgentReason":""})).unwrap()
}
fn host(f: &Fixture, database: Arc<Database>) {
    let security = f.security.clone();
    in_line_lib::mcp::ipc::start(
        security.clone(),
        Arc::new(move |c, tool, args| {
            in_line_lib::mcp::service::execute(&security, &database, c, tool, args)
        }),
    )
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_import_real_process_and_scoped_read_pagination_match_host_and_gui() {
    let f = Fixture::new();
    f.security
        .update_client(
            &f.credentials.client_id,
            Default::default(),
            in_line_lib::mcp::scope::Scope {
                departments: Some(vec!["A".into()]),
                task_types: None,
            },
        )
        .unwrap();
    let db = Arc::new(Database::open_at(f.root.join("synthetic.db")).unwrap());
    let first = db.save_task(task("第一项", "A")).unwrap();
    let second = db.save_task(task("第二项", "A")).unwrap();
    let outside = db.save_task(task("范围外", "B")).unwrap();
    db.record_work_event(in_line_lib::models::WorkEventInput {
        task_id: first.id,
        result_status: "processed".into(),
        handled_at: "2026-08-08T10:00:00+08:00".into(),
        note: "private-work-note".into(),
        sync_status: true,
    })
    .unwrap();
    host(&f, db.clone());
    let (package, profile) = f.package(None, None);
    let (code, v) = f
        .run(
            &[
                "client",
                "import",
                "--package",
                &package,
                "--format",
                "json",
            ],
            None,
        )
        .await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["profileName"], profile);
    assert_eq!(v["data"]["cliConnectionVerified"], true);
    assert_eq!(v["data"]["currentMcpSessionVerified"], false);
    let (code, v) = f
        .run(
            &[
                "client",
                "import",
                "--package",
                &package,
                "--format",
                "json",
            ],
            None,
        )
        .await;
    assert_eq!(code, 6, "{v}");
    assert_eq!(v["error"]["code"], "import_used");
    let (code, v) = f
        .run(
            &[
                "task",
                "list",
                "--profile",
                &profile,
                "--limit",
                "1",
                "--format",
                "json",
            ],
            None,
        )
        .await;
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["total"], 2);
    assert!(!v.to_string().contains("private-"));
    assert!(!v.to_string().contains("范围外"));
    let cursor = v["data"]["nextCursor"].as_str().unwrap();
    // Each run is a fresh process; the host retains the shared immutable snapshot.
    let (code, next) = f
        .run(
            &[
                "task",
                "list",
                "--profile",
                &profile,
                "--limit",
                "1",
                "--cursor",
                cursor,
                "--format",
                "json",
            ],
            None,
        )
        .await;
    assert_eq!(code, 0, "{next}");
    assert_eq!(
        v["data"]["meta"]["snapshotId"],
        next["data"]["meta"]["snapshotId"]
    );
    assert_ne!(v["data"]["items"][0]["id"], next["data"]["items"][0]["id"]);
    let (code, all) = f
        .run(
            &[
                "task",
                "list",
                "--profile",
                &profile,
                "--limit",
                "1",
                "--all",
                "--format",
                "json",
            ],
            None,
        )
        .await;
    assert_eq!(code, 0, "{all}");
    assert_eq!(all["data"]["pagination"]["complete"], true);
    assert_eq!(all["data"]["pagination"]["itemsReturned"], 2);
    for id in [outside.id, 999999] {
        let (code, v) = f
            .run(
                &[
                    "task",
                    "get",
                    &id.to_string(),
                    "--profile",
                    &profile,
                    "--format",
                    "json",
                ],
                None,
            )
            .await;
        assert_eq!(code, 4, "{v}");
        assert_eq!(v["error"]["code"], "not_found");
    }
    let (code, v) = f
        .run(
            &[
                "task",
                "get",
                &second.id.to_string(),
                "--projection",
                "id,contacts",
                "--profile",
                &profile,
                "--format",
                "json",
            ],
            None,
        )
        .await;
    assert_eq!(code, 3, "{v}");
    assert_eq!(v["error"]["code"], "forbidden");
    let (code, v) = f
        .run(
            &[
                "task",
                "list",
                "--profile",
                &profile,
                "--limit",
                "1",
                "--input",
                "-",
                "--format",
                "json",
            ],
            Some(br#"{"limit":1}"#),
        )
        .await;
    assert_eq!(code, 2, "{v}");
    f.security.revoke(&f.credentials.client_id).unwrap();
    let (code, v) = f
        .run(
            &["task", "list", "--profile", &profile, "--format", "json"],
            None,
        )
        .await;
    assert_eq!(code, 3, "{v}");
    assert_eq!(v["error"]["code"], "revoked");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_import_failure_rolls_back_and_conflicts_never_overwrite_profiles() {
    let f = Fixture::new();
    // A verified IPC peer returning a mismatched contract must fail and roll back.
    in_line_lib::mcp::ipc::start(
        f.security.clone(),
        Arc::new(move |_, _, _| Ok(json!({"clientId":"wrong"}))),
    )
    .unwrap();
    let (package, profile) = f.package(None, None);
    let (code, v) = f
        .run(
            &[
                "client",
                "import",
                "--package",
                &package,
                "--format",
                "json",
            ],
            None,
        )
        .await;
    assert_eq!(code, 6, "{v}");
    assert_eq!(v["error"]["code"], "incompatible");
    assert!(!f
        .security_root()
        .join("cli-profiles")
        .join(format!("{profile}.dpapi"))
        .exists());
    let result: Value = serde_json::from_slice(
        &std::fs::read(
            f.security_root()
                .join("cli-onboarding")
                .join(&package)
                .join("result.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(result["rolledBack"], true);
    let (package, profile) = f.package(None, None);
    let mut p = f.profile();
    p["name"] = json!(profile);
    f.save(&profile, &p);
    let path = f
        .security_root()
        .join("cli-profiles")
        .join(format!("{profile}.dpapi"));
    let before = std::fs::read(&path).unwrap();
    let (code, v) = f
        .run(
            &[
                "client",
                "import",
                "--package",
                &package,
                "--format",
                "json",
            ],
            None,
        )
        .await;
    assert_eq!(code, 5, "{v}");
    assert_eq!(v["error"]["code"], "import_conflict");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let (package, _) = f.package(Some(&before), Some(chrono::Utc::now().timestamp()));
    let (code, v) = f
        .run(
            &[
                "client",
                "import",
                "--package",
                &package,
                "--format",
                "json",
            ],
            None,
        )
        .await;
    assert_eq!(code, 6, "{v}");
    assert_eq!(v["error"]["code"], "import_expired");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let (package, _) = f.package(Some(&before), None);
    let (code, v) = f
        .run(
            &[
                "client",
                "import",
                "--package",
                &package,
                "--format",
                "json",
            ],
            None,
        )
        .await;
    assert_eq!(code, 6, "{v}");
    assert_eq!(v["error"]["code"], "incompatible");
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "failed rotation restores exact previous encrypted profile"
    );
}

struct Fixture {
    root: PathBuf,
    exe: PathBuf,
    security: Arc<Security>,
    credentials: Credentials,
}
impl Fixture {
    fn package(&self, previous: Option<&[u8]>, expires: Option<i64>) -> (String, String) {
        self.security
            .change(|store| {
                store
                    .clients
                    .iter_mut()
                    .find(|c| c.id == self.credentials.client_id)
                    .unwrap()
                    .name = "Codex CLI".into();
                Ok(())
            })
            .unwrap();
        let id = platform::random_secret().unwrap();
        let name = format!("cli-{}", &self.credentials.client_id[..32]);
        let mut profile = self.profile();
        profile["name"] = json!(name);
        let now = chrono::Utc::now().timestamp();
        let ticket = json!({"version":1,"purpose":"cli","packageId":id,"selectedClient":"codex-cli","issuedAt":expires.map_or(now, |t|t-600),"expiresAt":expires.unwrap_or(now+600),"previousHash":previous.map(|v|format!("{:x}",Sha256::digest(v))),"profile":profile});
        let home = self.security_root().join("cli-onboarding");
        let dir = home.join(&id);
        for p in [&home, &dir] {
            std::fs::create_dir_all(p).unwrap();
            platform::Descriptor::new(true).unwrap().apply(p).unwrap();
        }
        let path = dir.join("ticket.dpapi");
        std::fs::write(
            &path,
            platform::seal(&serde_json::to_vec(&ticket).unwrap(), false).unwrap(),
        )
        .unwrap();
        platform::Descriptor::new(false)
            .unwrap()
            .apply(&path)
            .unwrap();
        (id, name)
    }
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_read_command_families_preserve_readonly_scope_statistics_and_projection() {
    for family in 0..3 {
        let f = Fixture::new();
        f.security
            .update_client(
                &f.credentials.client_id,
                Default::default(),
                in_line_lib::mcp::scope::Scope {
                    departments: Some(vec!["A".into()]),
                    task_types: None,
                },
            )
            .unwrap();
        f.save("chosen", &f.profile());
        let profile = String::from("chosen");
        let db = Arc::new(Database::open_at(f.root.join("synthetic.db")).unwrap());
        let first = db.save_task(task("第一项", "A")).unwrap();
        let second = db.save_task(task("第二项", "A")).unwrap();
        db.save_task(task("范围外", "B")).unwrap();
        db.record_work_event(in_line_lib::models::WorkEventInput {
            task_id: first.id,
            result_status: "processed".into(),
            handled_at: "2026-08-08T10:00:00+08:00".into(),
            note: "private-work-note".into(),
            sync_status: true,
        })
        .unwrap();
        let before = business_view(&db);
        host(&f, db.clone());
        let first_id = first.id.to_string();
        let second_id = second.id.to_string();
        if family < 2 {
            let commands = if family == 0 {
                vec![
                    vec!["task", "search", "第二"],
                    vec!["task", "ticket", &second_id],
                    vec!["task", "history", &first_id],
                    vec!["task", "children", &first_id],
                ]
            } else {
                vec![
                    vec!["queue", "list"],
                    vec!["queue", "future"],
                    vec!["queue", "history", &first_id],
                    vec!["data", "health"],
                ]
            };
            for mut args in commands {
                args.extend(["--profile", &profile, "--format", "json"]);
                let (code, v) = f.run(&args, None).await;
                assert_eq!(code, 0, "{args:?}: {v}");
                assert!(!v.to_string().contains("private-"));
                assert!(!v.to_string().contains("范围外"));
            }
        } else {
            for cmd in [
                vec!["stats", "summary"],
                vec!["report", "daily"],
                vec!["report", "weekly"],
                vec!["report", "monthly"],
                vec!["report", "items"],
                vec!["calendar", "list"],
            ] {
                let summary = cmd == vec!["stats", "summary"];
                let mut args = cmd;
                args.extend([
                    "--from",
                    "2026-08-08",
                    "--to",
                    "2026-08-08",
                    "--timezone-offset-minutes",
                    "480",
                    "--profile",
                    &profile,
                    "--format",
                    "json",
                ]);
                let (code, v) = f.run(&args, None).await;
                assert_eq!(code, 0, "{args:?}: {v}");
                assert!(!v.to_string().contains("private-"));
                if summary {
                    let native = db
                        .statistics_scoped(
                            "2026-08-08T00:00:00+08:00".into(),
                            "2026-08-09T00:00:00+08:00".into(),
                            480,
                            in_line_lib::mcp::scope::Scope {
                                departments: Some(vec!["A".into()]),
                                task_types: None,
                            },
                        )
                        .unwrap();
                    assert_eq!(
                        v["data"]["statistics"],
                        serde_json::to_value(native).unwrap()
                    );
                }
            }
        }
        assert_eq!(
            business_view(&db),
            before,
            "read-only commands must not change business basis"
        );
    }
}

fn business_view(db: &Database) -> Value {
    json!({"tasks":(1..=3).map(|id|db.get_task(id).unwrap()).collect::<Vec<_>>(),"events":(1..=3).map(|id|db.list_work_events(id).unwrap()).collect::<Vec<_>>()})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_full_read_requires_both_global_and_independent_client_permission() {
    use in_line_lib::mcp::security::Permissions;
    let f = Fixture::new();
    f.save("chosen", &f.profile());
    let db = Arc::new(Database::open_at(f.root.join("synthetic.db")).unwrap());
    let row = db.save_task(task("敏感字段测试", "A")).unwrap();
    host(&f, db);
    let id = row.id.to_string();
    let args = [
        "task",
        "get",
        &id,
        "--projection",
        "id,contacts,details",
        "--profile",
        "chosen",
        "--format",
        "json",
    ];
    let full = Permissions {
        regular_read: true,
        full_read: true,
        write: false,
    };
    f.security.set_groups(full.clone()).unwrap();
    let (code, v) = f.run(&args, None).await;
    assert_eq!(code, 3, "{v}");
    assert!(!v.to_string().contains("private-"));
    f.security
        .update_client(&f.credentials.client_id, full, Default::default())
        .unwrap();
    let (code, v) = f.run(&args, None).await;
    assert_eq!(code, 0, "{v}");
    assert!(v.to_string().contains("private-contact"));
    f.security.set_groups(Default::default()).unwrap();
    let (code, v) = f.run(&args, None).await;
    assert_eq!(code, 3, "{v}");
    assert!(!v.to_string().contains("private-"));
    let (code, v) = f
        .run(
            &[
                "diagnostic",
                "get",
                "unknown",
                "--profile",
                "chosen",
                "--format",
                "json",
            ],
            None,
        )
        .await;
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["error"]["code"], "not_found");
}
