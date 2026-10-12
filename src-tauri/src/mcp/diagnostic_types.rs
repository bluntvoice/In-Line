//! Typed technical facts only. No API accepts raw log text, business bodies or file paths.
use super::basis::DataVersion;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticArgs {
    pub error_id: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Event {
    #[serde(default)]
    pub receipt_hash: Option<String>,
    pub request_id: String,
    pub client: Option<String>,
    pub tool: String,
    pub module: String,
    pub code: String,
    pub at: i64,
    pub duration_ms: u64,
    pub job_id: Option<String>,
    pub round: Option<u32>,
    pub audit_id: Option<i64>,
    pub data_version: Option<DataVersion>,
    pub build: Value,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Episode {
    #[serde(default)]
    pub receipt_hash: Option<String>,
    #[serde(default)]
    pub job_id: Option<String>,
    #[serde(default)]
    pub execution_round: Option<u32>,
    #[serde(default)]
    pub audit_id: Option<i64>,
    pub id: String,
    pub number: u32,
    pub started: i64,
    pub last_seen: i64,
    pub evidence_count: u64,
    pub first_error_id: String,
    pub last_error_id: String,
    pub resolved_at: Option<i64>,
    pub state: String,
    pub verification_started: Option<i64>,
    pub verification_successes: u32,
    pub muted_at: Option<i64>,
    pub muted_until: Option<i64>,
    pub archived: bool,
    pub build: Value,
    pub data_version: Option<DataVersion>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fault {
    pub id: String,
    pub client: Option<String>,
    pub tool: String,
    pub module: String,
    pub code: String,
    pub severity: String,
    pub attention: String,
    pub episodes: Vec<Episode>,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct State {
    pub events: Vec<Event>,
    pub faults: Vec<Fault>,
    pub last_clients: Vec<Event>,
}
pub fn known_code(code: &str) -> &str {
    match code {
        "ok"
        | "unauthenticated"
        | "cancelled"
        | "sensitive_notice_required"
        | "idempotency_conflict"
        | "possible_duplicate"
        | "needs_disambiguation"
        | "business_rule"
        | "cache_capacity"
        | "cache_bypass"
        | "cache_cleared"
        | "index_rebuilt"
        | "audit_cleared"
        | "security_unavailable"
        | "internal_error"
        | "host_unavailable"
        | "verification_failed"
        | "result_unknown"
        | "resync_required"
        | "snapshot_invalid"
        | "snapshot_expired"
        | "cursor_invalid"
        | "authorization_changed"
        | "forbidden"
        | "revoked"
        | "paused"
        | "rate_limited"
        | "invalid_arguments"
        | "unsupported"
        | "not_found"
        | "conflict"
        | "field_conflict"
        | "explicit_intent_required"
        | "result_expired"
        | "job_not_complete"
        | "waiter_cancelled"
        | "pause_expired"
        | "recovery_expired"
        | "clock_invalid"
        | "resource_limit"
        | "incompatible" => code,
        _ => "internal_error",
    }
}
pub fn severity(code: &str) -> Option<&'static str> {
    match code {
        "security_unavailable" | "verification_failed" | "result_unknown" => Some("P1"),
        "internal_error" | "host_unavailable" | "resync_required" | "clock_invalid"
        | "incompatible" => Some("P2"),
        "cache_capacity" => Some("P3"),
        _ => None, // Expected authorization/business/expiry rejections are diagnosable, not defect episodes.
    }
}
pub fn module(tool: &str) -> &'static str {
    match tool {
        "query_changes" => "changes",
        "manage_jobs" => "jobs",
        "get_report_summary"
        | "list_report_items"
        | "query_tasks"
        | "query_task_history"
        | "query_work_calendar"
        | "inspect_data" => "query",
        "get_diagnostic" => "diagnostics",
        _ => "service",
    }
}
pub fn guidance(code: &str) -> &'static str {
    match code {
        "security_unavailable" => {
            "在软件内检查独立授权及私有存储；不要发送Token或自行放宽文件权限。"
        }
        "verification_failed" | "result_unknown" => {
            "先用原幂等键核验已提交结果；不要新建请求键或盲目重复写入。"
        }
        "resync_required" | "snapshot_invalid" | "snapshot_expired" | "cursor_invalid" => {
            "检查数据代际/索引健康及当前授权；重新建立全量同步或报告快照，不重构缺失历史。"
        }
        "authorization_changed" | "revoked" | "forbidden" | "paused" => {
            "核对软件内当前客户端权限和全局暂停；AI不能自行修改授权。"
        }
        "rate_limited" | "pause_expired" => {
            "遵守暂缓期限，停止高频重试；首次暂停30分钟上限不续期。"
        }
        "recovery_expired" | "clock_invalid" => {
            "可靠检查点恢复最多24小时；先核对本机时间，再由用户明确发起新只读查询。"
        }
        "cache_capacity" | "resource_limit" => {
            "缩小报告范围或清理仅报告缓存；不要删除数据库、审计、任务检查点或正式导出。"
        }
        "incompatible" | "host_unavailable" => {
            "检查整套主程序/sidecar版本与本机连接，升级和配置验证不等于当前会话已接入。"
        }
        _ => {
            "使用此关联编号在软件内检查技术状态；此概述未证明根因，不自动修复业务数据或调用外部AI。"
        }
    }
}
