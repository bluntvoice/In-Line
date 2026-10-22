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
        let mut capabilities = json!({"clientId":credentials.client_id,"softwareVersion":env!("CARGO_PKG_VERSION"),"mcpVersion":API_VERSION,"schemaVersion":crate::database::DATABASE_SCHEMA_VERSION,"commit":super::build_identity::current()["exactCommit"],"build":super::build_identity::current(),"diagnostics":{"ownErrorIdOnly":true,"rawLogsAvailableToAi":false},"transport":"stdio","permissions":auth.permissions,"scope":auth.scope,"authorizationRevision":auth.revision,"tools":TOOLS,"writeTools":["mutate_task","manage_preferences","request_undo","batch_mutate","manage_business_data"],"queryDefinitionWriteTools":["manage_saved_query"],"unsupported":["permanent_delete","restore_database","attachments","remote_transport"],"breakingChanges":["all_tools_require_credentials","enveloped_results","page_limit_100","work_note_requires_full_read","offset_requires_snapshot_after_first_page"],"batchLimit":db.mcp_batch_limit()?,"batchHardLimit":100,"pageLimit":100,"reportMaxDays":36501,"snapshotTtlSeconds":600,"snapshotDatabaseMaxBytes":33554432,"backgroundJobs":{"globalActiveLimit":2,"perClientActiveLimit":1,"resultMaxBytes":16777216,"checkpointDiskMaxBytes":67108864,"resultRetentionDays":7,"summaryRetentionDays":30,"maxPauseSeconds":1800,"maxRecoverySeconds":86400,"maxFullRequeries":1,"nativeScheduler":false},"changes":{"modes":["events","current"],"initialFullSyncRequired":true,"incrementalCursorTtlSeconds":86400,"metadataDiskMaxBytes":16777216},"inspection":{"actions":["health","trace","compare","cache","recompute"],"businessRepairAllowed":false},"reportCache":{"tier":"memory","ttlSeconds":60,"capacityBytes":8388608,"entryMaxBytes":2097152,"maxEntries":16,"perClientMaxEntries":4,"diskCacheEnabled":false,"explicitRequerySupported":true},"connectionVerified":true});
        if store
            .clients
            .iter()
            .any(|c| c.id == credentials.client_id && c.kind.as_deref() == Some("cli"))
        {
            capabilities["cli"] = crate::application::cli::capabilities();
        }
        return Ok(capabilities);
    }
    let cli_route = crate::application::cli::ROUTES.contains(&tool);
    if cli_route
        && !store
            .clients
            .iter()
            .any(|c| c.id == credentials.client_id && c.kind.as_deref() == Some("cli"))
    {
        return Err(McpError::new("forbidden"));
    }
    if !TOOLS.contains(&tool) && !cli_route {
        return Err(McpError::new("unsupported"));
    }
    if !auth.permissions.regular_read {
        return Err(McpError::new("forbidden"));
    }
    if tool == "get_diagnostic" {
        let args: super::diagnostic_types::DiagnosticArgs =
            serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?;
        return security
            .diagnostics
            .own(&credentials.client_id, &args.error_id);
    }
    if tool != "inspect_data" && db.mcp_quality_health()?["healthy"] != true {
        return Err(McpError::new("resync_required"));
    }
    if matches!(
        tool,
        "mutate_task" | "request_undo" | "batch_mutate" | "manage_business_data"
    ) && !auth.permissions.write
    {
        return Err(McpError::new("forbidden"));
    }
    if cli_route {
        if crate::application::cli::write_route(tool) && !auth.permissions.write {
            return Err(McpError::new("forbidden"));
        }
        if tool == "cli_report" {
            let mut queries = security
                .queries
                .lock()
                .map_err(|_| McpError::new("security_unavailable"))?;
            return query::cli_report(
                &mut queries,
                db,
                &credentials.client_id,
                &auth,
                serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?,
            );
        }
        if tool == "cli_parent_candidates" {
            let mut queries = security
                .queries
                .lock()
                .map_err(|_| McpError::new("security_unavailable"))?;
            return query::cli_parent_candidates(
                &mut queries,
                db,
                &credentials.client_id,
                &auth,
                serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?,
            );
        }
        return crate::application::cli::execute(db, &credentials.client_id, &auth, tool, args);
    }
    match tool {
        "manage_jobs" => {
            return security.jobs.execute(
                db,
                &credentials.client_id,
                &auth,
                serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?,
            )
        }
        "batch_mutate" => {
            return db.mcp_batch(
                &credentials.client_id,
                &auth,
                serde_json::from_value::<BatchArgs>(args)
                    .map_err(|_| McpError::new("invalid_arguments"))?,
            )
        }
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
        "inspect_data" => super::inspect::execute(
            security,
            &mut queries,
            db,
            &credentials.client_id,
            &auth,
            serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?,
        ),
        "query_changes" => security
            .changes
            .lock()
            .map_err(|_| McpError::new("security_unavailable"))?
            .execute(
                &mut queries,
                db,
                &credentials.client_id,
                &auth,
                serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?,
            ),
        "manage_business_data" => super::business_data::execute(
            &mut queries,
            db,
            &credentials.client_id,
            &auth,
            serde_json::from_value::<BusinessDataArgs>(args)
                .map_err(|_| McpError::new("invalid_arguments"))?,
        ),
        "get_report_summary" => {
            let mut args: DateRangeArgs =
                serde_json::from_value(args).map_err(|_| McpError::new("invalid_arguments"))?;
            args.timezone_offset_minutes = Some(
                args.timezone_offset_minutes
                    .unwrap_or_else(|| chrono::Local::now().offset().local_minus_utc() / 60),
            );
            let live = db.mcp_basis()?;
            let cache_key = super::cache::Cache::key(
                &credentials.client_id,
                &serde_json::to_value(&args).map_err(|_| McpError::new("invalid_arguments"))?,
                &auth,
                &live,
            );
            if !args.explicit_requery && args.snapshot.is_none() {
                if let Some(value) = security
                    .cache
                    .lock()
                    .map_err(|_| McpError::new("security_unavailable"))?
                    .get(&cache_key, &credentials.client_id, &auth, &live)
                {
                    if value["meta"]["snapshotId"]
                        .as_str()
                        .and_then(|id| {
                            query::snapshot_health(&queries, db, &credentials.client_id, &auth, id)
                                .ok()
                        })
                        .is_some_and(|v| v["valid"] == true)
                        && db.mcp_basis()? == live
                    {
                        return Ok(value);
                    }
                }
            }
            let explicit_snapshot = args.snapshot.is_some();
            let mut value = query::report(
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
            )?;
            value["meta"]["complete"] = json!(true);
            value["meta"]["coverage"] = value["coverage"].clone();
            let computed: super::basis::DataVersion =
                serde_json::from_value(value["meta"]["dataVersion"].clone())
                    .map_err(|_| McpError::new("internal_error"))?;
            if !explicit_snapshot && computed == db.mcp_basis()? {
                security
                    .cache
                    .lock()
                    .map_err(|_| McpError::new("security_unavailable"))?
                    .put(cache_key, &credentials.client_id, &auth, computed, value)
            } else {
                value["meta"]["cache"] = json!({"reused":false,"stored":false,"computedAt":chrono::Utc::now().timestamp(),"validationStatus":"explicit_or_changed_snapshot","sourceSnapshotId":value["meta"]["snapshotId"]});
                Ok(value)
            }
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
