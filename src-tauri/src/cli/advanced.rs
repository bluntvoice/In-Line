//! Fixed Phase5 adapters. Business validation, snapshots and commits stay on the host.
use super::{
    arguments_error,
    parse::{AdvancedCommand, Options},
    write::{nested, put, typed},
};
use crate::mcp::{
    contract::*,
    inspect_types::InspectArgs,
    job_types::{ChangesArgs, JobsArgs},
    query_types::SavedQueryArgs,
    write_types::*,
};
use serde_json::{json, Value};

pub(super) struct Request {
    pub tool: &'static str,
    pub args: Value,
    pub write_permission: bool,
    pub key: Option<String>,
    pub apply_batch: bool,
}

pub(super) fn build(c: &AdvancedCommand, o: &Options, input: Value) -> Result<Request, McpError> {
    use AdvancedCommand::*;
    if o.all || o.execute != c.requires_execute() {
        return Err(arguments_error());
    }
    let mut a = input.as_object().ok_or_else(arguments_error)?.clone();
    for (flag, v) in &o.flags {
        let (k, value) = match flag.as_str() {
            "--idempotency-key" => ("idempotencyKey", json!(v)),
            "--reason" => ("reason", json!(v)),
            "--intent-summary"
            | "--explicit-user-request"
            | "--confirmed-real-work"
            | "--allow-possible-duplicate"
                if matches!(c, Batch(_) | Data(_) | Preferences(true)) =>
            {
                let k = match flag.as_str() {
                    "--intent-summary" => "summary",
                    "--explicit-user-request" => "explicitUserRequest",
                    "--confirmed-real-work" => "confirmedRealWork",
                    _ => "allowPossibleDuplicate",
                };
                nested(
                    &mut a,
                    "intent",
                    k,
                    if k == "summary" {
                        json!(v)
                    } else {
                        json!(true)
                    },
                )?;
                continue;
            }
            "--name" => ("name", json!(v)),
            "--metric" => ("metric", json!(v)),
            "--dimension" => ("dimension", json!(v)),
            "--period-start" => ("periodStart", json!(v)),
            "--snapshot" => ("snapshot", json!(v)),
            "--cursor" => ("cursor", json!(v)),
            "--from" => ("startDate", json!(v)),
            "--to" => ("endDate", json!(v)),
            "--date" => ("date", json!(v)),
            "--job-id" => ("jobId", json!(v)),
            "--waiter-id" => ("waiterId", json!(v)),
            "--kind" => ("kind", json!(v)),
            "--since" => ("since", json!(v)),
            "--mode" => ("mode", json!(v)),
            "--export-format" => ("format", json!(v)),
            "--limit" | "--offset" | "--timezone-offset-minutes" => {
                let n = v.parse::<i64>().map_err(|_| arguments_error())?;
                if (flag == "--limit" && !(1..=100).contains(&n)) || (flag == "--offset" && n < 0) {
                    return Err(arguments_error());
                }
                (
                    match flag.as_str() {
                        "--limit" => "limit",
                        "--offset" => "offset",
                        _ => "timezoneOffsetMinutes",
                    },
                    json!(n),
                )
            }
            "--projection" => {
                let fields: Vec<_> = v.split(',').collect();
                if fields.iter().any(|s| s.is_empty() || s.trim() != *s) {
                    return Err(arguments_error());
                }
                ("projection", json!(fields))
            }
            "--explicit-intent" => ("explicitIntent", json!(true)),
            "--explicit-user-request" => ("explicitUserRequest", json!(true)),
            "--explicit-requery" => ("explicitRequery", json!(true)),
            "--allow-latest-after-invalidation" => ("allowLatestAfterInvalidation", json!(true)),
            _ => return Err(arguments_error()),
        };
        put(&mut a, k, value)?;
    }
    let tool = match c {
        Report(period) => {
            put(&mut a, "period", json!(period))?;
            put(&mut a, "text", json!(o.text))?;
            "cli_report"
        }
        Trace | Compare | Cache => {
            put(
                &mut a,
                "action",
                json!(match c {
                    Trace => "trace",
                    Compare => "compare",
                    _ => "cache",
                }),
            )?;
            "inspect_data"
        }
        Query(action) => {
            put(&mut a, "action", json!(action))?;
            "manage_saved_query"
        }
        Batch(apply) => {
            put(
                &mut a,
                "action",
                json!(if *apply { "apply" } else { "dryRun" }),
            )?;
            "batch_mutate"
        }
        Changes => "query_changes",
        Jobs(action) => {
            put(
                &mut a,
                "action",
                json!(match action.as_str() {
                    "cancel" => "cancelJob",
                    "cancel-waiter" => "cancelWaiter",
                    a => a,
                }),
            )?;
            "manage_jobs"
        }
        Data(action) => {
            put(&mut a, "action", json!(action))?;
            "manage_business_data"
        }
        Preferences(patch) => {
            put(
                &mut a,
                "action",
                json!(if *patch { "patch" } else { "get" }),
            )?;
            "manage_preferences"
        }
    };
    let a = Value::Object(a);
    let args = match tool {
        "cli_report" => typed::<crate::application::cli::ReportArgs>(a)?,
        "inspect_data" => typed::<InspectArgs>(a)?,
        "manage_saved_query" => typed::<SavedQueryArgs>(a)?,
        "batch_mutate" => typed::<BatchArgs>(a)?,
        "query_changes" => typed::<ChangesArgs>(a)?,
        "manage_jobs" => typed::<JobsArgs>(a)?,
        "manage_business_data" => typed::<BusinessDataArgs>(a)?,
        "manage_preferences" => typed::<PreferenceArgs>(a)?,
        _ => return Err(arguments_error()),
    };
    if matches!(c,Query(action) if action=="save" || action=="delete")
        && args["explicitIntent"] != true
    {
        return Err(McpError::new("explicit_intent_required"));
    }
    if matches!(c,Jobs(action) if action=="cancel") && args["explicitUserRequest"] != true {
        return Err(McpError::new("explicit_intent_required"));
    }
    if matches!(c,Jobs(action) if action=="start")
        && !matches!(
            args["kind"].as_str(),
            Some(
                "query_tasks"
                    | "get_report_summary"
                    | "list_report_items"
                    | "query_work_calendar"
                    | "export"
            )
        )
    {
        return Err(arguments_error());
    }
    let key = if matches!(c, Batch(_) | Data(_) | Preferences(true)) {
        let key = args["idempotencyKey"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 128 && !s.chars().any(char::is_control))
            .ok_or_else(arguments_error)?;
        if args["intent"]["explicitUserRequest"] != true
            || args["intent"]["summary"]
                .as_str()
                .is_none_or(|s| s.trim().is_empty())
            || args["reason"].as_str().is_none_or(|s| s.trim().is_empty())
        {
            return Err(McpError::new("explicit_intent_required"));
        }
        Some(key.to_owned())
    } else {
        None
    };
    Ok(Request {
        tool,
        args,
        write_permission: matches!(c, Batch(_) | Data(_) | Preferences(true))
            || matches!(c,Query(a) if a=="save"||a=="delete"),
        key,
        apply_batch: matches!(c, Batch(true)),
    })
}

