//! CLI adapter only. No database, GUI automation, shell or MCP subprocess.
mod output;
mod parse;
#[cfg(windows)]
mod profile;

use crate::mcp::{build_identity, contract::*};
use serde_json::{json, Value};
use std::{
    ffi::OsString,
    io::{IsTerminal, Read},
    path::Path,
    time::Duration,
};

pub use output::exit_code;
use parse::{Command, Options};

const MAX_INPUT: u64 = 1024 * 1024;
const HELP: &str = "In Line CLI（Windows，Phase2基础设施）
用法：in-line-cli [--help | --version | doctor | status] [选项]
  --help                 显示帮助；不启动主程序
  --version              显示版本和构建身份；不启动主程序
  doctor                 离线检查同目录程序及配置是否存在；不读取凭证或业务库
  status                 使用显式CLI profile，通过受保护IPC验证自身认证及兼容性
  --format table|json    默认table；json的stdout恰好一个UTF-8 Envelope
  --no-color             禁止终端颜色（目前所有输出均无颜色）
  --profile <name>       已授权CLI身份；不回退到MCP环境变量或其他profile
  --input <file|->       仅status接受空JSON对象{}；-读取管道；上限1MiB/10秒，支持UTF-8 BOM
Phase3将实现GUI授权包导入和只读业务命令；当前不提供授权、事项查询或写入。
首版通过完整路径调用，不自动修改PATH。长期凭证不得放入参数或聊天。";

fn arguments_error() -> McpError {
    let mut e = McpError::new("invalid_arguments");
    e.message = "CLI参数或输入无效；请使用--help。未执行操作，参数原文不回显".into();
    e
}

fn read_input(path: &str) -> Result<(), McpError> {
    if path == "-" && std::io::stdin().is_terminal() {
        return Err(arguments_error());
    }
    let path = path.to_owned();
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    // A pipe or file can stop producing bytes without closing. A plain worker
    // thread lets the console return a bounded error; main exits the process,
    // so a blocked reader cannot keep Tokio's shutdown waiting indefinitely.
    std::thread::Builder::new()
        .name("cli-input".into())
        .spawn(move || {
            let _ = send.send(read_input_bytes(&path).and_then(|bytes| validate_input(&bytes)));
        })
        .map_err(|_| McpError::new("resource_limit"))?;
    receive
        .recv_timeout(Duration::from_secs(10))
        .map_err(|_| arguments_error())?
}
fn read_input_bytes(path: &str) -> Result<Vec<u8>, McpError> {
    let reader: Box<dyn Read> = if path == "-" {
        Box::new(std::io::stdin())
    } else {
        if !std::fs::metadata(path)
            .map_err(|_| arguments_error())?
            .is_file()
        {
            return Err(arguments_error());
        }
        Box::new(std::fs::File::open(path).map_err(|_| arguments_error())?)
    };
    let mut bytes = Vec::new();
    reader
        .take(MAX_INPUT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| arguments_error())?;
    Ok(bytes)
}
fn validate_input(bytes: &[u8]) -> Result<(), McpError> {
    if bytes.len() as u64 > MAX_INPUT {
        return Err(arguments_error());
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| arguments_error())?;
    if !text.trim_start().starts_with('{') {
        return Err(arguments_error());
    }
    let _: CapabilityArgs = serde_json::from_str(text).map_err(|_| arguments_error())?;
    Ok(())
}

fn doctor_at(executable: &Path, root: Option<&Path>, name: Option<&str>) -> Value {
    // Existence is not proof of build compatibility, authorization or connection.
    // No Security::open/read_at, directory creation, process launch or DB access.
    let directory = executable.parent().unwrap_or(Path::new(""));
    let host = directory.join("in-line.exe").is_file();
    let sidecar = directory.join("in-line-mcp.exe").is_file();
    json!({"platform":std::env::consts::OS,"cliBuild":build_identity::current(),
        "hostPresent":host,"mcpPresent":sidecar,
        "authorizationStorePresent":root.is_some_and(|p|p.join("authorization.dpapi").is_file()),
        "selectedProfilePresent":name.map(|n|root.is_some_and(|p|p.join("cli-profiles").join(format!("{n}.dpapi")).is_file())),
        "compatibility":"not_verified","connectionVerified":false,
        "databaseConnection":"not_checked_offline","credentialsRead":false,
        "pathModified":false,"readyForStatus":cfg!(windows)&&host&&sidecar})
}

#[cfg(windows)]
async fn status(options: &Options, id: String) -> Result<Value, McpError> {
    let name = options
        .profile
        .as_deref()
        .ok_or_else(|| McpError::new("unauthenticated"))?;
    let root = crate::mcp::platform::root()?;
    let executable = std::env::current_exe().map_err(|_| McpError::new("host_unavailable"))?;
    let credentials = profile::load_at(&root, name, &executable)?;
    let data = crate::mcp::ipc::call_at_with_id(
        root,
        credentials.clone(),
        "get_capabilities",
        json!({}),
        id,
    )
    .await?;
    validate_status(data, &credentials.client_id)
}

