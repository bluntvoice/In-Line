//! Shared application services inside the single database-owning host.
//! External authentication, scoped snapshots and result-release checks stay at
//! the existing gateway; none of these crate-private functions is an IPC route.
use crate::{database::Database, mcp::scope::Scope, models::StatisticsResult};
use rusqlite::Connection;
pub(crate) mod cli;

pub(crate) fn complete_task(
    db: &Database,
    input: crate::models::CompleteTaskInput,
) -> Result<crate::models::CompleteTaskResult, String> {
    db.with_transaction(|tx| complete_in_transaction(tx, input))
}
pub(crate) fn complete_in_transaction(
    connection: &Connection,
    input: crate::models::CompleteTaskInput,
) -> Result<crate::models::CompleteTaskResult, String> {
    if connection.is_autocommit() {
        return Err("事项完成必须在事务内执行".into());
    }
    crate::database::domain::complete_task_on_domain(connection, input)
}

/// Both local GUI queries and authorized, filtered MCP snapshots use the same
/// report calculation. Callers supply the range/scope already validated by
/// their adapter; the existing database validation and aggregation are retained.
pub(crate) fn statistics(
    db: &Database,
    start: String,
    end: String,
    timezone_offset_minutes: i32,
    scope: Scope,
) -> Result<StatisticsResult, String> {
    db.statistics_scoped(start, end, timezone_offset_minutes, scope)
}

/// The local GUI owns a transaction. The external write coordinator instead
/// calls the in-transaction operation below, preserving its atomic receipt,
/// audit, version checks and post-commit readback in the same transaction.
pub(crate) fn set_status(db: &Database, id: i64, status: String) -> Result<(), String> {
    db.with_transaction(|tx| set_status_in_transaction(tx, id, status))
}

pub(crate) fn set_status_in_transaction(
    connection: &Connection,
    id: i64,
    status: String,
) -> Result<(), String> {
    // A status change may allocate a new queue number before a later rule
    // rejects it. Never allow a new caller to perform those steps in autocommit.
    if connection.is_autocommit() {
        return Err("事项状态修改必须在事务内执行".into());
    }
    crate::database::domain::set_status_on(connection, id, status)
}
