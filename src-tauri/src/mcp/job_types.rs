use super::query_types::QueryFilters;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangesArgs {
    /// Optional RFC3339 event lower bound; initial current-state sync remains mandatory.
    pub since: Option<String>,
    /// events (every indexed mutation) or current (one current state per affected task).
    pub mode: Option<String>,
    pub filters: Option<QueryFilters>,
    pub projection: Option<Vec<String>>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JobsArgs {
    /// start/list/get/result/cancelWaiter/cancelJob. No business write tool can be scheduled.
    pub action: String,
    pub kind: Option<String>,
    pub arguments: Option<Value>,
    pub job_id: Option<String>,
    pub waiter_id: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    #[serde(default)]
    pub explicit_requery: bool,
    #[serde(default)]
    pub allow_latest_after_invalidation: bool,
    #[serde(default)]
    pub explicit_user_request: bool,
}
