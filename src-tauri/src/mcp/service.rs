use super::{
    contract::*,
    security::{Limiter, Security},
};
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
            json!({"clientId":credentials.client_id,"softwareVersion":env!("CARGO_PKG_VERSION"),"mcpVersion":API_VERSION,"schemaVersion":10,"commit":null,"transport":"stdio","permissions":auth.permissions,"scope":auth.scope,"authorizationRevision":auth.revision,"tools":TOOLS,"writeTools":[],"unsupported":["business_write","permanent_delete","restore_database","attachments","remote_transport","stable_snapshot_pagination","full_history"],"breakingChanges":["all_tools_require_credentials","enveloped_results","page_limit_100","work_note_requires_full_read"],"pageLimit":100,"reportMaxDays":371,"connectionVerified":true}),
        );
    }
    if !TOOLS.contains(&tool) {
        return Err(McpError::new("unsupported"));
    }
    if !auth.permissions.regular_read {
        return Err(McpError::new("forbidden"));
    }
    match tool {
        "get_report_summary" => {
            let args: DateRangeArgs =
                serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?;
            let (start, end, offset) = report_range(&args.start_date, &args.end_date)?;
            let statistics = db.statistics_scoped(start, end, offset, auth.scope)?;
            Ok(
                json!({"startDate":args.start_date,"endDate":args.end_date,"timezoneOffsetMinutes":offset,"statistics":statistics}),
            )
        }
        "list_report_items" => {
            let args: ReportItemsArgs =
                serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?;
            let limit = args.limit.unwrap_or(100);
            let offset = args.offset.unwrap_or(0);
            if !(1..=100).contains(&limit) || offset < 0 {
                return Err(McpError::new("invalid_arguments"));
            }
            let (start, end, tz) = report_range(&args.start_date, &args.end_date)?;
            let mut page = db.report_items_scoped(start, end, limit, offset, auth.scope)?;
            if !auth.permissions.full_read {
                for item in &mut page.items {
                    for event in &mut item.work_events {
                        event.note.clear();
                    }
                }
            }
            Ok(
                json!({"startDate":args.start_date,"endDate":args.end_date,"timezoneOffsetMinutes":tz,"page":page,"redactedFields":if auth.permissions.full_read{vec![]}else{vec!["workEvents.note"]}}),
            )
        }
        _ => Err(McpError::new("unsupported")),
    }
}
