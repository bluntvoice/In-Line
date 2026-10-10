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
}
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub enum TaskAction {
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
