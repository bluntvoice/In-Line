//! CLI-only operations use the existing database transaction, scope, audit and receipt rules.
use super::mcp_write::*;
use super::*;
use crate::{
    application::cli::{CompleteArgs, ReceiptArgs},
    mcp::{contract::McpError, security::Authorization},
};
use serde_json::{json, Value as Json};
#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;

impl Database {
    pub(crate) fn cli_create_subtask(
        &self,
        client: &str,
        auth: &Authorization,
        args: crate::application::cli::ChildArgs,
    ) -> Result<Json, McpError> {
        if !auth.permissions.regular_read || !auth.permissions.write {
            return Err(err("forbidden"));
        }
        validate_intent(&args.idempotency_key, &args.intent, &args.reason)?;
        let request = json!({"tool":"cli_create_subtask","args":args});
        self.mcp_transaction(|conn| {
            mutate_with(
                conn,
                client,
                auth,
                &args.idempotency_key,
                &args.intent,
                &args.reason,
                &request,
                || {
                    let parent =
                        get_task_on(conn, args.parent_task_id).map_err(|_| err("not_found"))?;
                    if !allowed(&parent, auth) || parent.deleted_at.is_some() {
                        return Err(err("not_found"));
                    }
                    if version(conn, parent.id)? != args.parent_version {
                        return Err(err("conflict"));
                    }
                    let task = &args.task;
                    let departments = task
                        .departments
                        .clone()
                        .unwrap_or_else(|| parent.departments.clone());
                    let task_type = task
                        .task_type
                        .clone()
                        .unwrap_or_else(|| parent.task_type.clone());
                    if !auth.scope.allows(
                        &serde_json::to_string(&departments).map_err(|_| err("internal_error"))?,
                        &task_type,
                    ) {
                        return Err(err("forbidden"));
                    }
                    if !args.intent.allow_possible_duplicate
                        && load_all_tasks(conn).map_err(domain)?.iter().any(|t| {
                            allowed(t, auth)
                                && t.deleted_at.is_none()
                                && t.title.trim() == task.title.trim()
                        })
                    {
                        return Err(err("possible_duplicate"));
                    }
                    let child = create_subtask_on_domain(
                        conn,
                        CreateSubtaskInput {
                            parent_task_id: parent.id,
                            planned_date: task.planned_date.clone(),
                            title: task.title.clone(),
                            details: task.details.clone(),
                            departments: task.departments.clone(),
                            contacts: task.contacts.clone(),
                            task_type: task.task_type.clone(),
                            priority: task.priority.clone(),
                            workload: task.workload.clone(),
                            requested_deadline: task.requested_deadline.clone(),
                            requested_deadline_label: None,
                            internal_notes: task.internal_notes.clone(),
                            is_urgent: false,
                            urgent_requester: String::new(),
                            urgent_reason: String::new(),
                            enqueue_today: args.enqueue_today,
                        },
                    )
                    .map_err(domain)?;
                    Ok((
                        Some(child.id),
                        Json::Null,
                        proof(conn, child.id)?,
                        "createSubtask".into(),
                        false,
                    ))
                },
            )
        })
    }
    pub(crate) fn cli_candidate_source(
        &self,
        auth: &Authorization,
        id: i64,
    ) -> Result<(bool, i64), McpError> {
        let guard = self.connection.lock().map_err(|_| err("internal_error"))?;
        let conn = guard.as_ref().ok_or_else(|| err("host_unavailable"))?;

        let task = get_task_on(conn, id).map_err(|_| err("not_found"))?;
        if !allowed(&task, auth) || task.deleted_at.is_some() {
            return Err(err("not_found"));
        }
        Ok((
            child_ids_on(conn, id).map_err(domain)?.is_empty(),
            version(conn, id)?,
        ))
    }
    pub(crate) fn cli_receipt(
        &self,
        client: &str,
        auth: &Authorization,
        args: ReceiptArgs,
    ) -> Result<Json, McpError> {
        if args.idempotency_key.is_empty()
            || args.idempotency_key.len() > 128
            || args.idempotency_key.chars().any(char::is_control)
        {
            return Err(err("invalid_arguments"));
        }
        let guard = self.connection.lock().map_err(|_| err("internal_error"))?;
        let conn = guard.as_ref().ok_or_else(|| err("host_unavailable"))?;
        if let Some(kind) = args.kind.as_deref() {
            if kind != "artifact" {
                return Err(err("invalid_arguments"));
            }
            let (status,raw):(String,Option<String>)=conn.query_row("SELECT status,result_json FROM mcp_artifact_jobs WHERE client_id=? AND request_key=?",params![client,args.idempotency_key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(sql)?.ok_or_else(||err("not_found"))?;
            let value = raw
                .map(|s| serde_json::from_str::<Json>(&s).map_err(|_| err("internal_error")))
                .transpose()?;
            // Metadata only. Never open, create, repair or read back the artifact file.
            return Ok(
                json!({"idempotencyKey":args.idempotency_key,"readOnly":true,"kind":"artifact","ledgerStatus":status,"currentStateVerification":"not_reexecuted","receipt":value,"artifactContentsReturned":false}),
            );
        }
        let (task,audit_id,raw):(Option<i64>,Option<i64>,String)=conn.query_row("SELECT task_id,audit_id,result_json FROM mcp_receipts WHERE client_id=? AND request_key=?",params![client,args.idempotency_key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(sql)?.ok_or_else(||err("not_found"))?;
        if let Some(id) = task {
            if !allowed(&get_task_on(conn, id).map_err(|_| err("not_found"))?, auth) {
                return Err(err("forbidden"));
            }
        }
        if let Some(id) = audit_id {
            let (_, _, _, _, after) = audit_row(conn, id)?;
            related_scope(conn, &after, auth)?;
        }
        let value = redact_receipt(
            serde_json::from_str(&raw).map_err(|_| err("internal_error"))?,
            auth,
        );
        // The committed ledger confirms persistence, not unchanged current business state.
        Ok(
            json!({"idempotencyKey":args.idempotency_key,"readOnly":true,"ledgerStatus":"recorded","currentStateVerification":"not_reexecuted","receipt":value}),
        )
    }
    pub(crate) fn cli_complete(
        &self,
        client: &str,
        auth: &Authorization,
        args: CompleteArgs,
    ) -> Result<Json, McpError> {
        if !auth.permissions.regular_read || !auth.permissions.write {
            return Err(err("forbidden"));
        }
        validate_intent(&args.idempotency_key, &args.intent, &args.reason)?;
        if !args.intent.confirmed_real_work {
            return Err(err("explicit_intent_required"));
        }
        if args.task_id <= 0 || args.task_version < 0 || args.task_versions.len() > 99 {
            return Err(err("invalid_arguments"));
        }
        let request = json!({"tool":"cli_complete_task","args":args});
        self.mcp_transaction(|conn|{
            if let Some(value)=replay(conn,client,&args.idempotency_key,&request,auth)? {return Ok(value);}
            let task=get_task_on(conn,args.task_id).map_err(|_|err("not_found"))?;
            if !allowed(&task,auth)||task.deleted_at.is_some() {return Err(err("not_found"));}
            if version(conn,task.id)?!=args.task_version {return Err(err("conflict"));}
            if task.parent_task_id.is_some()&&args.include_eligible_subtasks {return Err(err("invalid_arguments"));}
            let children=child_ids_on(conn,task.id).map_err(domain)?;
            if children.len()!=args.task_versions.len()||children.len()>99 {return Err(err("conflict"));}
            let mut completion_proofs=vec![];
            for id in &children {
                let child=get_task_on(conn,*id).map_err(domain)?;
                if !allowed(&child,auth) {return Err(err("forbidden"));}
                if args.task_versions.get(&id.to_string()).copied()!=Some(version(conn,*id)?) {return Err(err("conflict"));}
                completion_proofs.push(proof(conn,*id)?);
            }
            let prior=load_all_tasks(conn).map_err(domain)?.into_iter().map(|t|(t.id,t)).collect::<HashMap<_,_>>();
            let mut before=proof(conn,task.id)?;
            before["completionProofs"]=json!(completion_proofs);
            let completed=crate::application::complete_in_transaction(conn,CompleteTaskInput {task_id:task.id,include_eligible_subtasks:args.include_eligible_subtasks}).map_err(domain)?;
            let mut after=proof(conn,task.id)?;
            let mut related_before=vec![];let mut related_after=vec![];
            let mut versions=serde_json::Map::new();versions.insert(task.id.to_string(),json!(version(conn,task.id)?));
            for current in load_all_tasks(conn).map_err(domain)? {
                if prior.get(&current.id).is_none_or(|old|serde_json::to_value(old).unwrap()!=serde_json::to_value(&current).unwrap()) {
                    if !allowed(&current,auth)||prior.get(&current.id).is_some_and(|old|!allowed(old,auth)) {return Err(err("forbidden"));}
                    versions.insert(current.id.to_string(),json!(version(conn,current.id)?));
                    if current.id!=task.id {related_before.push(json!(prior.get(&current.id)));related_after.push(proof(conn,current.id)?);}
                }
            }
            if !related_after.is_empty() { before["relatedTasks"]=json!(related_before);after["relatedTasks"]=json!(related_after); }
            let audit_id=audit(conn,client,"cliCompleteGroup",Some(task.id),&args.intent,&args.reason,&before,&after,&request,None)?;
            let result=json!({"commitStatus":"committed","verificationStatus":"verified_in_transaction","taskId":task.id,"taskVersions":versions,"completedTaskIds":completed.completed_task_ids,"auditId":audit_id,"replayed":false});
            receipt(conn,client,&args.idempotency_key,&request,Some(task.id),Some(audit_id),&result)?;
            Ok(result)
        })
    }
}
