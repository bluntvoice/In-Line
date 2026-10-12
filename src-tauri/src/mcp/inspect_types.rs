use super::contract::ReportItemsArgs;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectArgs {
    /// health/trace/compare/cache/recompute; no business repair or arbitrary SQL.
    pub action: String,
    pub report: Option<ReportItemsArgs>,
    pub right: Option<ReportItemsArgs>,
    pub metric: Option<String>,
    /// Exact task type or department for dimensional metrics.
    pub dimension: Option<String>,
    /// Existing trend bucket's YYYY-MM-DD start; never guesses a missing bucket.
    pub period_start: Option<String>,
    pub snapshot: Option<String>,
    pub cursor: Option<String>,
}
