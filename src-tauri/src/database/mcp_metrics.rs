//! Shared authoritative report source relations; extraction preserves GUI SQL.
use super::*;
use crate::mcp::contract::McpError;
use crate::mcp::scope::Scope;
use serde_json::{json, Value};
use std::collections::BTreeMap;
impl Database {
    /// Sources come from the exact GUI relations on the caller's frozen, filtered image.
    pub(crate) fn mcp_metric_sources(
        &self,
        start: String,
        end: String,
        scope: Scope,
        eligible: bool,
    ) -> Result<Vec<Value>, McpError> {
        self.with_conn(|c|{
            scope.register(c)?;
            let mut stmt=c.prepare(&format!("{RANGED_CTE} SELECT task_id,id,result_status FROM ranked WHERE position=1 ORDER BY task_id")).map_err(display_error)?;
            let winners=stmt.query_map(params![start,end],|r|Ok((r.get::<_,i64>(0)?,(r.get::<_,i64>(1)?,r.get::<_,String>(2)?)))).map_err(display_error)?.collect::<Result<BTreeMap<_,_>,_>>().map_err(display_error)?;
            let ids:Vec<i64>=if eligible{
                let prefix=ELIGIBLE_COUNT_SQL.strip_suffix("SELECT count(*) FROM eligible").ok_or("统计来源关系无效")?;
                let mut s=c.prepare(&format!("{prefix} SELECT task_id FROM eligible ORDER BY task_id")).map_err(display_error)?;
                let rows=s.query_map(params![start,end],|r|r.get(0)).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;rows
            }else{winners.keys().copied().collect()};
            if ids.len()>100_000{return Err("统计来源超过资源上限".into());}
            let mut rows=Vec::with_capacity(ids.len());
            let mut task=c.prepare("SELECT permanent_number,title,parent_task_id IS NOT NULL,task_type,department,requested_deadline FROM tasks WHERE id=?").map_err(display_error)?;
            let mut events=c.prepare("SELECT id,handled_at,result_status FROM task_work_events WHERE task_id=?1 AND voided_at IS NULL AND strftime('%s',handled_at)>=strftime('%s',?2) AND strftime('%s',handled_at)<strftime('%s',?3) ORDER BY strftime('%s',handled_at),id").map_err(display_error)?;
            let mut queues=c.prepare("SELECT id,enqueued_at,closed_at FROM task_queue_entries WHERE task_id=?1 AND strftime('%s',enqueued_at)<strftime('%s',?3) AND (closed_at IS NULL OR strftime('%s',closed_at)>strftime('%s',?2)) ORDER BY id").map_err(display_error)?;
            for id in ids{
                let (number,title,child,kind,department,deadline):(String,String,bool,String,String,Option<String>)=task.query_row([id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(display_error)?;
                let ev=events.query_map(params![id,start,end],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"handledAt":r.get::<_,String>(1)?,"resultStatus":r.get::<_,String>(2)?}))).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;
                let q=queues.query_map(params![id,start,end],|r|Ok(json!({"id":r.get::<_,i64>(0)?,"enqueuedAt":r.get::<_,String>(1)?,"closedAt":r.get::<_,Option<String>>(2)?}))).map_err(display_error)?.collect::<Result<Vec<_>,_>>().map_err(display_error)?;
                let winner=winners.get(&id);
                rows.push(json!({"taskId":id,"permanentNumber":number,"title":title,"isSubtask":child,"taskType":kind,"departments":parse_contacts(&department),"requestedDeadline":deadline,"winningEventId":winner.map(|w|w.0),"winningResult":winner.map(|w|&w.1),"validEvents":ev,"queueIntervals":q,"eligible":eligible,"numeratorWeight":0,"denominatorWeight":0}));
            }
            // Only the snapshot image is inspected; no live source or unrelated row leaks.
            Ok(rows)
        }).map_err(Into::into)
    }
}
pub(super) const RANGED_CTE: &str = r#"WITH ranged AS (
                SELECT event.id,event.task_id,event.result_status,event.handled_at,
                       tasks.task_type AS current_task_type,tasks.department AS current_department,
                       tasks.parent_task_id AS current_parent_task_id
                FROM task_work_events event
                JOIN tasks ON tasks.id=event.task_id
                WHERE event.voided_at IS NULL AND tasks.deleted_at IS NULL AND mcp_scope(tasks.department,tasks.task_type)
                  AND strftime('%s',event.handled_at)>=strftime('%s',?1)
                  AND strftime('%s',event.handled_at)<strftime('%s',?2)
              ), ranked AS (
                SELECT *,ROW_NUMBER() OVER(
                  PARTITION BY task_id ORDER BY strftime('%s',handled_at) DESC,id DESC
                ) AS position
                FROM ranged
              )"#;
pub(super) const ELIGIBLE_COUNT_SQL: &str = r#"WITH eligible AS (
                           SELECT entry.task_id
                           FROM task_queue_entries entry
                           JOIN tasks ON tasks.id=entry.task_id
                           WHERE tasks.deleted_at IS NULL AND mcp_scope(tasks.department,tasks.task_type)
                             AND strftime('%s',entry.enqueued_at)<strftime('%s',?2)
                             AND (entry.closed_at IS NULL OR strftime('%s',entry.closed_at)>strftime('%s',?1))
                             AND (tasks.requested_deadline IS NULL OR (
                               strftime('%s',tasks.requested_deadline)>=strftime('%s',?1)
                               AND strftime('%s',tasks.requested_deadline)<strftime('%s',?2)
                             ))
                           UNION
                           SELECT event.task_id
                           FROM task_work_events event
                           JOIN tasks ON tasks.id=event.task_id
                           WHERE event.voided_at IS NULL AND tasks.deleted_at IS NULL AND mcp_scope(tasks.department,tasks.task_type)
                             AND strftime('%s',event.handled_at)>=strftime('%s',?1)
                             AND strftime('%s',event.handled_at)<strftime('%s',?2)
                         ) SELECT count(*) FROM eligible"#;
