use super::write_types::*;
use super::{
    contract::*,
    security::{Limiter, Security},
};
use super::{query, query_types::*};
use crate::database::Database;
use serde_json::{json, Value};
use std::time::Instant;

pub fn recheck_result(
    security: &Security,
    credentials: &Credentials,
    revision: u64,
    result: Result<Value, McpError>,
) -> Result<Value, McpError> {
    let value = result?;
    let store = security
        .store
        .lock()
        .map_err(|_| McpError::new("security_unavailable"))?;
    let auth = store.authorize(credentials)?;
    if auth.revision != revision {
        return Err(McpError::new("authorization_changed"));
    }
    Ok(value)
}

pub fn execute(
    security: &Security,
    db: &Database,
    credentials: &Credentials,
    tool: &str,
    args: Value,
) -> Result<Value, McpError> {
    // Hold authorization through result materialization: revocation and calls have one order.
    let store = security
        .store
        .lock()
        .map_err(|_| McpError::new("security_unavailable"))?;
    let authorization = store.authorize(credentials);
    security
        .limits
        .lock()
        .map_err(|_| McpError::new("security_unavailable"))?
        .check(
            Limiter::key(credentials, authorization.is_ok()),
            authorization.is_ok(),
            Instant::now(),
        )?;
    let auth = authorization?;
    if tool == "get_capabilities" {
        let _: CapabilityArgs =
            serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?;
        return Ok(
            json!({"clientId":credentials.client_id,"softwareVersion":env!("CARGO_PKG_VERSION"),"mcpVersion":API_VERSION,"schemaVersion":12,"commit":null,"transport":"stdio","permissions":auth.permissions,"scope":auth.scope,"authorizationRevision":auth.revision,"tools":TOOLS,"writeTools":["mutate_task","manage_preferences","request_undo"],"queryDefinitionWriteTools":["manage_saved_query"],"unsupported":["batch_write","queue_write","subtask_write","permanent_delete","restore_database","attachments","remote_transport"],"breakingChanges":["all_tools_require_credentials","enveloped_results","page_limit_100","work_note_requires_full_read","offset_requires_snapshot_after_first_page"],"pageLimit":100,"reportMaxDays":36501,"snapshotTtlSeconds":600,"snapshotDatabaseMaxBytes":33554432,"connectionVerified":true}),
        );
    }
    if !TOOLS.contains(&tool) {
        return Err(McpError::new("unsupported"));
    }
    if !auth.permissions.regular_read {
        return Err(McpError::new("forbidden"));
    }
    if matches!(tool, "mutate_task" | "request_undo") && !auth.permissions.write {
        return Err(McpError::new("forbidden"));
    }
    match tool {
        "mutate_task" => {
            return db.mcp_mutate(
                &credentials.client_id,
                &auth,
                serde_json::from_value::<MutateArgs>(args)
                    .map_err(|_| McpError::new("invalid_arguments"))?,
            )
        }
        "request_undo" => {
            return db.mcp_request_undo(
                &credentials.client_id,
                &auth,
                serde_json::from_value::<UndoArgs>(args)
                    .map_err(|_| McpError::new("invalid_arguments"))?,
            )
        }
        "manage_preferences" => {
            return db.mcp_preferences(
                &credentials.client_id,
                &auth,
                serde_json::from_value::<PreferenceArgs>(args)
                    .map_err(|_| McpError::new("invalid_arguments"))?,
            )
        }
        _ => {}
    }
    let mut queries = security
        .queries
        .lock()
        .map_err(|_| McpError::new("security_unavailable"))?;
    match tool {
        "get_report_summary" => {
            let args: DateRangeArgs =
                serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?;
            query::report(
                &mut queries,
                db,
                &credentials.client_id,
                &auth,
                ReportItemsArgs {
                    start_date: args.start_date,
                    end_date: args.end_date,
                    filters: args.filters,
                    snapshot: args.snapshot,
                    timezone_offset_minutes: args.timezone_offset_minutes,
                    limit: None,
                    offset: None,
                    cursor: None,
                },
                true,
            )
        }
        "list_report_items" => {
            let args: ReportItemsArgs =
                serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?;
            query::report(&mut queries, db, &credentials.client_id, &auth, args, false)
        }
        "query_tasks" => query::tasks(
            &mut queries,
            db,
            &credentials.client_id,
            &auth,
            serde_json::from_value::<QueryArgs>(args)
                .map_err(|_| McpError::new("invalid_arguments"))?,
        ),
        "query_task_history" => query::history(
            &mut queries,
            db,
            &credentials.client_id,
            &auth,
            serde_json::from_value::<HistoryArgs>(args)
                .map_err(|_| McpError::new("invalid_arguments"))?,
        ),
        "query_work_calendar" => query::calendar(
            &mut queries,
            db,
            &credentials.client_id,
            &auth,
            serde_json::from_value::<CalendarArgs>(args)
                .map_err(|_| McpError::new("invalid_arguments"))?,
        ),
        "manage_saved_query" => query::saved(
            &mut queries,
            &auth,
            &credentials.client_id,
            serde_json::from_value::<SavedQueryArgs>(args)
                .map_err(|_| McpError::new("invalid_arguments"))?,
        ),
        _ => Err(McpError::new("unsupported")),
    }
}
