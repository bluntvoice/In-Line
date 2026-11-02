//! P2-D acceptance uses only fresh synthetic stores and test-owned stdio children.
//! Never opens a real Codex home, installs software, or performs an allowed MCP write.
#![cfg(windows)]
use in_line_lib::{
    database::Database,
    mcp::{
        contract::Credentials,
        ipc,
        onboarding::{self, PrepareArgs},
        platform,
        security::{Grant, Permissions, Scope, Security},
        service,
    },
    models::TaskInput,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
};

// Write only explicit metadata fields, never raw requests, responses, config or stores.
// Direct stdout keeps these records in ordinary CI logs without changing the Workflow
// or requiring --nocapture. Normal test assertions and failure behavior are unchanged.
fn record(event: &str, fields: Value) {
    use std::io::Write;
    let row = json!({"schema":1,"event":event,"fields":fields});
    let mut output = std::io::stdout().lock();
    writeln!(output, "P2D_EVIDENCE {row}").unwrap();
}
fn creation_time(child: &Child) -> u64 {
    use windows_sys::Win32::{Foundation::FILETIME, System::Threading::GetProcessTimes};
    let mut created: FILETIME = unsafe { std::mem::zeroed() };
    let mut exit = created;
    let mut kernel = created;
    let mut user = created;
    let result = unsafe {
        GetProcessTimes(
            child.raw_handle().unwrap(),
            &mut created,
            &mut exit,
            &mut kernel,
            &mut user,
        )
    };
    assert_ne!(result, 0, "test-owned process creation time unavailable");
    ((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64
}

// Keep protocol/process ownership explicit: EOF and wait() prove the old child exited.
// Deadlines fail the test; they are not product timeouts or forced termination rules.
struct Connection {
    child: Child,
    input: Option<ChildStdin>,
    lines: Lines<BufReader<ChildStdout>>,
    sequence: u64,
    case: &'static str,
    created: u64,
}
impl Connection {
    async fn open(f: &Fixture, credentials: Option<&Credentials>) -> Self {
        let mut command = f.command();
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(c) = credentials {
            command
                .env("IN_LINE_MCP_CLIENT_ID", &c.client_id)
                .env("IN_LINE_MCP_TOKEN", &c.token);
        }
        let mut child = command.spawn().expect("spawn synthetic sidecar");
        let created = creation_time(&child);
        record(
            "connection_start",
            json!({"case":f.case,"pid":child.id(),"created":created,
            "sidecarSha256":format!("{:x}",Sha256::digest(std::fs::read(env!("CARGO_BIN_EXE_in-line-mcp")).unwrap()))}),
        );
        let input = child.stdin.take();
        let lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let mut connection = Self {
            child,
            input,
            lines,
            sequence: 0,
            case: f.case,
            created,
        };
        let initialized = connection
            .request(
                "initialize",
                json!({
                    "protocolVersion":"2025-11-25", "capabilities":{},
                    "clientInfo":{"name":"isolated-p2d-acceptance","version":"1"}
                }),
            )
            .await;
        assert!(
            initialized.get("result").is_some(),
            "initialize result missing"
        );
        connection
            .send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        connection
    }
    async fn send(&mut self, message: Value) {
        let bytes = serde_json::to_vec(&message).unwrap();
        let input = self.input.as_mut().unwrap();
        input.write_all(&bytes).await.unwrap();
        input.write_all(b"\n").await.unwrap();
        input.flush().await.unwrap();
    }
    async fn request(&mut self, method: &str, params: Value) -> Value {
        self.sequence += 1;
        let id = self.sequence;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await;
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let line = self
                    .lines
                    .next_line()
                    .await
                    .expect("protocol read")
                    .expect("unexpected EOF");
                // Do not include raw responses/arguments in diagnostics, even on failure.
                let response: Value =
                    serde_json::from_str(&line).expect("JSON protocol output required");
                assert_eq!(response["jsonrpc"], "2.0");
                if response["id"] == id {
                    return response;
                }
                assert!(
                    response.get("method").is_some(),
                    "unexpected protocol response id"
                );
            }
        })
        .await
        .expect("synthetic protocol deadline exceeded")
    }
    async fn call(&mut self, name: &str, arguments: Value) -> Value {
        let response = self
            .request("tools/call", json!({"name":name,"arguments":arguments}))
            .await;
        assert!(response.get("error").is_none(), "unexpected JSON-RPC error");
        let result = &response["result"];
        let body = result["structuredContent"].clone();
        assert!(body.is_object(), "structured MCP envelope required");
        assert_eq!(result["isError"], body["status"] == "error");
        let mut fields = json!({"case":self.case,"pid":self.child.id(),"tool":name,
            "status":body["status"],"errorCode":body["error"]["code"]});
        if name == "get_capabilities" && body["status"] == "ok" {
            for field in [
                "permissions",
                "scope",
                "credentialResolution",
                "verificationScope",
                "softwareVersion",
                "mcpVersion",
                "schemaVersion",
                "commit",
            ] {
                fields[field] = body["data"][field].clone();
            }
        }
        if name == "query_tasks" && body["status"] == "ok" {
            fields["returnedItems"] = json!(body["data"]["items"].as_array().map(Vec::len));
            fields["total"] = body["data"]["total"].clone();
        }
        record("tool_result", fields);
        body
    }
    fn still_running(&mut self, original_pid: u32) {
        assert_eq!(self.child.id(), Some(original_pid));
        assert!(
            self.child.try_wait().unwrap().is_none(),
            "original sidecar ended"
        );
        assert_eq!(creation_time(&self.child), self.created);
        record(
            "same_process_alive",
            json!({"case":self.case,"pid":original_pid,"created":self.created}),
        );
    }
    async fn eof(mut self) {
        let pid = self.child.id().unwrap();
        drop(self.input.take());
        let status = tokio::time::timeout(Duration::from_secs(30), self.child.wait())
            .await
            .expect("test-owned sidecar did not exit after EOF")
            .unwrap();
        assert!(status.success(), "sidecar EOF exit was not successful");
        assert!(
            self.child.try_wait().unwrap().is_some(),
            "exit not observed on original child handle"
        );
        assert!(
            self.lines.next_line().await.unwrap().is_none(),
            "extra protocol output after EOF"
        );
        let mut stderr = Vec::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_end(&mut stderr)
            .await
            .unwrap();
        assert!(stderr.is_empty(), "unexpected sidecar stderr (redacted)");
        record(
            "connection_eof_exit",
            json!({"case":self.case,"pid":pid,"created":self.created,"exitCode":status.code(),"stdoutEof":true,"stderrEmpty":true}),
        );
    }
}

