use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const ALL_STATUSES: [&str; 8] = [
    "pending",
    "processing",
    "waiting_materials",
    "waiting_confirmation",
    "paused",
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
    pub contact: String,
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
    pub internal_notes: String,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub archived_at: Option<String>,
    pub deleted_at: Option<String>,
    pub custom_sort_order: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    pub id: Option<i64>,
    pub department: String,
    pub contact: String,
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
pub struct MasterData {
    pub departments: Vec<String>,
    pub task_types: Vec<String>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenTaskAction {
    pub id: i64,
    pub action: String,
}

pub fn validate_task_input(input: &TaskInput) -> Result<(), String> {
    let required = [
        ("部门或团队", input.department.trim()),
        ("对接人", input.contact.trim()),
        ("事项类型", input.task_type.trim()),
        ("事项标题", input.title.trim()),
        ("事项详情", input.details.trim()),
    ];
    let missing: Vec<&str> = required
        .iter()
        .filter_map(|(label, value)| value.is_empty().then_some(*label))
        .collect();
    if !missing.is_empty() {
        return Err(format!("请填写{}", missing.join("、")));
    }
    if input.department.chars().count() > 100
        || input.contact.chars().count() > 100
        || input.task_type.chars().count() > 100
        || input.title.chars().count() > 100
    {
        return Err("部门、对接人、事项类型和标题均不能超过 100 个字符".into());
    }
    if input.details.chars().count() > 10_000 || input.internal_notes.chars().count() > 10_000 {
        return Err("事项详情和内部备注均不能超过 10000 个字符".into());
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
    if input.is_urgent
        && (input.urgent_requester.trim().is_empty() || input.urgent_reason.trim().is_empty())
    {
        return Err("加急事项需要填写加急申请人和加急原因".into());
    }
    Ok(())
}
