//! Fixed write grammar; never fetch versions or manufacture intent, reason or keys.
use super::{
    arguments_error,
    parse::{Options, WriteCommand},
};
use crate::mcp::{contract::*, write_types::*};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Map, Value};

pub(super) struct Request {
    pub tool: &'static str,
    pub args: Value,
    pub key: String,
    pub create_title: Option<String>,
}
fn put(map: &mut Map<String, Value>, key: &str, value: Value) -> Result<(), McpError> {
    if map.contains_key(key) {
        return Err(arguments_error());
    }
    map.insert(key.into(), value);
    Ok(())
}
fn nested(
    map: &mut Map<String, Value>,
    object: &str,
    key: &str,
    value: Value,
) -> Result<(), McpError> {
    let m = map
        .entry(object)
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(arguments_error)?;
    put(m, key, value)
}
fn typed<T: DeserializeOwned + Serialize>(value: Value) -> Result<Value, McpError> {
    let result = serde_json::to_value(
        serde_json::from_value::<T>(value.clone()).map_err(|_| arguments_error())?,
    )
    .map_err(|_| arguments_error())?;
    // MutateArgs flattens its action; explicitly reject outer fields ignored by serde.
    if value
        .as_object()
        .is_none_or(|m| m.keys().any(|k| result.get(k).is_none()))
    {
        return Err(arguments_error());
    }
    Ok(result)
}
pub(super) fn build(
    command: &WriteCommand,
    options: &Options,
    input: Value,
) -> Result<Request, McpError> {
    use WriteCommand::*;
    if !options.execute {
        return Err(arguments_error());
    }
    let mut args = input.as_object().ok_or_else(arguments_error)?.clone();
    for (flag, value) in &options.flags {
        let value = json!(value);
        let key = match flag.as_str() {
            "--idempotency-key" => "idempotencyKey",
            "--reason" => "reason",
            "--task-version" => "taskVersion",
            "--parent-version" => "parentVersion",
            "--event-id" => "eventId",
            "--audit-id" => "auditId",
            "--date" if matches!(command, Reschedule(_)) => "plannedDate",
            "--requester" if matches!(command, Urgent(_, true)) => "requester",
            "--urgent-reason" if matches!(command, Urgent(_, true)) => "urgentReason",
            "--result-status" if matches!(command, Record(_) | Correct(_)) => "resultStatus",
            "--handled-at" if matches!(command, Record(_) | Correct(_)) => "handledAt",
            "--note" if matches!(command, Record(_) | Correct(_)) => "note",
            "--include-subtasks" if matches!(command, Trash(_)) => "includeSubtasks",
            "--include-completed-subtasks" if matches!(command, Archive(_)) => {
                "includeCompletedSubtasks"
            }
            "--inherit-deadline" if matches!(command, Enter(_) | Reopen(_)) => "inheritDeadline",
            "--confirm-historical-impact" if matches!(command, Correct(_) | Void(_)) => {
                "confirmHistoricalImpact"
            }
            "--sync-status" if matches!(command, Record(_)) => "syncStatus",
            "--include-eligible-subtasks" | "--parent-only" if matches!(command, Complete(_)) => {
                "includeEligibleSubtasks"
            }
            "--enqueue-today" if matches!(command, ChildCreate(_)) => "enqueueToday",
            "--intent-summary"
            | "--explicit-user-request"
            | "--confirmed-real-work"
            | "--allow-possible-duplicate"
            | "--replace-whole-text" => {
                let (key, v) = match flag.as_str() {
                    "--intent-summary" => ("summary", value),
                    "--explicit-user-request" => ("explicitUserRequest", json!(true)),
                    "--confirmed-real-work" => ("confirmedRealWork", json!(true)),
                    "--allow-possible-duplicate" => ("allowPossibleDuplicate", json!(true)),
                    _ => ("replaceWholeText", json!(true)),
                };
                nested(&mut args, "intent", key, v)?;
                continue;
            }
            "--title"
            | "--departments"
            | "--contacts"
            | "--task-type"
            | "--planned-date"
            | "--priority"
            | "--workload"
            | "--details"
            | "--internal-notes"
            | "--requested-deadline"
                if matches!(command, Create | ChildCreate(_)) =>
            {
                let (key, v) = match flag.as_str() {
                    "--departments" | "--contacts" => {
                        let parts: Vec<_> = value.as_str().unwrap().split(',').collect();
                        if parts.iter().any(|s| s.is_empty() || s.trim() != *s) {
                            return Err(arguments_error());
                        }
                        (
                            if flag == "--departments" {
                                "departments"
                            } else {
                                "contacts"
                            },
                            json!(parts),
                        )
                    }
                    "--task-type" => ("taskType", value),
                    "--planned-date" => ("plannedDate", value),
                    "--internal-notes" => ("internalNotes", value),
                    "--requested-deadline" => ("requestedDeadline", value),
                    _ => (flag.trim_start_matches("--"), value),
                };
                nested(&mut args, "task", key, v)?;
                continue;
            }
            _ => return Err(arguments_error()),
        };
        let v = if matches!(key, "taskVersion" | "parentVersion" | "eventId" | "auditId") {
            let n = value
                .as_str()
                .unwrap()
                .parse::<i64>()
                .map_err(|_| arguments_error())?;
            if n < 0 {
                return Err(arguments_error());
            }
            json!(n)
        } else if matches!(
            key,
            "includeSubtasks"
                | "includeCompletedSubtasks"
                | "inheritDeadline"
                | "confirmHistoricalImpact"
                | "syncStatus"
                | "includeEligibleSubtasks"
        ) {
            json!(flag != "--parent-only")
        } else if key == "enqueueToday" {
            json!(match value.as_str().unwrap() {
                "true" => true,
                "false" => false,
                _ => return Err(arguments_error()),
            })
        } else {
            value
        };
        put(&mut args, key, v)?;
    }
    let mut tool = "mutate_task";
    let (action, target) = match command {
        Create => ("create", None),
        Update(id) => ("patch", Some(*id)),
        Status(id, status) => {
            put(&mut args, "status", json!(status))?;
            ("setStatus", Some(*id))
        }
        Complete(id) if args.contains_key("includeEligibleSubtasks") => {
            tool = "cli_complete_task";
            put(&mut args, "taskId", json!(id))?;
            ("", None)
        }
        Complete(id) => {
            put(&mut args, "status", json!("completed"))?;
            ("setStatus", Some(*id))
        }
        Urgent(id, on) => {
            put(&mut args, "isUrgent", json!(on))?;
            if !on {
                put(&mut args, "requester", json!(""))?;
                put(&mut args, "urgentReason", json!(""))?;
            }
            ("setUrgent", Some(*id))
        }
        Archive(id) => ("archive", Some(*id)),
        Trash(id) => ("trash", Some(*id)),
        Restore(id) => ("restore", Some(*id)),
        ChildCreate(id) => {
            tool = "cli_create_subtask";
            put(&mut args, "parentTaskId", json!(id))?;
            ("", None)
        }
        ParentSet(id, parent) => {
            put(
                &mut args,
                "parent",
                parent.map_or(Value::Null, |id| json!({"taskId":id})),
            )?;
            ("setParent", Some(*id))
        }
        Reorder(id) => {
            put(&mut args, "parent", json!({"taskId":id}))?;
            ("reorderSubtasks", None)
        }
        Enter(id) | Reopen(id) | Process(id) | CompleteRound(id) | Move(id, _) => {
            let mode = match command {
                Enter(_) => "enqueue",
                Reopen(_) => "reopen",
                Process(_) => "processRound",
                CompleteRound(_) => "completeRound",
                Move(_, v) if v == "up" => "moveUp",
                _ => "moveDown",
            };
            put(&mut args, "mode", json!(mode))?;
            ("queue", Some(*id))
        }
        Reschedule(id) => ("reschedule", Some(*id)),
        Record(id) => ("recordWorkEvent", Some(*id)),
        Correct(id) => ("correctWorkEvent", Some(*id)),
        Void(id) => ("voidWorkEvent", Some(*id)),
        Undo => {
            tool = "request_undo";
            ("", None)
        }
    };
    if let Some(id) = target {
        put(&mut args, "target", json!({"taskId":id}))?;
    }
    if !action.is_empty() {
        put(&mut args, "action", json!(action))?;
    }
    let args = match tool {
        "cli_complete_task" => typed::<crate::application::cli::CompleteArgs>(Value::Object(args))?,
        "cli_create_subtask" => typed::<crate::application::cli::ChildArgs>(Value::Object(args))?,
        "request_undo" => typed::<UndoArgs>(Value::Object(args))?,
        _ => typed::<MutateArgs>(Value::Object(args))?,
    };
    let key = args["idempotencyKey"]
        .as_str()
        .filter(|k| !k.is_empty() && k.len() <= 128 && !k.chars().any(char::is_control))
        .ok_or_else(arguments_error)?
        .to_string();
    if args["intent"]["explicitUserRequest"] != true {
        return Err(McpError::new("explicit_intent_required"));
    }
    if args["intent"]["summary"]
        .as_str()
        .is_none_or(|s| s.trim().is_empty() || s.len() > 1000)
        || args["reason"]
            .as_str()
            .is_none_or(|s| s.trim().is_empty() || s.len() > 1000)
    {
        return Err(arguments_error());
    }
    let create_title = matches!(command, Create | ChildCreate(_)).then(|| {
        args["task"]["title"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_owned()
    });
    Ok(Request {
        tool,
        args,
        key,
        create_title,
    })
}

/// Only known committed and independently verified writes can exit successfully.
pub(super) fn validate_result(value: Value) -> Result<Value, McpError> {
    // Keep known commit/verification facts in the envelope even when exit 9 is
    // required. The renderer classifies the result after attaching its key.
    if !value.is_object() {
        return Err(McpError::new("result_unknown"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_implicit_execution_identity_intent_key_version_or_extra_action() {
        let args = ["task", "status", "set", "1", "completed", "--execute"];
        let options =
            super::super::parse::parse(args.into_iter().map(std::ffi::OsString::from).collect())
                .unwrap();
        let super::super::parse::Command::Write(c) = &options.command else {
            panic!()
        };
        let base = json!({"taskVersion":1,"idempotencyKey":"original","reason":"user request","intent":{"summary":"complete task","explicitUserRequest":true,"confirmedRealWork":true}});
        let request = build(c, &options, base.clone()).unwrap();
        assert_eq!(request.args["action"], "setStatus");
        for field in ["taskVersion", "idempotencyKey", "reason", "intent"] {
            let mut bad = base.clone();
            bad.as_object_mut().unwrap().remove(field);
            assert!(build(c, &options, bad).is_err());
        }
        let mut bad = base.clone();
        bad["force"] = json!(true);
        assert!(build(c, &options, bad).is_err());
        let mut bad = base;
        bad["action"] = json!("trash");
        assert!(build(c, &options, bad).is_err());
        assert_eq!(
            validate_result(
                json!({"commitStatus":"committed","verificationStatus":"verification_failed"})
            )
            .unwrap()["verificationStatus"],
            "verification_failed"
        );
    }
}