struct Fixture {
    case: &'static str,
    root: PathBuf,
    security: Arc<Security>,
    db: Arc<Database>,
}
impl Fixture {
    fn new(case: &'static str) -> Self {
        let root =
            std::env::temp_dir().join(format!("inline-p2d-{}", platform::random_secret().unwrap()));
        std::fs::create_dir_all(root.join("codex")).unwrap();
        let security = Arc::new(Security::open_at(root.join("security")).unwrap());
        let db = Arc::new(Database::open_at(root.join("synthetic.db")).unwrap());
        let sec = security.clone();
        let database = db.clone();
        ipc::start(
            security.clone(),
            Arc::new(move |c, t, a| service::execute(&sec, &database, c, t, a)),
        )
        .unwrap();
        Self {
            root,
            security,
            db,
            case,
        }
    }
    fn command(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_in-line-mcp"));
        c.env("IN_LINE_MCP_TEST_SECURITY_ROOT", self.root.join("security"))
            .env("IN_LINE_MCP_TEST_CODEX_HOME", self.root.join("codex"))
            .env(
                "IN_LINE_MCP_TEST_HOST_EXE",
                std::env::current_exe().unwrap(),
            )
            .env(
                "IN_LINE_MCP_TEST_CLIENT_EXE",
                std::env::current_exe().unwrap(),
            )
            .env_remove("IN_LINE_MCP_CLIENT_ID")
            .env_remove("IN_LINE_MCP_TOKEN")
            .creation_flags(0x08000000);
        c
    }
    fn prepare(&self, permissions: Permissions, scope: Scope) -> onboarding::Receipt {
        onboarding::prepare_at(
            &self.security,
            &self.root.join("security"),
            std::path::Path::new(env!("CARGO_BIN_EXE_in-line-mcp")),
            PrepareArgs {
                client: "codex".into(),
                permissions,
                scope,
            },
            chrono::Utc::now().timestamp(),
        )
        .unwrap()
    }
    async fn import(&self, receipt: &onboarding::Receipt) -> Value {
        let output = tokio::time::timeout(
            Duration::from_secs(30),
            self.command()
                .args(["--import-client", &receipt.package_id])
                .output(),
        )
        .await
        .unwrap()
        .unwrap();
        // Do not print the package, config, credential, token digest or raw stdout.
        assert!(
            output.status.success(),
            "synthetic importer failed; output redacted"
        );
        assert!(output.stderr.is_empty(), "importer stderr redacted");
        let result: Value = serde_json::from_slice(&output.stdout).expect("importer JSON required");
        assert_eq!(result["status"], "ok");
        assert_eq!(result["data"]["configurationVerified"], true);
        assert_eq!(result["data"]["currentSessionVerified"], false);
        assert!(
            result["data"]["clientId"] == receipt.client_id,
            "imported identity mismatch"
        );
        record(
            "import_result",
            json!({"case":self.case,"exitCode":output.status.code(),
            "configurationChanged":result["data"]["configurationChanged"],
            "configurationVerified":true,"currentSessionVerified":false,"identityMatches":true}),
        );
        result
    }
    fn config(&self) -> PathBuf {
        self.root.join("codex/config.toml")
    }
    fn credentials(&self) -> Credentials {
        let config = std::fs::read_to_string(self.config())
            .unwrap()
            .parse::<toml_edit::DocumentMut>()
            .unwrap();
        let env = &config["mcp_servers"]["in_line"]["env"];
        Credentials {
            client_id: env["IN_LINE_MCP_CLIENT_ID"].as_str().unwrap().into(),
            token: env["IN_LINE_MCP_TOKEN"].as_str().unwrap().into(),
        }
    }
    fn seed(&self, title: &str, department: &str) -> i64 {
        // Fixture provisioning through the existing database API, never a positive MCP write.
        let input: TaskInput = serde_json::from_value(json!({
            "title":title,"department":department,"departments":[department],
            "contact":"synthetic contact","contacts":["synthetic contact"],"taskType":"T",
            "details":"synthetic private detail","internalNotes":"synthetic private note",
            "status":"pending","priority":"normal","workload":"standard","isUrgent":false,
            "urgentRequester":"","urgentReason":""
        }))
        .unwrap();
        self.db.save_task(input).unwrap().id
    }
}
fn ok(body: Value) -> Value {
    assert_eq!(body["status"], "ok");
    body["data"].clone()
}
fn denied(body: Value, code: &str) {
    assert_eq!(body["status"], "error");
    assert_eq!(body["error"]["code"], code);
    assert!(
        body["data"].is_null(),
        "error must not disclose business data"
    );
}
async fn titles(connection: &mut Connection, ids: &[i64]) -> Vec<String> {
    let data = ok(connection
        .call(
            "query_tasks",
            json!({"filters":{"ids":ids},"projection":["id","title"]}),
        )
        .await);
    data["items"]
        .as_array()
        .expect("query page items")
        .iter()
        .map(|v| v["title"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn p2d_d01_first_import_authorized_sentinel_credentials_and_d05_eof_reconnect() {
    let f = Fixture::new("D01-first-D05");
    let sentinel = f.seed("p2d-first-sentinel", "A");
    assert!(f.security.view().unwrap().clients.is_empty());
    let receipt = f.prepare(Permissions::default(), Scope::default());
    assert_eq!(f.security.view().unwrap().clients.len(), 1);
    assert!(receipt.expires_at > chrono::Utc::now().timestamp());
    f.import(&receipt).await;
    let credentials = f.credentials();
    assert!(
        credentials.client_id == receipt.client_id,
        "config identity mismatch"
    );
    let mut original = Connection::open(&f, Some(&credentials)).await;
    let caps = ok(original.call("get_capabilities", json!({})).await);
    assert!(
        caps["clientId"] == receipt.client_id,
        "protocol identity mismatch"
    );
    assert_eq!(caps["verificationScope"], "thisToolCall");
    assert_eq!(caps["sidecarBuild"]["exactCommit"], caps["commit"]);
    if let Ok(expected) = std::env::var("GITHUB_SHA") {
        assert_eq!(caps["commit"], expected);
    }
    assert_eq!(
        titles(&mut original, &[sentinel]).await,
        vec!["p2d-first-sentinel"]
    );
    original.eof().await; // Held Child handle confirms old process exit, not just cancellation.
    let mut reconnected = Connection::open(&f, Some(&credentials)).await;
    assert_eq!(
        titles(&mut reconnected, &[sentinel]).await,
        vec!["p2d-first-sentinel"]
    );
    reconnected.eof().await; // Driver-created connection, never Codex automatic recovery.
}

#[tokio::test]
async fn p2d_d01_unbound_missing_invalid_and_mismatched_credentials_are_rejected() {
    // No import/runtime binding in this fresh fixture: an explicitly authorized
    // binding refresh is a separate case, not a credential-rejection bypass.
    let f = Fixture::new("D01-rejected");
    f.seed("p2d-inaccessible-sentinel", "A");
    let issued = f
        .security
        .grant(Grant {
            name: "synthetic credential rejection".into(),
            permissions: Permissions::default(),
            scope: Scope::default(),
        })
        .unwrap();
    let credentials = Credentials {
        client_id: issued.client_id,
        token: issued.token,
    };
    let other = f
        .security
        .grant(Grant {
            name: "synthetic mismatch".into(),
            permissions: Permissions::default(),
            scope: Scope::default(),
        })
        .unwrap();
    let invalid = Credentials {
        client_id: credentials.client_id.clone(),
        token: "not-a-valid-token".into(),
    };
    let mismatched = Credentials {
        client_id: credentials.client_id.clone(),
        token: other.token,
    };
    for creds in [None, Some(&invalid), Some(&mismatched)] {
        let mut rejected = Connection::open(&f, creds).await;
        denied(
            rejected.call("get_capabilities", json!({})).await,
            "unauthenticated",
        );
        denied(
            rejected.call("query_tasks", json!({})).await,
            "unauthenticated",
        );
        rejected.eof().await;
    }
}

#[tokio::test]
async fn p2d_d03_equivalent_import_preserves_active_transport_host_policy_and_loaded_credentials() {
    let f = Fixture::new("D03");
    let sentinel = f.seed("p2d-active-sentinel", "A");
    std::fs::write(f.config(), "# synthetic host policy\nmodel='keep-model'\n[mcp_servers.other]\ncommand='keep-other'\nargs=[]\n").unwrap();
    let receipt = f.prepare(Permissions::default(), Scope::default());
    f.import(&receipt).await;
    let credentials = f.credentials();
    let mut original = Connection::open(&f, Some(&credentials)).await;
    let pid = original.child.id().unwrap();
    ok(original.call("get_capabilities", json!({})).await);
    let mut document = std::fs::read_to_string(f.config())
        .unwrap()
        .parse::<toml_edit::DocumentMut>()
        .unwrap();
    document["mcp_servers"]["other"]["command"] = toml_edit::value("updated-other");
    document["mcp_servers"]["in_line"]["default_tools_approval_mode"] = toml_edit::value("prompt");
    let before = document.to_string();
    std::fs::write(f.config(), &before).unwrap();
    platform::Descriptor::new(false)
        .unwrap()
        .apply(&f.config())
        .unwrap();
    let location = platform::file_location(&f.config()).unwrap().1;
    let auth = std::fs::read(f.root.join("security/authorization.dpapi")).unwrap();
    for _ in 0..2 {
        let reused = onboarding::reuse_at(
            &f.security,
            &f.root.join("security"),
            std::path::Path::new(env!("CARGO_BIN_EXE_in-line-mcp")),
            &f.root.join("codex"),
            &receipt.client_id,
            chrono::Utc::now().timestamp(),
        )
        .unwrap();
        assert_eq!(
            f.import(&reused).await["data"]["configurationChanged"],
            false
        );
        assert!(
            std::fs::read_to_string(f.config()).unwrap() == before,
            "equivalent import changed config"
        );
        assert_eq!(platform::file_location(&f.config()).unwrap().1, location);
        original.still_running(pid);
        let caps = ok(original.call("get_capabilities", json!({})).await);
        assert_eq!(caps["credentialResolution"]["changed"], false);
        assert_eq!(
            titles(&mut original, &[sentinel]).await,
            vec!["p2d-active-sentinel"]
        );
    }
    assert_eq!(f.security.view().unwrap().clients.len(), 1);
    assert!(
        std::fs::read(f.root.join("security/authorization.dpapi")).unwrap() == auth,
        "equivalent import changed authorization"
    );
    // A disk-only credential edit must not be reported as a change in loaded credentials.
    document["mcp_servers"]["in_line"]["env"]["IN_LINE_MCP_TOKEN"] =
        toml_edit::value("0".repeat(64));
    std::fs::write(f.config(), document.to_string()).unwrap();
    assert!(
        f.credentials().token != credentials.token,
        "synthetic disk edit missing"
    );
    let failed = onboarding::reuse_at(
        &f.security,
        &f.root.join("security"),
        std::path::Path::new(env!("CARGO_BIN_EXE_in-line-mcp")),
        &f.root.join("codex"),
        &receipt.client_id,
        chrono::Utc::now().timestamp(),
    );
    assert_eq!(
        failed.err().unwrap().code,
        "onboarding_credentials_unavailable"
    );
    original.still_running(pid);
    let caps = ok(original.call("get_capabilities", json!({})).await);
    assert_eq!(caps["credentialResolution"]["changed"], false);
    assert_eq!(
        titles(&mut original, &[sentinel]).await,
        vec!["p2d-active-sentinel"]
    );
    assert_eq!(f.security.view().unwrap().clients.len(), 1);
    assert!(
        std::fs::read(f.root.join("security/authorization.dpapi")).unwrap() == auth,
        "failed reuse changed authorization"
    );
    std::fs::write(f.config(), before).unwrap();
    original.eof().await;
}

#[tokio::test]
async fn p2d_d04_original_connection_enforces_live_intersection_scope_pause_and_revocation() {
    let f = Fixture::new("D04");
    let a = f.seed("p2d-A-sentinel", "A");
    let b = f.seed("p2d-B-sentinel", "B");
    let receipt = f.prepare(
        Permissions {
            regular_read: true,
            full_read: true,
            write: true,
        },
        Scope::default(),
    );
    // Global defaults limit the broader client grant. No positively authorized write is sent.
    f.import(&receipt).await;
    let credentials = f.credentials();
    let other = f
        .security
        .grant(Grant {
            name: "synthetic unaffected client".into(),
            permissions: Permissions::default(),
            scope: Scope::default(),
        })
        .unwrap();
    let other_credentials = Credentials {
        client_id: other.client_id,
        token: other.token,
    };
    let mut original = Connection::open(&f, Some(&credentials)).await;
    let mut control = Connection::open(&f, Some(&other_credentials)).await;
    let pid = original.child.id().unwrap();
    let caps = ok(original.call("get_capabilities", json!({})).await);
    assert_eq!(
        caps["permissions"],
        json!({"regularRead":true,"fullRead":false,"write":false})
    );
    assert_eq!(titles(&mut original, &[a, b]).await.len(), 2);
    denied(
        original
            .call("query_tasks", json!({"projection":["id","details"]}))
            .await,
        "forbidden",
    );
    let write_args = json!({"action":"create","task":{"title":"must-not-exist","departments":["A"],"contacts":["synthetic"],"taskType":"T"},
        "idempotencyKey":"p2d-denied-write","intent":{"summary":"synthetic denied request","explicitUserRequest":true},"reason":"synthetic negative test"});
    denied(
        original.call("mutate_task", write_args.clone()).await,
        "forbidden",
    );
    f.security
        .update_client(
            &receipt.client_id,
            Permissions::default(),
            Scope {
                departments: Some(vec!["A".into()]),
                task_types: Some(vec!["T".into()]),
            },
        )
        .unwrap();
    f.security
        .set_groups(Permissions {
            regular_read: true,
            full_read: true,
            write: true,
        })
        .unwrap();
    original.still_running(pid);
    let caps = ok(original.call("get_capabilities", json!({})).await);
    assert_eq!(
        caps["permissions"],
        json!({"regularRead":true,"fullRead":false,"write":false})
    );
    assert_eq!(titles(&mut original, &[a, b]).await, vec!["p2d-A-sentinel"]);
    denied(
        original
            .call("query_tasks", json!({"projection":["id","details"]}))
            .await,
        "forbidden",
    );
    assert!(
        titles(&mut original, &[b]).await.is_empty(),
        "out-of-scope id leaked"
    );
    denied(original.call("mutate_task", write_args).await, "forbidden");
    let control_before = ok(control.call("get_capabilities", json!({})).await);
    assert_eq!(titles(&mut control, &[a, b]).await.len(), 2);
    f.security
        .update_client(
            &receipt.client_id,
            Permissions {
                regular_read: false,
                full_read: false,
                write: false,
            },
            Scope::default(),
        )
        .unwrap();
    denied(original.call("query_tasks", json!({})).await, "forbidden");
    assert_eq!(titles(&mut control, &[a, b]).await.len(), 2);
    f.security.pause(true).unwrap();
    denied(original.call("get_capabilities", json!({})).await, "paused");
    f.security.pause(false).unwrap();
    f.security.revoke(&receipt.client_id).unwrap();
    denied(
        original.call("get_capabilities", json!({})).await,
        "revoked",
    );
    denied(original.call("query_tasks", json!({})).await, "revoked");
    original.still_running(pid);
    let control_after = ok(control.call("get_capabilities", json!({})).await);
    assert!(
        control_after["clientId"] == control_before["clientId"],
        "other identity changed"
    );
    assert_eq!(control_after["permissions"], control_before["permissions"]);
    assert_eq!(control_after["scope"], control_before["scope"]);
    assert_eq!(titles(&mut control, &[a, b]).await.len(), 2);
    // Both denied write requests left exactly the two provisioned synthetic tasks.
    assert_eq!(ok(control.call("query_tasks", json!({})).await)["total"], 2);
    original.eof().await;
    control.eof().await;
}
