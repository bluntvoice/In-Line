//! P3 wire types: no arbitrary commands, SQL, filesystem or authorization changes.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Intent {
    pub summary: String,
    pub explicit_user_request: bool,
    #[serde(default)]
    pub replace_whole_text: bool,
    #[serde(default)]
    pub confirmed_real_work: bool,
    #[serde(default)]
    pub allow_possible_duplicate: bool,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldBase {
    pub value: Value,
    pub version: i64,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    pub task_id: Option<i64>,
    pub permanent_number: Option<String>,
    pub title: Option<String>,
    pub batch_ref: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextEdit {
    pub field: String,
    pub find: String,
    pub replace: String,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewTask {
    pub title: String,
    pub departments: Vec<String>,
    pub contacts: Vec<String>,
    pub task_type: String,
    #[serde(default)]
    pub details: String,
    #[serde(default)]
    pub internal_notes: String,
    pub priority: Option<String>,
    pub workload: Option<String>,
    pub requested_deadline: Option<String>,
    pub planned_date: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub enum TaskAction {
    CreateSubtask {
        parent: Target,
        #[serde(rename = "parentVersion")]
        parent_version: i64,
        task: NewTask,
        #[serde(default = "default_enqueue", rename = "enqueueToday")]
        enqueue_today: bool,
    },
    SetParent {
        target: Target,
        #[serde(rename = "taskVersion")]
        task_version: i64,
        parent: Option<Target>,
        #[serde(rename = "parentVersion")]
        parent_version: Option<i64>,
    },
    ReorderSubtasks {
        parent: Target,
        #[serde(rename = "parentVersion")]
        parent_version: i64,
        #[serde(rename = "taskIds")]
        task_ids: Vec<i64>,
        #[serde(rename = "taskVersions")]
        task_versions: BTreeMap<String, i64>,
    },
    Queue {
        target: Target,
        #[serde(rename = "taskVersion")]
        task_version: i64,
        mode: String,
        #[serde(default, rename = "inheritDeadline")]
        inherit_deadline: bool,
    },
    Reschedule {
        target: Target,
        #[serde(rename = "taskVersion")]
        task_version: i64,
        #[serde(rename = "plannedDate")]
        planned_date: String,
        #[serde(rename = "requestedDeadline")]
        requested_deadline: Option<String>,
    },
    Archive {
        target: Target,
        #[serde(rename = "taskVersion")]
        task_version: i64,
        #[serde(default, rename = "includeCompletedSubtasks")]
        include_completed_subtasks: bool,
    },
    Trash {
        target: Target,
        #[serde(rename = "taskVersion")]
        task_version: i64,
        #[serde(default, rename = "includeSubtasks")]
        include_subtasks: bool,
    },
    Restore {
        target: Target,
        #[serde(rename = "taskVersion")]
        task_version: i64,
    },
    CorrectWorkEvent {
        target: Target,
        #[serde(rename = "taskVersion")]
        task_version: i64,
        #[serde(rename = "eventId")]
        event_id: i64,
        #[serde(rename = "resultStatus")]
        result_status: String,
        #[serde(rename = "handledAt")]
        handled_at: String,
        note: String,
        #[serde(rename = "confirmHistoricalImpact")]
        confirm_historical_impact: bool,
    },
    VoidWorkEvent {
        target: Target,
        #[serde(rename = "taskVersion")]
        task_version: i64,
        #[serde(rename = "eventId")]
        event_id: i64,
        #[serde(rename = "confirmHistoricalImpact")]
        confirm_historical_impact: bool,
    },
    Create {
        task: NewTask,
    },
    Patch {
        target: Target,
        #[serde(rename = "fieldBase")]
        field_base: BTreeMap<String, FieldBase>,
        patch: BTreeMap<String, Value>,
        #[serde(default, rename = "textEdits")]
        text_edits: Vec<TextEdit>,
    },
    SetStatus {
        target: Target,
        #[serde(rename = "taskVersion")]
        task_version: i64,
        status: String,
    },
    SetUrgent {
        target: Target,
        #[serde(rename = "taskVersion")]
        task_version: i64,
        #[serde(rename = "isUrgent")]
        is_urgent: bool,
        requester: String,
        #[serde(rename = "urgentReason")]
        reason: String,
    },
    RecordWorkEvent {
        target: Target,
        #[serde(rename = "taskVersion")]
        task_version: i64,
        #[serde(rename = "resultStatus")]
        result_status: String,
        #[serde(rename = "handledAt")]
        handled_at: String,
        note: String,
        #[serde(default, rename = "syncStatus")]
        sync_status: bool,
    },
}
fn default_enqueue() -> bool {
    true
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchItem {
    pub ref_key: Option<String>,
    pub operation: TaskAction,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchArgs {
    pub action: String,
    pub operations: Vec<BatchItem>,
    pub idempotency_key: String,
    pub intent: Intent,
    pub reason: String,
    pub expected_basis: Option<crate::mcp::basis::DataVersion>,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BusinessDataArgs {
    pub action: String,
    pub idempotency_key: String,
    pub intent: Intent,
    pub reason: String,
    pub format: Option<String>,
    pub filters: Option<crate::mcp::query_types::QueryFilters>,
    pub projection: Option<Vec<String>>,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MutateArgs {
    pub idempotency_key: String,
    pub intent: Intent,
    pub reason: String,
    #[serde(flatten)]
    pub operation: TaskAction,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UndoArgs {
    pub audit_id: i64,
    pub idempotency_key: String,
    pub intent: Intent,
    pub reason: String,
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreferenceArgs {
    pub action: String,
    pub patch: Option<BTreeMap<String, String>>,
    pub field_base: Option<BTreeMap<String, Option<String>>>,
    pub field_versions: Option<BTreeMap<String, i64>>,
    pub idempotency_key: Option<String>,
    pub intent: Option<Intent>,
    pub reason: Option<String>,
}
