use super::{
    contract::McpError,
    inspect_types::InspectArgs,
    query::{self, QueryState},
    security::{Authorization, Security},
};
use crate::database::Database;
use serde_json::{json, Value};
pub(crate) fn execute(
    s: &Security,
    q: &mut QueryState,
    db: &Database,
    client: &str,
    auth: &Authorization,
    args: InspectArgs,
) -> Result<Value, McpError> {
    let invalid = || McpError::new("invalid_arguments");
    if args.action != "trace"
        && (args.metric.is_some() || args.dimension.is_some() || args.period_start.is_some())
    {
        return Err(invalid());
    }
    if args.action != "health" && args.snapshot.is_some()
        || !["health", "trace"].contains(&args.action.as_str()) && args.cursor.is_some()
    {
        return Err(invalid());
    }
    if args.action != "compare" && args.right.is_some() {
        return Err(invalid());
    }
    match args.action.as_str() {
        "health" => {
            if args.report.is_some() {
                return Err(invalid());
            }
            let snapshot = args
                .snapshot
                .as_deref()
                .map(|id| query::snapshot_health(q, db, client, auth, id))
                .transpose()?;
            let cursor = args
                .cursor
                .as_deref()
                .map(|id| {
                    s.changes
                        .lock()
                        .map_err(|_| McpError::new("security_unavailable"))?
                        .health(db, client, auth, id)
                })
                .transpose()?;
            Ok(
                json!({"health":db.mcp_quality_health()?,"snapshot":snapshot,"cursor":cursor,"cache":s.cache.lock().map_err(|_|McpError::new("security_unavailable"))?.info(Some(client)),"readOnly":true,"businessRepairPerformed":false}),
            )
        }
        "cache" => {
            if args.report.is_some() {
                return Err(invalid());
            }
            Ok(s.cache
                .lock()
                .map_err(|_| McpError::new("security_unavailable"))?
                .info(Some(client)))
        }
        "trace" => query::trace(q, db, client, auth, args),
        "recompute" => {
            let a = args.report.ok_or_else(invalid)?;
            if a.snapshot.is_some() || a.cursor.is_some() || a.offset.is_some() {
                return Err(invalid());
            }
            if db.mcp_quality_health()?["healthy"] != true {
                return Err(McpError::new("resync_required"));
            }
            let mut value = query::report(q, db, client, auth, a, true)?;
            value["meta"]["cache"] = json!({"reused":false,"source":"explicit_recomputation","stored":false,"computedAt":chrono::Utc::now().timestamp(),"validatedAt":chrono::Utc::now().timestamp(),"validationStatus":"fresh_authorized_snapshot"});
            Ok(value)
        }
        "compare" => {
            let left = args.report.ok_or_else(invalid)?;
            let right = args.right.ok_or_else(invalid)?;
            if left.cursor.is_some()
                || right.cursor.is_some()
                || left.offset.is_some()
                || right.offset.is_some()
            {
                return Err(invalid());
            }
            let a = query::report(q, db, client, auth, left.clone(), true)?;
            let b = query::report(q, db, client, auth, right.clone(), true)?;
            let differences = [
                (
                    "range",
                    json!([a["startDate"], a["endDate"]]),
                    json!([b["startDate"], b["endDate"]]),
                ),
                (
                    "timezone",
                    a["timezoneOffsetMinutes"].clone(),
                    b["timezoneOffsetMinutes"].clone(),
                ),
                (
                    "scopeAndFilters",
                    a["coverage"].clone(),
                    b["coverage"].clone(),
                ),
                (
                    "definition",
                    a["definition"].clone(),
                    b["definition"].clone(),
                ),
                (
                    "rateMode",
                    a["statistics"]["summary"]["rateMode"].clone(),
                    b["statistics"]["summary"]["rateMode"].clone(),
                ),
                (
                    "dataVersion",
                    a["meta"]["dataVersion"].clone(),
                    b["meta"]["dataVersion"].clone(),
                ),
                (
                    "statistics",
                    a["statistics"].clone(),
                    b["statistics"].clone(),
                ),
            ]
            .into_iter()
            .filter(|(_, a, b)| a != b)
            .map(|(field, a, b)| json!({"field":field,"left":a,"right":b}))
            .collect::<Vec<_>>();
            query::bounded(
                json!({"left":a,"right":b,"differences":differences,"sourceInstructions":"Use inspect_data action trace with the respective report snapshotId and metric; use list_report_items for complete paginated handling details","readOnly":true,"originalRecordsChanged":false}),
            )
        }
        _ => Err(invalid()),
    }
}
