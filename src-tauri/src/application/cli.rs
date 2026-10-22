//! Private CLI application routes. They are not MCP tools or grant/approval APIs.
#[cfg(windows)]
use crate::mcp::security::Authorization;
use crate::{
    database::Database,
    mcp::{contract::McpError, write_types::Intent},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
pub(crate) const ROUTES: [&str; 5] = [
    "cli_get_receipt",
    "cli_parent_candidates",
    "cli_complete_task",
    "cli_create_subtask",
    "cli_report",
];
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReceiptArgs {
    pub idempotency_key: String,
    pub kind: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CandidateArgs {
    pub task_id: i64,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub snapshot: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CompleteArgs {
    pub task_id: i64,
    pub task_version: i64,
    pub task_versions: BTreeMap<String, i64>,
    pub include_eligible_subtasks: bool,
    pub idempotency_key: String,
    pub intent: Intent,
    pub reason: String,
}
pub(crate) fn capabilities() -> Value {
    json!({"version":1,"phase":5,"privateRoutes":ROUTES,"writeTools":["mutate_task","request_undo","cli_complete_task","cli_create_subtask"],"receiptLookupReadOnly":true})
}
pub(crate) fn write_route(tool: &str) -> bool {
    matches!(tool, "cli_complete_task" | "cli_create_subtask")
}
#[cfg(windows)]
pub(crate) fn execute(
    db: &Database,
    client: &str,
    auth: &Authorization,
    tool: &str,
    args: Value,
) -> Result<Value, McpError> {
    match tool {
        "cli_get_receipt" => db.cli_receipt(
            client,
            auth,
            serde_json::from_value::<ReceiptArgs>(args)
                .map_err(|_| McpError::new("invalid_arguments"))?,
        ),
        "cli_create_subtask" => db.cli_create_subtask(
            client,
            auth,
            serde_json::from_value::<ChildArgs>(args)
                .map_err(|_| McpError::new("invalid_arguments"))?,
        ),
        "cli_complete_task" => db.cli_complete(
            client,
            auth,
            serde_json::from_value::<CompleteArgs>(args)
                .map_err(|_| McpError::new("invalid_arguments"))?,
        ),
        _ => Err(McpError::new("unsupported")),
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ChildArgs {
    pub parent_task_id: i64,
    pub parent_version: i64,
    pub task: ChildTask,
    pub idempotency_key: String,
    pub intent: Intent,
    pub reason: String,
    #[serde(default = "yes")]
    pub enqueue_today: bool,
}
fn yes() -> bool {
    true
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ReportArgs {
    pub period: String,
    pub date: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub filters: Option<crate::mcp::query_types::QueryFilters>,
    pub snapshot: Option<String>,
    pub timezone_offset_minutes: Option<i32>,
    #[serde(default)]
    pub text: bool,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ChildTask {
    pub title: String,
    #[serde(default)]
    pub details: String,
    #[serde(default)]
    pub internal_notes: String,
    pub planned_date: Option<String>,
    pub departments: Option<Vec<String>>,
    pub contacts: Option<Vec<String>>,
    pub task_type: Option<String>,
    pub priority: Option<String>,
    pub workload: Option<String>,
    pub requested_deadline: Option<String>,
}
