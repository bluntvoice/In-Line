//! CLI adapter only. No database, GUI automation, shell or MCP subprocess.
#[cfg(windows)]
pub(crate) mod authorization;
mod json_input;
mod output;
mod parse;
#[cfg(windows)]
mod profile;
mod read;
mod write;

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
const HELP: &str = "In Line CLI（Windows，Phase4事项管理）
用法：in-line-cli <命令> [选项]
  --help                 显示帮助；不启动主程序
  --version              显示版本和构建身份；不启动主程序
  doctor                 离线检查同目录程序及配置是否存在；不读取凭证或业务库
  status                 使用显式CLI profile，通过受保护IPC验证自身认证及兼容性
  --format table|json    默认table；json的stdout恰好一个UTF-8 Envelope
  --no-color             禁止终端颜色（目前所有输出均无颜色）
  --profile <name>       已授权CLI身份；不回退到MCP环境变量或其他profile
  capabilities / client status   验证显式profile自身身份、权限和受保护IPC
  client import --package <id>   导入GUI签发的一次性CLI授权包；不接受Token
  task list / get <id> / search <text> / ticket <id> / children <id> / history <id>
  queue list / future / history <id>
  calendar list / stats summary / report daily|weekly|monthly|items --from YYYY-MM-DD --to YYYY-MM-DD
  data health / diagnostic get <error-id>
  --input <file|->       对应命令的请求JSON；-读取管道；上限1MiB/10秒，支持UTF-8 BOM
  --limit 1..100 / --cursor <值> / --snapshot <值> / --all
  --all                 同一快照逐页；上限100页/10000条/16MiB；不返回部分成功
  --projection <逗号分隔字段> / --ids <逗号分隔ID> / --text <文字>
  --statuses / --departments / --task-types / --priorities / --workloads / --permanent-numbers <逗号分隔值>
  --urgent / --scheduled / --active-queue <true|false>；--include-trash
  --parent-task-id / --structure / --archive / --planned-from / --planned-to
  --created-from / --created-to / --deadline-from / --deadline-to / --handled-from / --handled-to
  --timezone-offset-minutes <分钟>；history另支持--kinds及--include-voided
同一字段不得同时在命令参数和JSON指定；状态、字段、日期口径由主程序校验。
日报/周报/月报当前均需显式起止日期，返回原结构化摘要；完整文本留后续阶段。
写入命令：task create/update/status set/complete/urgent set|clear/archive/trash/restore/reopen/reschedule；
  task child create/parent set|clear/children reorder；queue enter/move；work process-round/complete-round/record/correct/void；undo request。
  写入必须--execute及显式--profile；--input提供意图、原因、原幂等键、版本或字段基准，禁止自动填版本或猜业务字段。
  通用：--idempotency-key、--reason、--intent-summary、--explicit-user-request；真实办理另须--confirmed-real-work。
  版本：--task-version、--parent-version；业务：--date、--event-id、--audit-id、--requester、--urgent-reason、--handled-at、--result-status、--note。
  父任务完成须明确--include-eligible-subtasks或--parent-only，并提供当前所有子任务taskVersions；普通完成保留原保护。
  归档/软删除可显式--include-completed-subtasks/--include-subtasks；恢复沿用GUI加入今日队列。
  task parent candidates <id>；receipt get --idempotency-key <原键>仅查当前profile原回执，绝不重做写入。
不提供授权增权、审批approve、force、任意工具调用或SQL。导入/CLI验证不代表当前聊天MCP已连接。
首版通过完整路径调用，不自动修改PATH。长期凭证不得放入参数或聊天。";

fn arguments_error() -> McpError {
    let mut e = McpError::new("invalid_arguments");
    e.message = "CLI参数或输入无效；请使用--help。未执行操作，参数原文不回显".into();
    e
}

