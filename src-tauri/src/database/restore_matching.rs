//! Resolve prior conflict copies using only evidence already stored by merge restore.
use super::*;

pub(super) struct RestorePlan {
    pub(super) ids: HashMap<i64, i64>,
    pub(super) verified_conflicts: HashSet<i64>,
}

pub(super) fn plan(
    source: &Connection,
    target: &Connection,
    tasks: &[LegalTask],
) -> Result<RestorePlan, String> {
    let by_id: HashMap<_, _> = tasks.iter().map(|task| (task.id, task)).collect();
    let mut mapped = HashMap::new();
    let mut occupied = HashSet::new();

    // Ordinary exact-title merge retains its existing history and relation behavior.
    // Reserve its destinations before looking for previously renamed copies.
    let mut ordinary = Vec::new();
    for task in tasks.iter().filter(|task| !task.is_import_conflict) {
        let mut matches = Vec::new();
        for candidate in tasks_with_title(target, &task.title)? {
            if !candidate.is_import_conflict
                && same_task_content(&candidate, task)
                && (!timeline::has_reviewed_conflict(target, candidate.id)?
                    || timeline::same_restore_history(source, target, task.id, candidate.id, None)?)
            {
                matches.push(candidate.id);
            }
        }
        ordinary.push((task.id, matches));
    }
    claim_unique(ordinary, &mut mapped, &mut occupied);
    let ordinary_targets = occupied.clone();

    for roots in [true, false] {
        let mut candidates = Vec::new();
        for task in tasks {
            if mapped.contains_key(&task.id) || task.parent_task_id.is_none() != roots {
                continue;
            }
            let parent = if let Some(parent) = task.parent_task_id {
                let Some(target_id) = mapped
                    .get(&parent)
                    .copied()
                    .filter(|_| valid_source_parent(task, &by_id))
                else {
                    continue;
                };
                let current = get_task_on(target, target_id)?;
                if current.parent_task_id.is_some() || current.deleted_at.is_some() {
                    continue;
                }
                Some(target_id)
            } else {
                None
            };
            let matches = matching_conflicts(source, target, task, parent)?;
            candidates.push((task.id, matches));
        }
        claim_unique(candidates, &mut mapped, &mut occupied);
    }
    Ok(RestorePlan {
        ids: mapped,
        verified_conflicts: occupied.difference(&ordinary_targets).copied().collect(),
    })
}

fn claim_unique(
    candidates: Vec<(i64, Vec<i64>)>,
    mapped: &mut HashMap<i64, i64>,
    occupied: &mut HashSet<i64>,
) {
    let mut claims = HashMap::<i64, usize>::new();
    for id in candidates.iter().flat_map(|(_, matches)| matches) {
        *claims.entry(*id).or_default() += 1;
    }
    for (task_id, matches) in candidates {
        if let [id] = matches[..] {
            if claims[&id] == 1 && occupied.insert(id) {
                mapped.insert(task_id, id);
            }
        }
    }
}

pub(super) fn valid_source_parent(task: &LegalTask, by_id: &HashMap<i64, &LegalTask>) -> bool {
    task.deleted_at.is_none()
        && task.parent_task_id.is_some_and(|id| {
            id != task.id
                && by_id.get(&id).is_some_and(|parent| {
                    parent.parent_task_id.is_none() && parent.deleted_at.is_none()
                })
        })
}

fn matching_conflicts(
    source: &Connection,
    target: &Connection,
    task: &LegalTask,
    parent: Option<i64>,
) -> Result<Vec<i64>, String> {
    let prefix = format!("{}（冲突", task.title);
    let mut query = target
        .prepare(&format!(
            "{SELECT_TASK} WHERE is_import_conflict=1 AND created_at=?1 AND parent_task_id IS ?3
           AND (substr(title,1,length(?2))=?2 OR (?4 AND title=?5))"
        ))
        .map_err(display_error)?;
    let rows = query
        .query_map(
            params![
                task.created_at,
                prefix,
                parent,
                task.is_import_conflict,
                task.title
            ],
            Database::row_task,
        )
        .map_err(display_error)?;
    let mut matches = Vec::new();
    for row in rows {
        let candidate = row.map_err(display_error)?;
        let suffix = candidate.title.strip_prefix(&prefix).unwrap_or_default();
        let title_matches = generated_title_suffix(suffix)
            || (task.is_import_conflict && candidate.title == task.title);
        if !title_matches || !same_task_content(&candidate, task) {
            continue;
        }
        let Some(remap_note) = number_history(source, target, task, &candidate)? else {
            continue;
        };
        if timeline::same_restore_history(
            source,
            target,
            task.id,
            candidate.id,
            remap_note.as_deref(),
        )? && copy_linked_work_events(source, target, task.id, candidate.id, true)?
            && same_row_history(
                source,
                target,
                "status_history",
                STATUS_HISTORY_COLUMNS,
                task.id,
                candidate.id,
            )?
            && same_row_history(
                source,
                target,
                "urgent_records",
                URGENT_HISTORY_COLUMNS,
                task.id,
                candidate.id,
            )?
        {
            matches.push(candidate.id);
        }
    }
    Ok(matches)
}

