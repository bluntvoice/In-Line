use chrono::{Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const API_VERSION: u32 = 1;
pub const TOOLS: [&str; 7] = [
    "get_capabilities",
    "get_report_summary",
    "list_report_items",
    "query_tasks",
    "query_task_history",
    "query_work_calendar",
    "manage_saved_query",
];

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Credentials {
    pub client_id: String,
    pub token: String,
}
impl Credentials {
    pub fn from_env() -> Self {
        Self {
            client_id: std::env::var("IN_LINE_MCP_CLIENT_ID").unwrap_or_default(),
            token: std::env::var("IN_LINE_MCP_TOKEN").unwrap_or_default(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct McpError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub retry_after_seconds: Option<u64>,
}
impl McpError {
    pub fn new(code: &str) -> Self {
        let message = match code {
            "unauthenticated" => "请在In Line软件设置中为当前客户端授权并配置环境凭证",
            "revoked" => "客户端授权已撤销，请由用户在软件内重新授权",
            "paused" => "MCP已由用户暂停，请由用户在软件内恢复",
            "forbidden" => "当前客户端没有所需读取权限",
            "authorization_changed" => "读取期间授权发生变化，结果未释放；请按当前权限重新查询",
            "incompatible" => "接口或授权存储版本不兼容，请升级并重新检查连接",
            "rate_limited" => "调用过于频繁，请等待冷却后重试",
            "invalid_arguments" => "参数无效：只接受白名单字段；日期须为YYYY-MM-DD，时间范围为RFC3339；分页1至100，后续offset必须绑定快照且不能与cursor混用",
            "unsupported" => "当前阶段不提供此操作",
            "host_unavailable" => "主程序协调服务不可用，请启动或升级In Line后检查连接",
            "security_unavailable" => "安全存储不可用；请在软件内检查授权，禁止自动恢复旧凭证",
            "onboarding_unavailable" => "当前客户端自动接入仍在验证，入口暂未开放",
            "import_expired" => "接入提示词已过期，请回到软件重新发起接入",
            "import_used" => "本次接入已执行，不能重复使用；请查看已有结果",
            "import_in_doubt" => "接入曾中断，配置结果尚待核验；未自动重试或修改授权",
            "import_conflict" => "客户端配置冲突或发生变化，未覆盖其他配置",
            "import_invalid" => "接入包或客户端配置无效，未读取业务数据",
            "import_failed" => "接入验证失败，请在软件内检查；未回显凭证",
            "snapshot_expired" => "查询快照已过期或服务已重启，请重新开始查询",
            "cursor_invalid" => "分页参数、客户端或查询条件与快照不一致，请重新查询",
            "snapshot_invalid" => "数据代际或事项授权范围变化，旧快照结果未释放",
            "resource_limit" => "查询资源超过本机安全上限，请缩小范围或结束旧查询",
            "not_found" => "未找到当前授权范围内的事项或查询",
            "explicit_intent_required" => "保存或删除命名查询须有用户明确要求及写入授权",
            _ => "操作失败，请在本机检查软件状态；未自动修改业务数据",
        };
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
            retry_after_seconds: None,
        }
    }
}
impl From<String> for McpError {
    fn from(_: String) -> Self {
        Self::new("internal_error")
    }
}
impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for McpError {}

#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub api_version: u32,
    pub request_id: String,
    pub status: String,
    pub data: Option<Value>,
    pub error: Option<Value>,
}
impl Envelope {
    pub fn result(id: String, result: Result<Value, McpError>) -> Self {
        match result {
            Ok(data) => Self {
                api_version: API_VERSION,
                request_id: id,
                status: "ok".into(),
                data: Some(data),
                error: None,
            },
            Err(error) => Self {
                api_version: API_VERSION,
                request_id: id,
                status: "error".into(),
                data: None,
                error: Some(serde_json::to_value(error).expect("error serializable")),
            },
        }
    }
}

#[derive(Deserialize, Serialize, schemars::JsonSchema, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityArgs {}
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DateRangeArgs {
    pub start_date: String,
    pub end_date: String,
    pub filters: Option<super::query_types::QueryFilters>,
    pub snapshot: Option<String>,
    pub timezone_offset_minutes: Option<i32>,
}
#[derive(Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReportItemsArgs {
    pub start_date: String,
    pub end_date: String,
    /// 每页1至100条，默认100；超过上限明确拒绝。
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub filters: Option<super::query_types::QueryFilters>,
    pub snapshot: Option<String>,
    pub cursor: Option<String>,
    pub timezone_offset_minutes: Option<i32>,
}
pub fn report_range(start: &str, end: &str) -> Result<(String, String, i32), McpError> {
    let invalid = || McpError::new("invalid_arguments");
    let s = NaiveDate::parse_from_str(start, "%Y-%m-%d").map_err(|_| invalid())?;
    let e = NaiveDate::parse_from_str(end, "%Y-%m-%d").map_err(|_| invalid())?;
    if s.format("%Y-%m-%d").to_string() != start
        || e.format("%Y-%m-%d").to_string() != end
        || e < s
        || (e - s).num_days() > 370
    {
        return Err(invalid());
    }
    let start = Local
        .from_local_datetime(&s.and_hms_opt(0, 0, 0).ok_or_else(invalid)?)
        .single()
        .ok_or_else(invalid)?;
    let end = Local
        .from_local_datetime(
            &e.succ_opt()
                .ok_or_else(invalid)?
                .and_hms_opt(0, 0, 0)
                .ok_or_else(invalid)?,
        )
        .single()
        .ok_or_else(invalid)?;
    Ok((
        start.to_rfc3339(),
        end.to_rfc3339(),
        start.offset().local_minus_utc() / 60,
    ))
}
