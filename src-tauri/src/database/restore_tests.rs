use super::*;
use sha2::{Digest, Sha256};
use tests::{sample, TestClock};

struct Fixture {
    root: PathBuf,
    source: Database,
    target: Database,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "inline-f3-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(root.join("source")).unwrap();
        fs::create_dir_all(root.join("target")).unwrap();
        Self {
            source: Database::open_at(root.join("source/inline.db")).unwrap(),
            target: Database::open_at(root.join("target/inline.db")).unwrap(),
            root,
        }
    }

    fn finish(self) {
        let Self {
            root,
            source,
            target,
        } = self;
        drop(source);
        drop(target);
        fs::remove_dir_all(root).unwrap();
    }
}

fn digest(path: impl AsRef<Path>) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

fn integrity(connection: &Connection, version: i64) {
    assert_eq!(Database::schema_version(connection).unwrap(), version);
    let check: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(check, "ok");
    assert!(!connection
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
}

fn business(connection: &Connection) -> Vec<Vec<Vec<Value>>> {
    [
        "tasks",
        "status_history",
        "task_queue_entries",
        "task_work_events",
        "urgent_records",
        "queue_number_allocations",
        "task_logs",
        "daily_sequences",
    ]
    .into_iter()
    .map(|table| {
        let mut query = connection
            .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
            .unwrap();
        let columns = query.column_count();
        query
            .query_map([], |row| (0..columns).map(|index| row.get(index)).collect())
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    })
    .collect()
}

fn generation(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT data_generation FROM mcp_data_basis", [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn current_business(db: &Database) -> Vec<Vec<Vec<Value>>> {
    db.with_conn(|connection| Ok(business(connection))).unwrap()
}

fn counts(result: BackupMergeResult) -> (usize, usize, usize) {
    assert_eq!(result.conflict_tasks as usize, result.conflicts.len());
    (
        result.added_tasks as usize,
        result.merged_tasks as usize,
        result.conflict_tasks as usize,
    )
}

fn staged_files_cleared(db: &Database) {
    assert!(fs::read_dir(&db.backup_dir).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".merge-source-")
    }));
}