fn read_input(path: &str) -> Result<Value, McpError> {
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
fn validate_input(bytes: &[u8]) -> Result<Value, McpError> {
    if bytes.len() as u64 > MAX_INPUT {
        return Err(arguments_error());
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let text = std::str::from_utf8(bytes).map_err(|_| arguments_error())?;
    if !text.trim_start().starts_with('{') {
        return Err(arguments_error());
    }
    json_input::parse(text)
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

#[cfg(windows)]
async fn query(
    options: &Options,
    mut request: read::Request,
    id: String,
) -> Result<Value, McpError> {
    let name = options
        .profile
        .as_deref()
        .ok_or_else(|| McpError::new("unauthenticated"))?;
    let root = crate::mcp::platform::root()?;
    let executable = std::env::current_exe().map_err(|_| McpError::new("host_unavailable"))?;
    let credentials = profile::load_at(&root, name, &executable)?;
    // Confirm the host contract before projecting any business data.
    let capabilities = crate::mcp::ipc::call_at_with_id(
        root.clone(),
        credentials.clone(),
        "get_capabilities",
        json!({}),
        id.clone(),
    )
    .await?;
    validate_private_route(&capabilities, request.tool)?;
    validate_status(capabilities, &credentials.client_id)?;
    let mut pages = read::Pages::new();
    loop {
        let data = crate::mcp::ipc::call_at_with_id(
            root.clone(),
            credentials.clone(),
            request.tool,
            request.args.clone(),
            id.clone(),
        )
        .await?;
        if request.exact && data["items"].as_array().is_some_and(Vec::is_empty) {
            return Err(McpError::new("not_found"));
        }
        if !options.all || !request.paginated {
            return Ok(data);
        }
        let next = pages.push(data)?;
        match next {
            Some(cursor) => {
                request.args["cursor"] = json!(cursor);
            }
            None => return Ok(pages.finish()),
        }
    }
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

async fn execute(
    options: &Options,
    id: String,
    receipt_key: &mut Option<String>,
) -> Result<Value, McpError> {
    let input = options
        .input
        .as_deref()
        .map(read_input)
        .transpose()?
        .unwrap_or_else(|| json!({}));
    match &options.command {
        Command::Help => Ok(json!({"help":HELP})),
        Command::Version => {
            Ok(json!({"name":"in-line-cli","build":build_identity::current(),"cliPhase":4}))
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
            let _: CapabilityArgs = serde_json::from_value(input).map_err(|_| arguments_error())?;
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
        Command::Import => {
            #[cfg(windows)]
            {
                authorization::import(options.package.as_deref().ok_or_else(arguments_error)?, id)
                    .await
            }
            #[cfg(not(windows))]
            {
                Err(McpError::new("unsupported"))
            }
        }
        Command::Write(command) => {
            let request = write::build(command, options, input)?;
            *receipt_key = Some(request.key.clone());
            #[cfg(windows)]
            {
                dispatch_write(options, request, id).await
            }
            #[cfg(not(windows))]
            {
                let _ = request;
                Err(McpError::new("unsupported"))
            }
        }
        Command::Read(command) => {
            let request = read::build(command, options, input)?;
            #[cfg(windows)]
            {
                query(options, request, id).await
            }
            #[cfg(not(windows))]
            {
                let _ = request;
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
    let mut receipt_key = None;
    let result = match &parsed {
        Ok(options) => execute(options, id.clone(), &mut receipt_key).await,
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
    if let Some(key) = receipt_key {
        let kind = if parsed
            .as_ref()
            .is_ok_and(|o| matches!(o.command, Command::Write(parse::WriteCommand::Undo)))
        {
            "undo_request"
        } else {
            "mutation"
        };
        let context = json!({"idempotencyKey":key,"kind":kind,"lookup":"receipt get --idempotency-key <original> --profile <same>","neverBlindlyReplay":true});
        if let Some(data) = envelope.data.as_mut().and_then(Value::as_object_mut) {
            data.insert("cliReceipt".into(), context.clone());
        }
        if let Some(error) = envelope.error.as_mut() {
            error["cliReceipt"] = context;
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
            b"{}{}",
            b"{\"limit\":1,\"limit\":2}",
            b"{\"filters\":{\"text\":\"a\",\"text\":\"b\"}}",
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

#[cfg(windows)]
fn validate_private_route(capabilities: &Value, tool: &str) -> Result<(), McpError> {
    if tool.starts_with("cli_")
        && (capabilities["cli"]["version"] != 1
            || capabilities["cli"]["phase"].as_u64().is_none_or(|p| p < 4)
            || !capabilities["cli"]["privateRoutes"]
                .as_array()
                .is_some_and(|a| a.iter().any(|v| v == tool)))
    {
        return Err(McpError::new("incompatible"));
    }
    Ok(())
}
#[cfg(windows)]
async fn dispatch_write(
    options: &Options,
    request: write::Request,
    id: String,
) -> Result<Value, McpError> {
    let name = options
        .profile
        .as_deref()
        .ok_or_else(|| McpError::new("unauthenticated"))?;
    let root = crate::mcp::platform::root()?;
    let exe = std::env::current_exe().map_err(|_| McpError::new("host_unavailable"))?;
    let credentials = profile::load_at(&root, name, &exe)?;
    let capability = crate::mcp::ipc::call_at_with_id(
        root.clone(),
        credentials.clone(),
        "get_capabilities",
        json!({}),
        id.clone(),
    )
    .await?;
    validate_status(capability.clone(), &credentials.client_id)?;
    if capability["cli"]["version"] != 1
        || capability["cli"]["phase"].as_u64().is_none_or(|p| p < 4)
    {
        return Err(McpError::new("incompatible"));
    }
    validate_private_route(&capability, request.tool)?;
    if capability["permissions"]["write"] != true
        || capability["permissions"]["regularRead"] != true
    {
        return Err(McpError::new("forbidden"));
    }
    if let Some(title) = &request.create_title {
        if title.is_empty() {
            return Err(arguments_error());
        }
        let recorded = crate::mcp::ipc::call_at_with_id(
            root.clone(),
            credentials.clone(),
            "cli_get_receipt",
            json!({"idempotencyKey":request.key}),
            id.clone(),
        )
        .await;
        let fresh = match recorded {
            Ok(_) => false,
            Err(e) if e.code == "not_found" => true,
            Err(e) => return Err(e),
        };
        if fresh {
            let candidates = crate::mcp::ipc::call_at_with_id(
                root.clone(),
                credentials.clone(),
                "query_tasks",
                json!({"filters":{"text":title},"projection":["id","title"],"limit":100}),
                id.clone(),
            )
            .await?;
            if candidates["items"]
                .as_array()
                .is_some_and(|a| !a.is_empty())
                && request.args["intent"]["allowPossibleDuplicate"] != true
            {
                return Err(McpError::new("possible_duplicate"));
            }
        }
    }
    // Exactly one write dispatch. Connection/revision ambiguity is classified by IPC;
    // no new key, replay, approval or fallback identity is attempted here.
    let expected_audit = request.args["auditId"].clone();
    let mut value =
        crate::mcp::ipc::call_at_with_id(root, credentials, request.tool, request.args, id).await?;
    if request.tool == "request_undo" {
        if !value.is_object()
            || !matches!(
                value["status"].as_str(),
                Some("needs_user_approval" | "approved" | "rejected")
            )
            || value["undoRequestId"].as_i64().is_none_or(|n| n <= 0)
            || value["auditId"] != expected_audit
        {
            return Err(McpError::new("result_unknown"));
        }
        value["submissionOnly"] = json!(true);
    }
    write::validate_result(value)
}