#[cfg(any(windows, test))]
fn validate_status(data: Value, client: &str) -> Result<Value, McpError> {
    let permissions_valid = data.get("permissions").is_some_and(|permissions| {
        ["regularRead", "fullRead", "write"]
            .iter()
            .all(|key| permissions.get(key).and_then(Value::as_bool).is_some())
    });
    let scope_valid = data.get("scope").is_some_and(|scope| {
        serde_json::from_value::<crate::mcp::scope::Scope>(scope.clone())
            .is_ok_and(|scope| scope.validate().is_ok())
    });
    if data["clientId"] != client
        || data["softwareVersion"] != env!("CARGO_PKG_VERSION")
        || data["mcpVersion"] != API_VERSION
        || data["schemaVersion"] != crate::database::DATABASE_SCHEMA_VERSION
        || data["tools"] != json!(TOOLS)
        || data["connectionVerified"] != true
        || !permissions_valid
        || !scope_valid
        || data["authorizationRevision"].as_u64().is_none()
    {
        return Err(McpError::new("incompatible"));
    }
    // Keep the legacy MCP result unchanged; label the actual CLI transport here.
    Ok(json!({"transport":"local_ipc","connectionVerified":true,
        "currentMcpSessionVerified":false,"cliBuild":build_identity::current(),"capabilities":data}))
}

async fn execute(options: &Options, id: String) -> Result<Value, McpError> {
    if let Some(input) = &options.input {
        read_input(input)?;
    }
    match options.command {
        Command::Help => Ok(json!({"help":HELP})),
        Command::Version => {
            Ok(json!({"name":"in-line-cli","build":build_identity::current(),"cliPhase":2}))
        }
        Command::Doctor => {
            let exe = std::env::current_exe().map_err(|_| McpError::new("host_unavailable"))?;
            let root = crate::mcp::platform::root().ok();
            let local = doctor_at(&exe, root.as_deref(), options.profile.as_deref());
            if local["readyForStatus"] != true {
                Err(McpError::new("host_unavailable"))
            } else {
                Ok(local)
            }
        }
        Command::Status => {
            #[cfg(windows)]
            {
                status(options, id).await
            }
            #[cfg(not(windows))]
            {
                let _ = id;
                Err(McpError::new("unsupported"))
            }
        }
    }
}

pub async fn run(args: impl Iterator<Item = OsString>) -> i32 {
    let args: Vec<_> = args.collect();
    let json_output = parse::requests_json(&args);
    let id = crate::mcp::platform::random_secret().unwrap_or_else(|_| "cli-local-error".into());
    let parsed = parse::parse(args);
    let result = match &parsed {
        Ok(options) => execute(options, id.clone()).await,
        Err(_) => Err(arguments_error()),
    };
    let mut envelope = Envelope::result(id, result);
    // A failed status/doctor still reports safe local facts, never an authenticated result.
    if envelope.status == "error" {
        if let Ok(options) = &parsed {
            if matches!(options.command, Command::Doctor | Command::Status) {
                if let Ok(exe) = std::env::current_exe() {
                    let root = crate::mcp::platform::root().ok();
                    envelope.data = Some(
                        json!({"local":doctor_at(&exe,root.as_deref(),options.profile.as_deref())}),
                    );
                }
            }
        }
        if let Some(error) = envelope.error.as_mut() {
            error["diagnosticAvailability"] = json!("local_cli_or_host_if_reached");
        }
    }
    output::emit(&envelope, json_output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_utf8_json_bom_unknown_fields_and_trailing_content() {
        for ok in [b"{}".as_slice(), b"\xef\xbb\xbf{}", b" \n {} \r\n"] {
            assert!(validate_input(ok).is_ok());
        }
        for bad in [
            b"[]".as_slice(),
            b"null",
            b"{\"token\":\"secret\"}",
            b"{}{}",
            b"\xff",
            b"\xef\xbb\xbf\xef\xbb\xbf{}",
        ] {
            assert_eq!(validate_input(bad).unwrap_err().code, "invalid_arguments");
        }
        assert!(validate_input(&vec![b' '; MAX_INPUT as usize + 1]).is_err());
    }
    #[test]
    fn offline_doctor_does_not_create_or_open_storage() {
        let root = std::env::temp_dir().join(format!(
            "inline-cli-doctor-{}",
            crate::mcp::platform::random_secret().unwrap()
        ));
        let v = doctor_at(
            &root.join("中文 空格/in-line-cli.exe"),
            Some(&root.join("nonexistent")),
            Some("chosen"),
        );
        assert_eq!(v["credentialsRead"], false);
        assert_eq!(v["connectionVerified"], false);
        assert!(!root.exists());
    }
    #[test]
    fn authenticated_status_requires_same_client_contract_and_actual_confirmation() {
        let data = json!({"clientId":"only-client","softwareVersion":env!("CARGO_PKG_VERSION"),"mcpVersion":API_VERSION,"schemaVersion":crate::database::DATABASE_SCHEMA_VERSION,"tools":TOOLS,"connectionVerified":true,"permissions":{"regularRead":true,"fullRead":false,"write":false},"scope":{"departments":null,"taskTypes":null},"authorizationRevision":1});
        assert_eq!(
            validate_status(data.clone(), "only-client").unwrap()["transport"],
            "local_ipc"
        );
        assert!(validate_status(data.clone(), "other-client").is_err());
        for (field, value) in [
            ("connectionVerified", json!(false)),
            ("tools", json!([])),
            ("schemaVersion", json!(0)),
            ("softwareVersion", json!("old")),
            ("mcpVersion", json!(0)),
            ("authorizationRevision", json!(null)),
            ("permissions", json!({"regularRead":true})),
            (
                "permissions",
                json!({"regularRead":true,"fullRead":false,"write":"yes"}),
            ),
            ("scope", json!({"departments":[" invalid "]})),
            ("scope", json!(null)),
        ] {
            let mut bad = data.clone();
            bad[field] = value;
            assert_eq!(
                validate_status(bad, "only-client").unwrap_err().code,
                "incompatible"
            );
        }
    }
}