#[test]
fn restoring_the_same_conflict_does_not_expand_the_next_backup() {
    let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
    let fixture = Fixture::new();
    let Fixture {
        source,
        target,
        root,
    } = &fixture;
    let title = "重复恢复的冲突事项";
    let mut input = sample(title);
    input.details = "合成详细说明".repeat(1_400);
    input.internal_notes = "独立办理记录".repeat(1_400);
    let original = source.save_task(input).unwrap();
    source.add_log(original.id, "合成源备注".into()).unwrap();
    let backup = source.create_backup("manual").unwrap();
    target.save_task(sample(title)).unwrap();
    target
        .save_task(sample(&format!("{title}（冲突）")))
        .unwrap();
    let source_hash = digest(&backup.path);
    let initial_generation = target.with_conn(|c| Ok(generation(c))).unwrap();
    let mut baseline = None;
    for round in 1..=3 {
        let imported = target.import_backup(backup.path.clone()).unwrap();
        assert_eq!(digest(&imported.path), source_hash);
        let previous_backups: HashSet<_> = target
            .list_backups()
            .unwrap()
            .into_iter()
            .map(|item| item.path)
            .collect();
        let before = current_business(target);
        let result = target.restore_backup(imported.path.clone()).unwrap();
        if round == 1 {
            assert_eq!(
                result.conflicts[0].imported_title,
                format!("{title}（冲突 2）")
            );
        }
        let result = counts(result);
        let snap = target.create_backup("manual").unwrap();
        let saved =
            Connection::open_with_flags(&snap.path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        integrity(&saved, 15);
        let rows = business(&saved);
        let pages = ["page_count", "page_size", "freelist_count"].map(|field| {
            saved
                .query_row(&format!("PRAGMA {field}"), [], |row| row.get::<_, u64>(0))
                .unwrap()
        });
        let backups = target.list_backups().unwrap();
        let emergency = backups
            .iter()
            .find(|item| {
                item.name.contains("before-restore") && !previous_backups.contains(&item.path)
            })
            .unwrap();
        let emergency_db =
            Connection::open_with_flags(&emergency.path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        integrity(&emergency_db, 15);
        assert_eq!(business(&emergency_db), before);
        assert_eq!(generation(&emergency_db), initial_generation + round - 1);
        let statistics = target
            .statistics(
                "2026-10-01T00:00:00+08:00".into(),
                "2026-10-03T00:00:00+08:00".into(),
                480,
            )
            .unwrap();
        let statistics = serde_json::to_value(statistics).unwrap();
        let live_sizes = ["inline.db", "inline.db-wal", "inline.db-shm"].map(|name| {
            fs::metadata(root.join("target").join(name))
                .map(|info| info.len())
                .unwrap_or(0)
        });
        println!("F3-01 round={round} result={result:?} business_counts={:?} size={} pages={pages:?} live_sizes={live_sizes:?} generation={} backups={} directory_bytes={} source_sha256={source_hash} emergency_sha256={} snapshot_sha256={} statistics={statistics}",
            rows.iter().map(Vec::len).collect::<Vec<_>>(), snap.size,
            generation(&saved),
            backups.len(), backups.iter().map(|item| item.size).sum::<u64>(),
            digest(&emergency.path), digest(&snap.path));
        assert_eq!(result, if round == 1 { (1, 0, 1) } else { (0, 1, 0) });
        assert_eq!(rows[0].len(), 3);
        assert_eq!(generation(&saved), initial_generation + round);
        assert_eq!(digest(&backup.path), source_hash);
        assert_eq!(digest(&imported.path), source_hash);
        staged_files_cleared(target);
        let observed = (rows, snap.size, pages, statistics);
        if let Some(baseline) = &baseline {
            assert_eq!(baseline, &observed);
        } else {
            baseline = Some(observed);
        }
    }
    fixture.finish();
}

#[test]
fn ambiguous_or_modified_conflict_copies_are_not_reused() {
    let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
    for (label, sql) in [
        ("reviewed", "UPDATE tasks SET is_import_conflict=0 WHERE id=?1"),
        ("renamed", "UPDATE tasks SET title='用户改名' WHERE id=?1"),
        ("manual suffix", "UPDATE tasks SET title='同题（冲突 01）' WHERE id=?1"),
        ("details", "UPDATE tasks SET details='用户修改' WHERE id=?1"),
        ("planned", "UPDATE tasks SET planned_date='2026-11-01' WHERE id=?1"),
        ("schedule action", "UPDATE tasks SET schedule_action='late',schedule_action_at=created_at WHERE id=?1"),
        ("status", "UPDATE tasks SET status='waiting_confirmation' WHERE id=?1"),
        ("notes", "UPDATE tasks SET internal_notes='本地备注' WHERE id=?1"),
        ("color", "UPDATE tasks SET ticket_color='#224466' WHERE id=?1"),
        ("owner", "UPDATE queue_number_allocations SET task_id=NULL WHERE task_id=?1"),
        ("void", "UPDATE queue_number_allocations SET voided_at=allocated_at WHERE task_id=?1 AND voided_at IS NULL"),
        ("missing historical number", "DELETE FROM queue_number_allocations WHERE task_id=?1 AND void_reason LIKE '备份原号%'"),
        ("extra remap", "INSERT INTO task_logs(task_id,log_type,content,created_at,operation_id)
                         SELECT task_id,log_type,content,created_at,operation_id FROM task_logs
                          WHERE task_id=?1 AND log_type='backup_number_remapped'"),
        ("extra user history", "INSERT INTO task_logs(task_id,log_type,content,created_at)
                               VALUES(?1,'note','本地办理备注','2026-10-02T10:00:00+08:00')"),
    ] {
        let fixture = Fixture::new();
        let mut input = sample("同题");
        input.details = "源详情".into();
        fixture.source.save_task(input).unwrap();
        fixture.target.save_task(sample("同题")).unwrap();
        let b = fixture.source.create_backup("manual").unwrap();
        let imported = fixture.target.import_backup(b.path).unwrap();
        let path = imported.path;
        let first = fixture.target.restore_backup(path.clone()).unwrap();
        let id = first.conflicts[0].task_id;
        fixture.target.with_conn(|c| {
            assert!(c.execute(sql, [id]).map_err(display_error)? > 0, "{label}");
            Ok(())
        }).unwrap();
        let before = fixture.target.get_task(id).unwrap();
        let second = fixture.target.restore_backup(path).unwrap();
        assert_eq!(counts(second), (1, 0, 1), "{label}");
        assert_eq!(serde_json::to_value(fixture.target.get_task(id).unwrap()).unwrap(),
            serde_json::to_value(before).unwrap(), "{label}");
        println!("F3-02 incompatible {label}: independent conflict");
        fixture.finish();
    }

    let f = Fixture::new();
    let mut input = sample("双候选");
    input.details = "源详情".into();
    f.source.save_task(input).unwrap();
    f.target.save_task(sample("双候选")).unwrap();
    let b = f.source.create_backup("manual").unwrap();
    let path = f.target.import_backup(b.path).unwrap().path;
    let first = f.target.restore_backup(path.clone()).unwrap().conflicts[0].task_id;
    f.target
        .with_conn(|c| {
            c.execute("UPDATE tasks SET is_import_conflict=0 WHERE id=?", [first])
                .map_err(display_error)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(
        counts(f.target.restore_backup(path.clone()).unwrap()),
        (1, 0, 1)
    );
    f.target
        .with_conn(|c| {
            c.execute("UPDATE tasks SET is_import_conflict=1 WHERE id=?", [first])
                .map_err(display_error)?;
            Ok(())
        })
        .unwrap();
    assert_eq!(counts(f.target.restore_backup(path).unwrap()), (1, 0, 1));
    f.finish();
}

#[test]
fn remapped_parent_and_children_keep_relationship_numbers_and_history() {
    let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
    let f = Fixture::new();
    let mut child = sample("待办理子");
    child.details = "来源子事项".into();
    let child = f.source.save_task(child).unwrap();
    let mut parent = sample("预约父");
    parent.planned_date = Some("2026-10-05".into());
    parent.details = "来源父事项".into();
    let parent = f.source.save_task(parent).unwrap();
    assert!(parent.id > child.id);
    f.source.set_parent_task(child.id, Some(parent.id)).unwrap();
    let mut local_parent = sample("预约父");
    local_parent.planned_date = Some("2026-10-05".into());
    let local_parent = f.target.save_task(local_parent).unwrap();
    let local_child = f.target.save_task(sample("待办理子")).unwrap();
    f.target
        .set_parent_task(local_child.id, Some(local_parent.id))
        .unwrap();
    let other = f.target.save_task(sample("独立本地父")).unwrap();
    let b = f.source.create_backup("manual").unwrap();
    let path = f.target.import_backup(b.path).unwrap().path;
    let first = f.target.restore_backup(path.clone()).unwrap();
    let imported_child = first
        .conflicts
        .iter()
        .find(|row| row.source_title == "待办理子")
        .unwrap()
        .task_id;
    let imported_parent = first
        .conflicts
        .iter()
        .find(|row| row.source_title == "预约父")
        .unwrap()
        .task_id;
    assert_eq!(counts(first), (2, 0, 2));
    let c = f.target.get_task(imported_child).unwrap();
    let p = f.target.get_task(imported_parent).unwrap();
    assert_eq!(c.parent_task_id, Some(imported_parent));
    assert_eq!(c.subtask_sort_order, 1);
    assert!(c.has_active_queue && p.is_scheduled && !p.has_active_queue);
    assert_ne!(c.daily_sequence, child.daily_sequence);
    assert_ne!(p.daily_sequence, parent.daily_sequence);
    assert_eq!(
        f.target.get_task(local_child.id).unwrap().daily_sequence,
        local_child.daily_sequence
    );
    assert_eq!(
        f.target.get_task(local_parent.id).unwrap().daily_sequence,
        local_parent.daily_sequence
    );
    let before = current_business(&f.target);
    for _ in 0..2 {
        assert_eq!(
            counts(f.target.restore_backup(path.clone()).unwrap()),
            (0, 2, 0)
        );
        assert_eq!(current_business(&f.target), before);
    }
    println!(
        "F3-03/04 parent {} -> {} child {} -> {}, #{} / #{}",
        parent.id,
        imported_parent,
        child.id,
        imported_child,
        p.permanent_number,
        c.permanent_number
    );
    f.target
        .set_parent_task(imported_child, Some(other.id))
        .unwrap();
    assert_eq!(counts(f.target.restore_backup(path).unwrap()), (1, 1, 1));
    assert_eq!(
        f.target.get_task(imported_child).unwrap().parent_task_id,
        Some(other.id)
    );
    let _later = TestClock::at("2026-10-05T11:00:00+08:00");
    assert_eq!(f.target.activate_due_scheduled().unwrap(), 2);
    let activated = f.target.get_task(p.id).unwrap();
    assert_eq!(activated.daily_sequence, p.daily_sequence);
    assert!(activated.has_active_queue && !activated.is_scheduled);
    f.target
        .with_conn(|c| {
            integrity(c, 15);
            let current = scheduling::owned_allocations(c, p.id)?;
            assert_eq!(
                current.iter().filter(|row| row.voided_at.is_none()).count(),
                1
            );
            assert!(current
                .iter()
                .any(|row| row.is_current(&activated) && row.activated_at.is_some()));
            let ledger_is_covered: bool = c
                .query_row(
                    "SELECT NOT EXISTS(SELECT 1 FROM queue_number_allocations a
                 LEFT JOIN daily_sequences s ON s.ticket_date=a.queue_date
                 WHERE s.last_sequence IS NULL OR s.last_sequence<a.daily_sequence)",
                    [],
                    |row| row.get(0),
                )
                .map_err(display_error)?;
            assert!(ledger_is_covered);
            Ok(())
        })
        .unwrap();
    f.finish();
}

#[test]
fn invalid_parent_links_and_unproven_parent_candidates_stay_independent() {
    let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
    for label in ["missing", "self", "cycle", "ambiguous"] {
        let f = Fixture::new();
        let mut child = sample("关系子事项");
        child.details = "源子".into();
        let child = f.source.save_task(child).unwrap();
        let mut parent = sample("关系父事项");
        parent.details = "源父".into();
        let parent = f.source.save_task(parent).unwrap();
        f.source.set_parent_task(child.id, Some(parent.id)).unwrap();
        f.target.save_task(sample("关系子事项")).unwrap();
        f.target.save_task(sample("关系父事项")).unwrap();
        f.source
            .with_conn(|c| {
                if label == "missing" {
                    c.pragma_update(None, "foreign_keys", false)
                        .map_err(display_error)?;
                    c.execute("UPDATE tasks SET parent_task_id=999 WHERE id=?", [child.id])
                        .map_err(display_error)?;
                    c.pragma_update(None, "foreign_keys", true)
                        .map_err(display_error)?;
                } else if label == "self" {
                    c.execute("UPDATE tasks SET parent_task_id=id WHERE id=?", [child.id])
                        .map_err(display_error)?;
                } else if label == "cycle" {
                    c.execute(
                        "UPDATE tasks SET parent_task_id=? WHERE id=?",
                        params![child.id, parent.id],
                    )
                    .map_err(display_error)?;
                }
                Ok(())
            })
            .unwrap();
        let backup = f.source.create_backup("manual").unwrap();
        let path = f.target.import_backup(backup.path).unwrap().path;
        let first = f.target.restore_backup(path.clone()).unwrap();
        let copied_parent = first
            .conflicts
            .iter()
            .find(|row| row.source_title == "关系父事项")
            .unwrap()
            .task_id;
        let copied_child = first
            .conflicts
            .iter()
            .find(|row| row.source_title == "关系子事项")
            .unwrap()
            .task_id;
        assert_eq!(counts(first), (2, 0, 2), "{label}");
        if label == "ambiguous" {
            // Manufacture two independently restored copies with complete evidence.
            f.target
                .with_conn(|c| {
                    c.execute(
                        "UPDATE tasks SET is_import_conflict=0 WHERE id=?",
                        [copied_parent],
                    )
                    .map_err(display_error)?;
                    Ok(())
                })
                .unwrap();
            assert_eq!(
                counts(f.target.restore_backup(path.clone()).unwrap()),
                (2, 0, 2)
            );
            f.target
                .with_conn(|c| {
                    c.execute(
                        "UPDATE tasks SET is_import_conflict=1 WHERE id=?",
                        [copied_parent],
                    )
                    .map_err(display_error)?;
                    Ok(())
                })
                .unwrap();
        }
        let result = f.target.restore_backup(path).unwrap();
        let new_child = result
            .conflicts
            .iter()
            .find(|row| row.source_title == "关系子事项")
            .unwrap()
            .task_id;
        assert_ne!(new_child, copied_child);
        assert_eq!(
            counts(result),
            if matches!(label, "missing" | "self") {
                (1, 1, 1)
            } else {
                (2, 0, 2)
            },
            "{label}"
        );
        let child_parent = f.target.get_task(new_child).unwrap().parent_task_id;
        if label == "ambiguous" {
            assert!(child_parent.is_some_and(|id| id != copied_parent));
            assert_eq!(
                f.target.get_task(copied_child).unwrap().parent_task_id,
                Some(copied_parent)
            );
        } else {
            assert_eq!(child_parent, None, "{label}");
            assert_eq!(
                f.target.get_task(copied_child).unwrap().parent_task_id,
                None,
                "{label}"
            );
        }
        f.target
            .with_conn(|c| {
                integrity(c, 15);
                Ok(())
            })
            .unwrap();
        println!("F3-03 {label}: no uncertain parent reuse");
        f.finish();
    }
}

#[test]
fn two_source_tasks_do_not_claim_the_same_exact_title_target() {
    let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
    let f = Fixture::new();
    for _ in 0..2 {
        f.source.save_task(sample("同题不同事项")).unwrap();
    }
    let existing = f.target.save_task(sample("同题不同事项")).unwrap();
    let original = current_business(&f.target);
    let backup = f.source.create_backup("manual").unwrap();
    let path = f.target.import_backup(backup.path).unwrap().path;
    assert_eq!(
        counts(f.target.restore_backup(path.clone()).unwrap()),
        (2, 0, 2)
    );
    let restored = current_business(&f.target);
    for (before, after) in original.iter().take(7).zip(&restored) {
        assert_eq!(&after[..before.len()], before);
    }
    assert_eq!(
        f.target.get_task(existing.id).unwrap().permanent_number,
        existing.permanent_number
    );
    f.finish();
}

#[test]
fn existing_conflict_with_separate_permanent_and_active_queue_numbers_is_reused() {
    let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
    let f = Fixture::new();
    let mut input = sample("旧恢复副本");
    input.details = "源内容".into();
    f.source.save_task(input).unwrap();
    f.target.save_task(sample("旧恢复副本")).unwrap();
    let b = f.source.create_backup("manual").unwrap();
    let path = f.target.import_backup(b.path).unwrap().path;
    let first = f.target.restore_backup(path.clone()).unwrap();
    let id = first.conflicts[0].task_id;
    f.target.with_conn(|c| {
        let task = get_task_on(c, id)?;
        // Recreate the prior restore's second allocation for the active queue.
        let queue_sequence = next_import_sequence(c, &task.ticket_date)?;
        c.execute(
            "INSERT INTO queue_number_allocations(task_id,permanent_number,queue_date,daily_sequence,allocated_at,activated_at)
             SELECT q.task_id,t.permanent_number,q.queue_date,?2,q.created_at,q.enqueued_at
             FROM task_queue_entries q JOIN tasks t ON t.id=q.task_id WHERE q.task_id=?1",
            params![id, queue_sequence],
        ).map_err(display_error)?;
        c.execute(
            "UPDATE queue_number_allocations SET activated_at=NULL,voided_at=?3,void_reason='导入时调整当前队列号码'
             WHERE task_id=?1 AND daily_sequence=?2",
            params![id, task.daily_sequence, now()],
        ).map_err(display_error)?;
        c.execute("UPDATE task_queue_entries SET daily_sequence=?2 WHERE task_id=?1", params![id, queue_sequence])
            .map_err(display_error)?;
        c.execute("UPDATE tasks SET daily_sequence=?2 WHERE id=?1", params![id, queue_sequence])
            .map_err(display_error)?;
        Ok(())
    }).unwrap();
    let before = current_business(&f.target);
    assert_eq!(counts(f.target.restore_backup(path).unwrap()), (0, 1, 0));
    assert_eq!(current_business(&f.target), before);
    f.finish();
}

#[test]
fn conflict_restore_preserves_closed_queues_colors_and_linked_histories() {
    let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
    let f = Fixture::new();
    let mut input = sample("办理历史");
    input.details = "备份专有字段".into();
    input.internal_notes = "源内部备注".into();
    let task = f.source.save_task(input).unwrap();
    f.source
        .set_task_ticket_color(task.id, Some("#1a2b3c".into()))
        .unwrap();
    f.source.process_round(task.id).unwrap();
    f.source
        .enqueue_task(QueueInput {
            id: task.id,
            inherit_deadline: false,
            reason: "合成历史轮次".into(),
        })
        .unwrap();
    let work = f.source.list_work_events(task.id).unwrap()[0].id;
    let op = "timeline:v2:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1";
    let other_op = "timeline:v2:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:1";
    let legacy = "timeline:v1:cccccccccccccccccccccccccccccccc:1:queue";
    f.source.with_conn(|c| {
        c.execute(
            "UPDATE task_work_events SET voided_at=created_at,note='旧办理' WHERE id=?", [work]
        ).map_err(display_error)?;
        c.execute(
            "INSERT INTO task_work_events(task_id,result_status,handled_at,task_type_snapshot,source,note,created_at,updated_at,correction_of)
             SELECT task_id,'waiting_materials',handled_at,task_type_snapshot,source,'纠错办理',created_at,updated_at,id
             FROM task_work_events WHERE id=?", [work]
        ).map_err(display_error)?;
        for operation in [Some(op), Some(op), Some(other_op), None] {
            c.execute("INSERT INTO task_logs(task_id,log_type,content,created_at,operation_id) VALUES(?,?,?,?,?)",
                params![task.id, "queue", "同秒办理各记录", now(), operation]).map_err(display_error)?;
        }
        c.execute("INSERT INTO task_logs(task_id,log_type,content,created_at) VALUES(?,?,?,?)",
            params![task.id, legacy, "旧版关联", now()]).map_err(display_error)?;
        c.execute("INSERT INTO urgent_records(task_id,requester,reason,requested_at,confirmation_status)
                   VALUES(?,'合成请求人','合成加急',?,'confirmed')", params![task.id, now()]).map_err(display_error)?;
        Ok(())
    }).unwrap();
    f.source.add_log(task.id, "源人工备注".into()).unwrap();
    let local = f.target.save_task(sample("办理历史")).unwrap();
    f.target
        .record_work_event(WorkEventInput {
            task_id: local.id,
            result_status: "processed".into(),
            handled_at: now(),
            note: "无关本地事件".into(),
            sync_status: false,
        })
        .unwrap();
    f.target
        .with_conn(|c| {
            c.execute(
                "INSERT INTO mcp_ai_audit(id,client_id,action,task_id,reason,intent,before_json,after_json,request_json,created_at)
                 VALUES(1,'synthetic','retain',?,'synthetic','synthetic','{}','{}','{}',?)",
                params![local.id, now()],
            ).map_err(display_error)?;
            c.execute(
                "INSERT INTO mcp_receipts(client_id,request_key,request_hash,task_id,audit_id,result_json)
                 VALUES('synthetic','retain','synthetic',?,1,'{}')",
                [local.id],
            ).map_err(display_error)?;
            c.execute("INSERT INTO mcp_undo_requests(audit_id,client_id,reason,created_at)
                       VALUES(1,'synthetic','retain',?)", [now()]).map_err(display_error)?;
            c.execute("INSERT INTO mcp_batch_approvals(client_id,request_key,request_json,basis_json,preview_json,created_at)
                       VALUES('synthetic','retain','{}','{}','{}',?)", [now()]).map_err(display_error)?;
            Ok(())
        })
        .unwrap();
    let b = f.source.create_backup("manual").unwrap();
    let hash = digest(&b.path);
    let path = f.target.import_backup(b.path.clone()).unwrap().path;
    let first = f.target.restore_backup(path.clone()).unwrap();
    let id = first.conflicts[0].task_id;
    assert_eq!(counts(first), (1, 0, 1));
    let saved = f.target.get_task(id).unwrap();
    assert_eq!(saved.ticket_color, Some("#1A2B3C".into()));
    assert_eq!(saved.internal_notes, "源内部备注");
    let before = current_business(&f.target);
    for _ in 0..2 {
        assert_eq!(
            counts(f.target.restore_backup(path.clone()).unwrap()),
            (0, 1, 0)
        );
        assert_eq!(current_business(&f.target), before);
    }
    f.target
        .with_conn(|c| {
            integrity(c, 15);
            let translated: (i64, i64) = c
                .query_row(
                    "SELECT original.id,corrected.id FROM task_work_events corrected
               JOIN task_work_events original ON original.id=corrected.correction_of
             WHERE corrected.task_id=?1 AND original.task_id=?1
               AND original.voided_at IS NOT NULL AND corrected.voided_at IS NULL",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(display_error)?;
            assert_ne!(translated.0, work);
            let receipt: i64 = c
                .query_row(
                    "SELECT r.task_id FROM mcp_receipts r JOIN mcp_ai_audit a ON a.id=r.audit_id AND a.task_id=r.task_id
                     JOIN mcp_undo_requests u ON u.audit_id=a.id AND u.status='pending'
                     JOIN mcp_batch_approvals b ON b.client_id=r.client_id AND b.request_key=r.request_key AND b.status='pending'
                     WHERE r.request_key='retain' AND r.result_json='{}'",
                    [],
                    |r| r.get(0),
                )
                .map_err(display_error)?;
            assert_eq!(receipt, local.id);
            Ok(())
        })
        .unwrap();
    let logs = f.target.get_logs(id).unwrap();
    assert_eq!(
        logs.iter()
            .filter(|l| l.operation_id.as_deref() == Some(op))
            .count(),
        2
    );
    assert_eq!(
        logs.iter()
            .filter(|l| l.operation_id.as_deref() == Some(other_op))
            .count(),
        1
    );
    assert!(logs
        .iter()
        .any(|l| l.content == "源人工备注" && l.operation_id.is_none()));
    assert!(logs.iter().any(|l| l.content == "旧版关联"
        && l.operation_id.as_deref() == Some("timeline:v1:cccccccccccccccccccccccccccccccc:1")));
    let history = f.target.mcp_history(id, true, true, false).unwrap();
    assert!(history.iter().any(|row| row["correctionOf"].is_i64()));
    assert_eq!(digest(&b.path), hash);
    println!("F3-05 matched closed + active queues, notes, two operation IDs and correction chain; digest={hash}");
    f.finish();
}

#[test]
fn legacy_restore_migrates_only_the_stage_and_rejects_unsafe_backups() {
    let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
    for version in [8, 14, 15] {
        let f = Fixture::new();
        let mut input = sample("兼容备份");
        input.details = "版本夹具".into();
        let item = f.source.save_task(input).unwrap();
        f.source.with_conn(|c| {
            c.execute("INSERT INTO task_logs(task_id,log_type,content,created_at)
                       VALUES(?,'timeline:v1:cccccccccccccccccccccccccccccccc:1:queue','版本兼容旧标识',?)",
                params![item.id, now()]).map_err(display_error)?;
            if version == 8 {
                c.execute_batch("DROP INDEX idx_scheduled_due; DROP TABLE queue_number_allocations;
                     ALTER TABLE tasks DROP COLUMN planned_date; ALTER TABLE tasks DROP COLUMN is_scheduled;
                     ALTER TABLE tasks DROP COLUMN schedule_action; ALTER TABLE tasks DROP COLUMN schedule_action_at;")
                    .map_err(display_error)?;
            }
            if version < 15 {
                c.execute_batch("ALTER TABLE task_logs DROP COLUMN operation_id;").map_err(display_error)?;
            }
            c.execute("UPDATE schema_meta SET version=?", [version]).map_err(display_error)?;
            Ok(())
        }).unwrap();
        let b = f.source.create_backup("manual").unwrap();
        let hash = digest(&b.path);
        let saved = Connection::open_with_flags(&b.path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        integrity(&saved, version);
        drop(saved);
        f.target.save_task(sample("兼容备份")).unwrap();
        let path = f.target.import_backup(b.path.clone()).unwrap().path;
        let first = f.target.restore_backup(path.clone()).unwrap();
        let id = first.conflicts[0].task_id;
        assert_eq!(counts(first), (1, 0, 1));
        let before = current_business(&f.target);
        assert_eq!(
            counts(f.target.restore_backup(path.clone()).unwrap()),
            (0, 1, 0)
        );
        assert_eq!(current_business(&f.target), before);
        assert_eq!(digest(&b.path), hash);
        assert_eq!(digest(path), hash);
        assert!(f
            .target
            .get_logs(id)
            .unwrap()
            .iter()
            .any(|l| l.content == "版本兼容旧标识" && l.operation_id.is_some()));
        f.target
            .with_conn(|c| {
                integrity(c, 15);
                Ok(())
            })
            .unwrap();
        staged_files_cleared(&f.target);
        println!(
            "F3-07 v{version} -> target v15 repeated restore: digest={hash}, integrity=ok, FK=0"
        );
        f.finish();
    }

    let f = Fixture::new();
    f.target.save_task(sample("本地事项")).unwrap();
    let before = current_business(&f.target);
    let corrupt = f.target.backup_dir.join("synthetic-corrupt.db");
    fs::write(&corrupt, b"invalid SQLite file").unwrap();
    let future = f.target.backup_dir.join("synthetic-v16.db");
    fs::copy(f.source.create_backup("manual").unwrap().path, &future).unwrap();
    Connection::open(&future)
        .unwrap()
        .execute("UPDATE schema_meta SET version=16", [])
        .unwrap();
    let generation_before = f.target.with_conn(|c| Ok(generation(c))).unwrap();
    for path in [corrupt, future] {
        let hash = digest(&path);
        let error = f
            .target
            .restore_backup(path.to_string_lossy().into_owned())
            .unwrap_err();
        assert_eq!(digest(&path), hash);
        assert_eq!(current_business(&f.target), before);
        assert_eq!(
            f.target.with_conn(|c| Ok(generation(c))).unwrap(),
            generation_before
        );
        println!(
            "F3-07 rejected {}: {error}; digest={hash}",
            path.file_name().unwrap().to_string_lossy()
        );
    }
    assert!(f
        .target
        .list_backups()
        .unwrap()
        .iter()
        .all(|b| !b.name.contains("before-restore")));
    staged_files_cleared(&f.target);
    f.finish();
}

#[test]
fn failed_restore_rolls_back_owner_history_and_generation_and_preserves_emergency_file() {
    let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
    let f = Fixture::new();
    let a = f.source.save_task(sample("待回滚源事项")).unwrap();
    f.source.add_log(a.id, "触发回滚".into()).unwrap();
    f.target.save_task(sample("本地保全事项")).unwrap();
    let backup = f.source.create_backup("manual").unwrap();
    let path = f.target.import_backup(backup.path.clone()).unwrap().path;
    let source_hash = digest(&backup.path);
    let before = current_business(&f.target);
    let generation_before = f.target.with_conn(|c| Ok(generation(c))).unwrap();
    f.target
        .with_conn(|c| {
            c.execute_batch(
                "CREATE TRIGGER synthetic_restore_abort BEFORE INSERT ON task_logs
         WHEN NEW.content='触发回滚' BEGIN SELECT RAISE(ABORT,'synthetic restore rollback'); END;",
            )
            .map_err(display_error)
        })
        .unwrap();
    let error = f.target.restore_backup(path.clone()).unwrap_err();
    assert!(error.contains("synthetic restore rollback"), "{error}");
    assert_eq!(current_business(&f.target), before);
    assert_eq!(
        f.target.with_conn(|c| Ok(generation(c))).unwrap(),
        generation_before
    );
    let backup = f
        .target
        .list_backups()
        .unwrap()
        .into_iter()
        .find(|b| b.name.contains("before-restore"))
        .unwrap();
    let saved =
        Connection::open_with_flags(&backup.path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    integrity(&saved, 15);
    assert_eq!(business(&saved), before);
    assert_eq!(generation(&saved), generation_before);
    drop(saved);
    assert_eq!(digest(path), source_hash);
    staged_files_cleared(&f.target);
    f.target
        .with_conn(|c| {
            integrity(c, 15);
            Ok(())
        })
        .unwrap();
    println!("F3-06 rollback: {error}; generation={generation_before} source_sha256={source_hash} emergency_sha256={}", digest(backup.path));
    f.finish();
}