fn generated_title_suffix(suffix: &str) -> bool {
    suffix == "）"
        || suffix
            .strip_prefix(' ')
            .and_then(|suffix| suffix.strip_suffix('）'))
            .and_then(|digits| {
                let number = digits.parse::<i32>().ok()?;
                (number >= 2 && number.to_string() == digits).then_some(())
            })
            .is_some()
}

fn same_row_history(
    source: &Connection,
    target: &Connection,
    table: &str,
    columns: &[&str],
    source_id: i64,
    target_id: i64,
) -> Result<bool, String> {
    let source = task_history_rows(source, table, columns, source_id)?;
    let mut target = task_history_rows(target, table, columns, target_id)?;
    for values in source {
        let Some(index) = target.iter().position(|row| row == &values) else {
            return Ok(false);
        };
        target.swap_remove(index);
    }
    Ok(target.is_empty())
}

fn number_history(
    source: &Connection,
    target: &Connection,
    task: &LegalTask,
    candidate: &LegalTask,
) -> Result<Option<Option<String>>, String> {
    // The candidate's current reservation and all historical records must
    // belong to that candidate, including archived evidence of an occupied number.
    if task.ticket_date != candidate.ticket_date {
        return Ok(None);
    }
    let source_allocations = scheduling::owned_allocations(source, task.id)?;
    let mut target_allocations = scheduling::owned_allocations(target, candidate.id)?;
    let Some(original) = source_allocations.iter().find(|row| row.is_current(task)) else {
        return Ok(None);
    };
    let Some(index) = target_allocations
        .iter()
        .position(|row| row.is_current(candidate))
    else {
        return Ok(None);
    };
    let current = target_allocations.swap_remove(index);
    let queues = queue_entries(source, task.id)?;
    let mut target_queues = queue_entries(target, candidate.id)?;
    let current_queue = queues.iter().find(|entry| {
        entry.queue_date == task.ticket_date && entry.daily_sequence == task.daily_sequence
    });
    let copied_activation = current_queue
        .is_some_and(|entry| current.activated_at.as_deref() == Some(entry.enqueued_at.as_str()));
    if (current.activated_at != original.activated_at && !copied_activation)
        || current.voided_at != original.voided_at
        || current.void_reason != original.void_reason
        || queues.len() != target_queues.len()
    {
        return Ok(None);
    }

    let renumbered = task.daily_sequence != candidate.daily_sequence;
    if !renumbered && task.permanent_number != candidate.permanent_number {
        return Ok(None);
    }
    let mut remapped_sequence = candidate.daily_sequence;
    if renumbered {
        let permanent_for =
            |sequence| format!("{}-{sequence:02}", task.ticket_date.replace('-', ""));
        if candidate.permanent_number != permanent_for(candidate.daily_sequence) {
            // Older restore allocated a permanent number first, then allocated
            // a second queue number when the active source pair was occupied.
            let legacy = target_allocations
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    current_queue.is_some_and(|entry| {
                        entry.closed_at.is_none() && current.allocated_at == entry.created_at
                    }) && row.queue_date == candidate.ticket_date
                        && row.permanent_number == candidate.permanent_number
                        && row.permanent_number == permanent_for(row.daily_sequence)
                        && row.activated_at.is_none()
                        && row.voided_at.is_some()
                        && row.void_reason == "导入时调整当前队列号码"
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            let [index] = legacy[..] else { return Ok(None) };
            remapped_sequence = target_allocations.swap_remove(index).daily_sequence;
        }
    }
    for original_entry in queues {
        let mut matches = target_queues.iter().enumerate().filter(|(_, entry)| {
            if !entry.same_history(&original_entry) {
                return false;
            }
            if original_entry.queue_date == original.queue_date
                && original_entry.daily_sequence == original.daily_sequence
            {
                return entry.daily_sequence == current.daily_sequence;
            }
            if original_entry.closed_at.is_none() {
                return false;
            }
            target_allocations.iter().any(|allocation| {
                allocation.queue_date == entry.queue_date
                    && allocation.daily_sequence == entry.daily_sequence
                    && (entry.daily_sequence == original_entry.daily_sequence
                        || allocation.is_imported_queue(entry, candidate))
            })
        });
        let Some((index, _)) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some() {
            return Ok(None);
        }
        let entry = target_queues.swap_remove(index);
        // A copied historical queue may have its own synthetic allocation in
        // addition to the archived original allocation.
        if let Some(index) = target_allocations.iter().position(|row| {
            row.daily_sequence == entry.daily_sequence && row.is_imported_queue(&entry, candidate)
        }) {
            target_allocations.swap_remove(index);
        }
    }

    for allocation in &source_allocations {
        if !renumbered && allocation.is_current(task) {
            continue;
        }
        let mut matches = target_allocations.iter().enumerate().filter(|(_, row)| {
            row == &allocation
                || row.is_archived_copy(allocation)
                || (renumbered
                    && allocation.is_current(task)
                    && row.is_retired_original(allocation))
        });
        let Some((index, _)) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some() {
            return Ok(None);
        }
        target_allocations.swap_remove(index);
    }
    if !target_allocations.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        renumbered.then(|| remapped_number_note(task, remapped_sequence)),
    ))
}