#[cfg(windows)]
pub(super) async fn dispatch(o: &Options, r: Request, id: String) -> Result<Value, McpError> {
    let name = o
        .profile
        .as_deref()
        .ok_or_else(|| McpError::new("unauthenticated"))?;
    let root = crate::mcp::platform::root()?;
    let exe = std::env::current_exe().map_err(|_| McpError::new("host_unavailable"))?;
    let credentials = super::profile::load_at(&root, name, &exe)?;
    let call = |tool: &'static str, args: Value| {
        crate::mcp::ipc::call_at_with_id(root.clone(), credentials.clone(), tool, args, id.clone())
    };
    let cap = call("get_capabilities", json!({})).await?;
    super::validate_status(cap.clone(), &credentials.client_id)?;
    super::validate_private_route(&cap, r.tool)?;
    if cap["cli"]["version"] != 1 || cap["cli"]["phase"].as_u64().is_none_or(|p| p < 5) {
        return Err(McpError::new("incompatible"));
    }
    if cap["permissions"]["regularRead"] != true
        || (r.write_permission && cap["permissions"]["write"] != true)
    {
        return Err(McpError::new("forbidden"));
    }
    let mut args = r.args;
    if r.apply_batch {
        let mut preview = args.clone();
        preview["action"] = json!("dryRun");
        let value = call("batch_mutate", preview).await?;
        if value["commitStatus"] != "dry_run" {
            return Err(McpError::new("result_unknown"));
        }
        let basis =
            serde_json::from_value::<crate::mcp::basis::DataVersion>(value["basis"].clone())
                .map_err(|_| McpError::new("result_unknown"))?;
        let basis = serde_json::to_value(basis).map_err(|_| McpError::new("result_unknown"))?;
        if !args["expectedBasis"].is_null() && args["expectedBasis"] != basis {
            return Err(McpError::new("conflict"));
        }
        args["expectedBasis"] = basis;
    }
    let action = args["action"].as_str().map(str::to_owned);
    let mut value = call(r.tool, args).await?;
    if !value.is_object() {
        return Err(McpError::new("result_unknown"));
    }
    validate_response(r.tool, action.as_deref(), &value)?;
    // Dry-run is a successful preview, never an independently verified business commit.
    if r.tool == "batch_mutate" && !r.apply_batch && value["commitStatus"] != "dry_run" {
        return Err(McpError::new("result_unknown"));
    }
    if r.apply_batch && value["commitStatus"] == "needs_user_approval" {
        value["submissionOnly"] = json!(true);
    }
    Ok(value)
}

