use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryFilters {
    pub ids: Option<Vec<i64>>,
    pub permanent_numbers: Option<Vec<String>>,
    pub text: Option<String>,
    pub statuses: Option<Vec<String>>,
    pub departments: Option<Vec<String>>,
    pub task_types: Option<Vec<String>>,
    pub priorities: Option<Vec<String>>,
    pub workloads: Option<Vec<String>>,
    pub urgent: Option<bool>,
    pub scheduled: Option<bool>,
    pub active_queue: Option<bool>,
    /// all (default), topLevel or subtask.
    pub structure: Option<String>,
    pub parent_task_id: Option<i64>,
    /// all (default), active or archived. Trash still needs explicit includeTrash.
    pub archive: Option<String>,
    #[serde(default)]
    pub include_trash: bool,
    /// Date-only inclusive bounds; timestamps below are RFC3339, half-open.
    pub planned_from: Option<String>,
    pub planned_to: Option<String>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
    pub deadline_from: Option<String>,
    pub deadline_to: Option<String>,
    pub handled_from: Option<String>,
    pub handled_to: Option<String>,
}

#[derive(Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryArgs {
    pub filters: Option<QueryFilters>,
    pub projection: Option<Vec<String>>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub snapshot: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryArgs {
    pub task_id: i64,
    pub kinds: Option<Vec<String>>,
    #[serde(default)]
    pub include_voided: bool,
    #[serde(default)]
    pub include_trash: bool,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub snapshot: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CalendarArgs {
    pub start_date: String,
    pub end_date: String,
    pub timezone_offset_minutes: Option<i32>,
    pub filters: Option<QueryFilters>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
    pub snapshot: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedQueryArgs {
    /// list/get/save/delete; save/delete require explicit user intent and write permission.
    pub action: String,
    pub name: Option<String>,
    pub filters: Option<QueryFilters>,
    pub projection: Option<Vec<String>>,
    #[serde(default)]
    pub explicit_intent: bool,
}
