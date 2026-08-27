use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const ALL_STATUSES: [&str; 10] = [
    "pending",
    "processing",
    "waiting_materials",
    "waiting_confirmation",
    "waiting_counterparty_confirmation",
    "paused",
    "processed",
    "completed",
    "cancelled",
    "archived",
];

pub const PRIORITIES: [&str; 4] = ["normal", "elevated", "urgent", "critical"];
pub const WORKLOADS: [&str; 4] = ["simple", "standard", "complex", "major"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalTask {
    pub id: i64,
    pub permanent_number: String,
    pub daily_sequence: i64,
    pub ticket_date: String,
    pub department: String,
    pub departments: Vec<String>,
    pub contact: String,
    pub contacts: Vec<String>,
    pub task_type: String,
    pub title: String,
    pub details: String,
    pub status: String,
    pub priority: String,
    pub workload: String,
    pub is_urgent: bool,
    pub urgent_requester: String,
    pub urgent_reason: String,
    pub requested_deadline: Option<String>,
    pub requested_deadline_label: Option<String>,
    pub internal_notes: String,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub archived_at: Option<String>,
    pub deleted_at: Option<String>,
    pub custom_sort_order: i64,
    pub processing_rounds: i64,
    pub has_active_queue: bool,
    pub deferred_entered_at: Option<String>,
    pub is_import_conflict: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    pub id: Option<i64>,
    pub department: String,
    #[serde(default)]
    pub departments: Vec<String>,
    #[serde(default)]
    pub contact: String,
    #[serde(default)]
    pub contacts: Vec<String>,
    pub task_type: String,
    pub title: String,
    pub details: String,
    pub status: String,
    pub priority: String,
    pub workload: String,
    pub is_urgent: bool,
    #[serde(default)]
    pub urgent_requester: String,
    #[serde(default)]
    pub urgent_reason: String,
    pub requested_deadline: Option<String>,
    #[serde(default)]
    pub requested_deadline_label: Option<String>,
    #[serde(default)]
    pub internal_notes: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskLog {
    pub id: i64,
    pub task_id: i64,
    pub log_type: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskWorkEvent {
    pub id: i64,
    pub task_id: i64,
    pub result_status: String,
    pub handled_at: String,
    pub task_type_snapshot: String,
    pub source: String,
    pub note: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_first_valid: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkEventInput {
    pub task_id: i64,
    pub result_status: String,
    pub handled_at: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub sync_status: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueInput {
    pub id: i64,
    #[serde(default)]
    pub inherit_deadline: bool,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeTaskInput {
    pub target_task_id: i64,
    pub source_task_id: i64,
    #[serde(default)]
    pub deduplicate_records: bool,
    #[serde(default)]
    pub trash_source: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsRange {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize, Default, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsSummary {
    pub handled_tasks: i64,
    pub processed: i64,
    pub completed: i64,
    pub waiting_materials: i64,
    pub waiting_confirmation: i64,
    pub waiting_counterparty_confirmation: i64,
    pub rate_mode: String,
    pub rate_numerator: i64,
    pub rate_denominator: i64,
    pub completion_rate: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TaskTypeStatistics {
    pub task_type: String,
    pub handled_tasks: i64,
    pub completed: i64,
    pub pending_follow_up: i64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DepartmentStatistics {
    pub department: String,
    pub handled_tasks: i64,
    pub completed: i64,
    pub pending_follow_up: i64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TrendPoint {
    pub period_start: String,
    pub handled_tasks: i64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsResult {
    pub range: StatisticsRange,
    pub summary: StatisticsSummary,
    pub by_task_type: Vec<TaskTypeStatistics>,
    pub by_department: Vec<DepartmentStatistics>,
    pub trend: Vec<TrendPoint>,
    pub trend_granularity: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsDetail {
    pub task_id: i64,
    pub permanent_number: String,
    pub title: String,
    pub department: String,
    pub contact: String,
    pub result_status: String,
    pub first_handled_at: String,
    pub last_handled_at: String,
    pub handling_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkCalendarRange {
    pub start: String,
    pub end: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkCalendarSummary {
    pub handled_tasks: i64,
    pub handling_rounds: i64,
    pub completed_tasks: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkCalendarInterval {
    pub queue_entry_id: i64,
    pub enqueued_at: String,
    pub closed_at: Option<String>,
    pub round_index: i64,
    pub result_status: Option<String>,
    pub handled_at: Option<String>,
    pub current_active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkCalendarEvent {
    pub event_id: i64,
    pub task_id: i64,
    pub permanent_number: String,
    pub title: String,
    pub task_type: String,
    pub result_status: String,
    pub handled_at: String,
    pub round_index: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkCalendarTask {
    pub task_id: i64,
    pub permanent_number: String,
    pub title: String,
    pub task_type: String,
    pub intervals: Vec<WorkCalendarInterval>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkCalendarResult {
    pub range: WorkCalendarRange,
    pub summary: WorkCalendarSummary,
    pub tasks: Vec<WorkCalendarTask>,
    pub events: Vec<WorkCalendarEvent>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReportWorkEvent {
    pub result_status: String,
    pub handled_at: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReportItem {
    pub task_id: i64,
    pub permanent_number: String,
    pub title: String,
    pub departments: Vec<String>,
    pub task_type: String,
    pub current_status: String,
    pub workload: String,
    pub completed_at: Option<String>,
    pub work_events: Vec<ReportWorkEvent>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReportItemsPage {
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
    pub has_more: bool,
    pub items: Vec<ReportItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketSnapshot {
    pub task: LegalTask,
    pub queue_ahead: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterData {
    pub departments: Vec<String>,
    pub task_types: Vec<String>,
    pub contacts: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub modified_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupMergeResult {
    pub added_tasks: usize,
    pub merged_tasks: usize,
    pub conflict_tasks: usize,
    pub applied_settings: usize,
    pub conflicts: Vec<BackupConflictItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupConflictItem {
    pub task_id: i64,
    pub permanent_number: String,
    pub source_title: String,
    pub imported_title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapData {
    pub queue: Vec<LegalTask>,
    pub archive: Vec<LegalTask>,
    pub trash: Vec<LegalTask>,
    pub masters: MasterData,
    pub settings: HashMap<String, String>,
    pub backups: Vec<BackupInfo>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskView {
    Queue,
    Archive,
    Trash,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MoveDirection {
    Up,
    Down,
}

pub fn normalized_contacts(input: &TaskInput) -> Vec<String> {
    let source = if input.contacts.is_empty() {
        vec![input.contact.as_str()]
    } else {
        input.contacts.iter().map(String::as_str).collect()
    };
    let mut contacts = Vec::new();
    for value in source {
        let name = value.trim();
        if !name.is_empty() && !contacts.iter().any(|existing| existing == name) {
            contacts.push(name.to_string());
        }
    }
    contacts
}

pub fn normalized_departments(input: &TaskInput) -> Vec<String> {
    let source = if input.departments.is_empty() {
        vec![input.department.as_str()]
    } else {
        input.departments.iter().map(String::as_str).collect()
    };
    let mut departments = Vec::new();
    for value in source {
        let name = value.trim();
        if !name.is_empty() && !departments.iter().any(|existing| existing == name) {
            departments.push(name.to_string());
        }
    }
    departments
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenTaskAction {
    pub id: i64,
    pub action: String,
}

pub fn validate_task_input(input: &TaskInput) -> Result<(), String> {
    let contacts = normalized_contacts(input);
    let departments = normalized_departments(input);
    let required = [
        (
            "部门或团队",
            if departments.is_empty() {
                ""
            } else {
                "已填写"
            },
        ),
        ("对接人", if contacts.is_empty() { "" } else { "已填写" }),
        ("事项类型", input.task_type.trim()),
        ("事项标题", input.title.trim()),
    ];
    let missing: Vec<&str> = required
        .iter()
        .filter_map(|(label, value)| value.is_empty().then_some(*label))
        .collect();
    if !missing.is_empty() {
        return Err(format!("请填写{}", missing.join("、")));
    }
    if input.task_type.chars().count() > 100
        || input.title.chars().count() > 100
        || departments
            .iter()
            .any(|department| department.chars().count() > 100)
        || contacts.iter().any(|contact| contact.chars().count() > 100)
    {
        return Err("部门、对接人、事项类型和标题均不能超过 100 个字符".into());
    }
    if contacts.len() > 10 {
        return Err("每个事项最多可选择 10 位对接人".into());
    }
    if departments.len() > 10 {
        return Err("每个事项最多可选择 10 个部门或团队".into());
    }
    if input.details.chars().count() > 10_000 || input.internal_notes.chars().count() > 10_000 {
        return Err("事项详情和内部备注均不能超过 10000 个字符".into());
    }
    if input
        .requested_deadline_label
        .as_deref()
        .is_some_and(|value| value.chars().count() > 50)
    {
        return Err("截止时间说明不能超过 50 个字符".into());
    }
    if !ALL_STATUSES.contains(&input.status.as_str()) {
        return Err("事项状态无效".into());
    }
    if !PRIORITIES.contains(&input.priority.as_str()) {
        return Err("优先级无效".into());
    }
    if !WORKLOADS.contains(&input.workload.as_str()) {
        return Err("预计工作量无效".into());
    }
    let clears_urgent = matches!(
        input.status.as_str(),
        "waiting_materials"
            | "waiting_confirmation"
            | "waiting_counterparty_confirmation"
            | "paused"
            | "processed"
            | "completed"
    );
    if input.is_urgent
        && !clears_urgent
        && (input.urgent_requester.trim().is_empty() || input.urgent_reason.trim().is_empty())
    {
        return Err("加急事项需要填写加急申请人和加急原因".into());
    }
    Ok(())
}