fn validate_response(tool: &str, action: Option<&str>, value: &Value) -> Result<(), McpError> {
    let known_job = |v: &Value| {
        v["jobId"].as_str().is_some_and(|s| !s.is_empty())
            && matches!(
                v["state"].as_str(),
                Some(
                    "queued"
                        | "running"
                        | "paused"
                        | "recovering"
                        | "cancelling"
                        | "publishing"
                        | "completed"
                        | "cancelled"
                        | "failed"
                )
            )
    };
    let valid = match (tool, action) {
        ("manage_saved_query", Some("save")) => value["saved"] == true,
        ("manage_saved_query", Some("delete")) => value["deleted"] == true,
        ("manage_saved_query", Some("get")) => value["definition"].is_object(),
        ("manage_saved_query", Some("list")) => value["names"]
            .as_array()
            .is_some_and(|a| a.iter().all(Value::is_string)),
        ("manage_jobs", Some("start")) => known_job(&value["job"]) && value["waiterId"].is_string(),
        ("manage_jobs", Some("get" | "cancelJob")) => known_job(value),
        ("manage_jobs", Some("list")) => value["items"]
            .as_array()
            .is_some_and(|a| a.iter().all(known_job)),
        ("manage_jobs", Some("cancelWaiter")) => {
            value["waiting"] == false && known_job(&value["job"])
        }
        ("manage_jobs", Some("result")) => {
            known_job(&value["job"])
                && value["job"]["state"] == "completed"
                && value["complete"] == true
                && value["items"].is_array()
                && value["hasMore"].is_boolean()
        }
        ("query_changes", _) => {
            value["items"].is_array()
                && value["nextCursor"].is_string()
                && matches!(value["phase"].as_str(), Some("fullSync" | "incremental"))
                && value["fullSyncComplete"].is_boolean()
        }
        ("manage_preferences", Some("get")) => {
            value["preferences"].is_object() && value["fieldVersions"].is_object()
        }
        ("cli_report", _) => {
            value["meta"]["complete"] == true
                && (value["statistics"].is_object()
                    || value["complete"] == true && value["text"].is_string())
        }
        _ => value.is_object(),
    };
    if valid {
        Ok(())
    } else {
        Err(McpError::new("result_unknown"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    fn request(words: &[&str], input: Value) -> Result<Request, McpError> {
        let o = super::super::parse::parse(words.iter().map(OsString::from).collect())
            .map_err(|_| arguments_error())?;
        let super::super::parse::Command::Advanced(c) = &o.command else {
            return Err(arguments_error());
        };
        build(c, &o, input)
    }
    #[test]
    fn advanced_routes_are_typed_and_never_approve_or_override_fixed_actions() {
        for words in [
            vec!["query", "save"],
            vec!["batch", "execute"],
            vec!["jobs", "start"],
            vec!["data", "export"],
            vec!["preferences", "patch"],
        ] {
            assert!(request(&words, json!({})).is_err(), "{words:?}");
        }
        for (words, body) in [
            (vec!["query", "list"], json!({"action":"save"})),
            (vec!["data", "cache"], json!({"action":"recompute"})),
            (
                vec!["report", "daily", "--format", "json", "--render", "text"],
                json!({}),
            ),
            (vec!["report", "weekly", "--render", "text"], json!({})),
            (
                vec!["jobs", "start", "--execute"],
                json!({"kind":"mutate_task","arguments":{}}),
            ),
            (
                vec!["query", "save", "--execute"],
                json!({"name":"x","explicitIntent":false}),
            ),
            (vec!["preferences", "get"], json!({"sql":"secret"})),
            (vec!["changes", "list", "--limit", "101"], json!({})),
            (vec!["changes", "list", "--all"], json!({})),
            (vec!["jobs", "get", "--job-id", "x"], json!({"jobId":"x"})),
        ] {
            assert!(request(&words, body).is_err(), "{words:?}");
        }
        for (words, tool) in [
            (vec!["stats", "trace"], "inspect_data"),
            (vec!["stats", "compare"], "inspect_data"),
            (vec!["changes", "list"], "query_changes"),
            (vec!["jobs", "list"], "manage_jobs"),
            (vec!["query", "list"], "manage_saved_query"),
            (vec!["preferences", "get"], "manage_preferences"),
            (
                vec!["report", "daily", "--date", "2026-10-22"],
                "cli_report",
            ),
        ] {
            assert_eq!(request(&words, json!({})).unwrap().tool, tool);
        }
    }
}
