//! Typed read-only request construction. All business decisions stay on the host.
use super::{
    arguments_error,
    parse::{Options, ReadCommand},
};
use crate::mcp::{
    contract::*, diagnostic_types::DiagnosticArgs, inspect_types::InspectArgs, query_types::*,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Map, Value};

pub(super) struct Request {
    pub tool: &'static str,
    pub args: Value,
    pub paginated: bool,
    pub exact: bool,
}
fn insert(map: &mut Map<String, Value>, key: &str, value: Value) -> Result<(), McpError> {
    if map.contains_key(key) {
        return Err(arguments_error());
    }
    map.insert(key.into(), value);
    Ok(())
}
fn typed<T: DeserializeOwned + Serialize>(v: Value) -> Result<Value, McpError> {
    serde_json::to_value(serde_json::from_value::<T>(v).map_err(|_| arguments_error())?)
        .map_err(|_| arguments_error())
}
fn forced_filter(args: &mut Map<String, Value>, key: &str, value: Value) -> Result<(), McpError> {
    let filters = args.entry("filters").or_insert_with(|| json!({}));
    insert(
        filters.as_object_mut().ok_or_else(arguments_error)?,
        key,
        value,
    )
}
fn list(value: &str, number: bool) -> Result<Value, McpError> {
    let parts: Vec<_> = value.split(',').collect();
    if parts.is_empty() || parts.len() > 100 || parts.iter().any(|p| p.is_empty() || p.trim() != *p)
    {
        return Err(arguments_error());
    }
    if number {
        parts
            .iter()
            .map(|p| {
                p.parse::<i64>()
                    .ok()
                    .filter(|n| *n > 0)
                    .map(Value::from)
                    .ok_or_else(arguments_error)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array)
    } else {
        Ok(json!(parts))
    }
}
pub(super) fn build(
    command: &ReadCommand,
    options: &Options,
    input: Value,
) -> Result<Request, McpError> {
    use ReadCommand::*;
    if matches!(command, ParentCandidates(_) | Receipt) {
        return private_read(command, options, input);
    }
    let mut args = input.as_object().ok_or_else(arguments_error)?.clone();
    let task = matches!(
        command,
        Tasks | Get(_) | Search(_) | Ticket(_) | Children(_) | Queue | Future
    );
    let history = matches!(command, History(_) | QueueHistory(_));
    let report = matches!(command, Summary | ReportItems);
    let calendar = matches!(command, Calendar);
    let paginated = task || history || calendar || matches!(command, ReportItems);
    if options.all
        && (!paginated
            || args.contains_key("cursor")
            || options.flags.contains_key("--cursor")
            || args.contains_key("offset"))
    {
        return Err(arguments_error());
    }
    for (flag, value) in &options.flags {
        let root = match flag.as_str() {
            "--limit" if paginated => Some((
                "limit",
                json!(value.parse::<i64>().map_err(|_| arguments_error())?),
            )),
            "--cursor" if paginated => Some(("cursor", json!(value))),
            "--snapshot" if paginated || report => Some(("snapshot", json!(value))),
            "--from" if calendar || report => Some(("startDate", json!(value))),
            "--to" if calendar || report => Some(("endDate", json!(value))),
            "--timezone-offset-minutes" if calendar || report => Some((
                "timezoneOffsetMinutes",
                json!(value.parse::<i32>().map_err(|_| arguments_error())?),
            )),
            "--projection" if task => Some(("projection", list(value, false)?)),
            "--kinds" if history => Some(("kinds", list(value, false)?)),
            "--include-voided" if history => Some(("includeVoided", json!(true))),
            "--include-trash" if history => Some(("includeTrash", json!(true))),
            _ => None,
        };
        if let Some((key, value)) = root {
            insert(&mut args, key, value)?;
            continue;
        }
        if !task && !calendar && !report {
            return Err(arguments_error());
        }
        let (key, value) = match flag.as_str() {
            "--ids" => ("ids", list(value, true)?),
            "--permanent-numbers" => ("permanentNumbers", list(value, false)?),
            "--text" => ("text", json!(value)),
            "--statuses" => ("statuses", list(value, false)?),
            "--departments" => ("departments", list(value, false)?),
            "--task-types" => ("taskTypes", list(value, false)?),
            "--priorities" => ("priorities", list(value, false)?),
            "--workloads" => ("workloads", list(value, false)?),
            "--urgent" | "--scheduled" | "--active-queue" => {
                let key = match flag.as_str() {
                    "--urgent" => "urgent",
                    "--scheduled" => "scheduled",
                    _ => "activeQueue",
                };
                let b = match value.as_str() {
                    "true" => true,
                    "false" => false,
                    _ => return Err(arguments_error()),
                };
                (key, json!(b))
            }
            "--include-trash" => ("includeTrash", json!(true)),
            "--parent-task-id" => (
                "parentTaskId",
                json!(value
                    .parse::<i64>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(arguments_error)?),
            ),
            "--structure" => ("structure", json!(value)),
            "--archive" => ("archive", json!(value)),
            "--planned-from" => ("plannedFrom", json!(value)),
            "--planned-to" => ("plannedTo", json!(value)),
            "--created-from" => ("createdFrom", json!(value)),
            "--created-to" => ("createdTo", json!(value)),
            "--deadline-from" => ("deadlineFrom", json!(value)),
            "--deadline-to" => ("deadlineTo", json!(value)),
            "--handled-from" => ("handledFrom", json!(value)),
            "--handled-to" => ("handledTo", json!(value)),
            _ => return Err(arguments_error()),
        };
        forced_filter(&mut args, key, value)?;
    }
    match command {
        Get(id) | Ticket(id) => forced_filter(&mut args, "ids", json!([id]))?,
        Search(text) => forced_filter(&mut args, "text", json!(text))?,
        Children(id) => forced_filter(&mut args, "parentTaskId", json!(id))?,
        Queue => forced_filter(&mut args, "activeQueue", json!(true))?,
        Future => forced_filter(&mut args, "scheduled", json!(true))?,
        History(id) | QueueHistory(id) => {
            insert(&mut args, "taskId", json!(id))?;
        }
        Health => {
            insert(&mut args, "action", json!("health"))?;
        }
        Diagnostic(id) => {
            insert(&mut args, "errorId", json!(id))?;
        }
        _ => {}
    }
    if matches!(command, QueueHistory(_)) {
        insert(&mut args, "kinds", json!(["queue"]))?;
    }
    if matches!(command, Ticket(_)) {
        insert(
            &mut args,
            "projection",
            json!([
                "id",
                "permanentNumber",
                "dailySequence",
                "ticketDate",
                "plannedDate",
                "isScheduled",
                "hasActiveQueue",
                "processingRounds"
            ]),
        )?;
    }
    if args
        .get("limit")
        .is_some_and(|v| v.as_i64().is_none_or(|n| !(1..=100).contains(&n)))
    {
        return Err(arguments_error());
    }
    let value = Value::Object(args);
    let (tool, args) = if task {
        ("query_tasks", typed::<QueryArgs>(value)?)
    } else if history {
        ("query_task_history", typed::<HistoryArgs>(value)?)
    } else if calendar {
        ("query_work_calendar", typed::<CalendarArgs>(value)?)
    } else {
        match command {
            Summary => ("get_report_summary", typed::<DateRangeArgs>(value)?),
            ReportItems => ("list_report_items", typed::<ReportItemsArgs>(value)?),
            Health => ("inspect_data", typed::<InspectArgs>(value)?),
            Diagnostic(_) => ("get_diagnostic", typed::<DiagnosticArgs>(value)?),
            _ => return Err(arguments_error()),
        }
    };
    // health is a fixed read endpoint; reject every unrelated inspect field.
    if matches!(command, Health)
        && args
            .as_object()
            .is_some_and(|m| m.iter().any(|(k, v)| k != "action" && !v.is_null()))
    {
        return Err(arguments_error());
    }
    Ok(Request {
        tool,
        args,
        paginated,
        exact: matches!(command, Get(_) | Ticket(_)),
    })
}

/// Validate bounded traversal without changing the original per-page response.
pub(super) struct Pages {
    pub values: Vec<Value>,
    snapshot: Option<String>,
    items: usize,
    bytes: usize,
}
impl Pages {
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            snapshot: None,
            items: 0,
            bytes: 0,
        }
    }
    pub fn push(&mut self, value: Value) -> Result<Option<String>, McpError> {
        let invalid = || McpError::new("snapshot_invalid");
        let snapshot = value["meta"]["snapshotId"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(invalid)?;
        if value["meta"]["complete"] != true
            || self.snapshot.as_deref().is_some_and(|s| s != snapshot)
        {
            return Err(invalid());
        }
        let page = value.get("page").unwrap_or(&value);
        let count = page["items"].as_array().ok_or_else(invalid)?.len();
        let next = match value.get("nextCursor") {
            Some(Value::Null) => None,
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => return Err(invalid()),
        };
        if page["hasMore"].as_bool() != Some(next.is_some()) {
            return Err(invalid());
        }
        self.items += count;
        self.bytes += serde_json::to_vec(&value).map_err(|_| invalid())?.len();
        if self.values.len() >= 100 || self.items > 10000 || self.bytes > 16 * 1024 * 1024 {
            return Err(McpError::new("resource_limit"));
        }
        self.snapshot = Some(snapshot.into());
        self.values.push(value);
        Ok(next)
    }
    pub fn finish(self) -> Value {
        json!({"pages":self.values,"pagination":{"all":true,"complete":true,"snapshotId":self.snapshot,"pageCount":self.values.len(),"itemsReturned":self.items}})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(args: &[&str], input: Value) -> Result<Request, McpError> {
        let o = super::super::parse::parse(args.iter().map(std::ffi::OsString::from).collect())
            .unwrap();
        let super::super::parse::Command::Read(c) = &o.command else {
            panic!()
        };
        build(c, &o, input)
    }
    #[test]
    fn typed_read_routes_reject_ambiguity_unknown_fields_and_writes() {
        assert_eq!(
            request(&["queue", "list"], json!({})).unwrap().args["filters"]["activeQueue"],
            true
        );
        assert!(request(&["task", "get", "1"], json!({"filters":{"ids":[2]}})).is_err());
        assert!(request(&["task", "list", "--limit", "101"], json!({})).is_err());
        assert!(request(&["task", "list", "--limit", "10"], json!({"limit":10})).is_err());
        assert!(request(&["task", "list"], json!({"filters":{"sql":"hidden"}})).is_err());
        assert!(request(&["data", "health"], json!({"action":"recompute"})).is_err());
        assert!(request(&["data", "health"], json!({"metric":"queued"})).is_err());
        assert!(request(&["stats", "summary"], json!({})).is_err());
        assert!(request(&["task", "list", "--all"], json!({"cursor":"opaque"})).is_err());
        assert_eq!(
            request(&["task", "search", "中文"], json!({}))
                .unwrap()
                .args["filters"]["text"],
            "中文"
        );
    }
    #[test]
    fn traversal_refuses_changed_incomplete_or_unbounded_snapshot() {
        let page = |s: &str| json!({"items":[],"hasMore":false,"nextCursor":null,"meta":{"snapshotId":s,"complete":true}});
        let mut p = Pages::new();
        assert!(p.push(page("a")).unwrap().is_none());
        assert!(p.push(page("b")).is_err());
        let mut p = Pages::new();
        for _ in 0..100 {
            p.push(page("a")).unwrap();
        }
        assert_eq!(p.push(page("a")).unwrap_err().code, "resource_limit");
    }
}

fn private_read(
    command: &ReadCommand,
    options: &Options,
    input: Value,
) -> Result<Request, McpError> {
    let mut args = input.as_object().ok_or_else(arguments_error)?.clone();
    let candidates = matches!(command, ReadCommand::ParentCandidates(_));
    if options.all && !candidates {
        return Err(arguments_error());
    }
    for (flag, value) in &options.flags {
        let (key, value) = match flag.as_str() {
            "--limit" if candidates => (
                "limit",
                json!(value.parse::<i64>().map_err(|_| arguments_error())?),
            ),
            "--cursor" if candidates => ("cursor", json!(value)),
            "--snapshot" if candidates => ("snapshot", json!(value)),
            "--idempotency-key" if !candidates => ("idempotencyKey", json!(value)),
            _ => return Err(arguments_error()),
        };
        insert(&mut args, key, value)?;
    }
    if options.all && args.contains_key("cursor") {
        return Err(arguments_error());
    }
    if args
        .get("limit")
        .is_some_and(|v| v.as_i64().is_none_or(|n| !(1..=100).contains(&n)))
    {
        return Err(arguments_error());
    }
    let (tool, value) = match command {
        ReadCommand::ParentCandidates(id) => {
            insert(&mut args, "taskId", json!(id))?;
            (
                "cli_parent_candidates",
                typed::<crate::application::cli::CandidateArgs>(Value::Object(args))?,
            )
        }
        ReadCommand::Receipt => (
            "cli_get_receipt",
            typed::<crate::application::cli::ReceiptArgs>(Value::Object(args))?,
        ),
        _ => return Err(arguments_error()),
    };
    Ok(Request {
        tool,
        args: value,
        paginated: candidates,
        exact: false,
    })
}
