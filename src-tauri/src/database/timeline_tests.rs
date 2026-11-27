use super::{
    tests::{sample, subtask_sample, TestClock},
    *,
};
use sha2::{Digest, Sha256};

fn fixture(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "inline-f2-{name}-{}",
        Utc::now().timestamp_nanos_opt().unwrap()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

fn sha(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

fn raw_log(db: &Database, task_id: i64, kind: &str, text: &str, operation: Option<&str>) {
    db.with_conn(|c| {
        c.execute(
            "INSERT INTO task_logs(task_id,log_type,content,created_at,operation_id) VALUES(?,?,?,?,?)",
            params![task_id, kind, text, now(), operation],
        ).map_err(display_error)?;
        Ok(())
    }).unwrap();
}

fn log_events(db: &Database, after: u64) -> Vec<String> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT operation FROM mcp_change_events WHERE source='task_logs' AND sequence>? ORDER BY sequence"
        ).map_err(display_error)?;
        let rows = stmt.query_map([after], |row| row.get(0)).map_err(display_error)?;
        rows.collect::<Result<_, _>>().map_err(display_error)
    }).unwrap()
}

fn enqueue(id: i64) -> QueueInput {
    QueueInput {
        id,
        inherit_deadline: false,
        reason: "合成继续办理".into(),
    }
}

#[test]
fn timeline_operations_keep_repeated_actions_notes_and_audit_independent() {
    let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
    let root = fixture("operations");
    let db = Database::open_at(root.join("inline.db")).unwrap();
    let task = db.save_task(sample("阶段2合成事项")).unwrap();
    let before = db.mcp_basis().unwrap().commit_sequence;
    let previous_len = db.get_logs(task.id).unwrap().len();
    db.with_transaction(|tx| {
        process_round_on_domain(tx, task.id)?;
        enqueue_task_on_domain(tx, enqueue(task.id))?;
        process_round_on_domain(tx, task.id)
    })
    .unwrap();
    let logs = db.get_logs(task.id).unwrap();
    let work: Vec<_> = logs.iter().filter(|row| row.log_type == "work").collect();
    assert_eq!(work.len(), 2);
    assert_eq!(work[0].created_at, work[1].created_at);
    assert!(work[0].id > work[1].id);
    assert_ne!(work[0].operation_id, work[1].operation_id);
    for row in work {
        assert!(row
            .operation_id
            .as_deref()
            .unwrap()
            .starts_with("timeline:v2:"));
        assert!(logs
            .iter()
            .any(|other| other.log_type == "queue" && other.operation_id == row.operation_id));
    }
    assert_eq!(
        log_events(&db, before),
        vec!["INSERT"; logs.len() - previous_len]
    );
    db.add_log(task.id, "保留 pending 原文".into()).unwrap();
    let event = db.list_work_events(task.id).unwrap()[0].id;
    db.void_work_event(event, true).unwrap();
    let logs = db.get_logs(task.id).unwrap();
    for kind in ["note", "audit"] {
        assert!(logs
            .iter()
            .any(|row| row.log_type == kind && row.operation_id.is_none()));
    }
    let note = logs.iter().find(|row| row.log_type == "note").unwrap();
    db.update_log(note.id, "更新 pending 原文".into()).unwrap();
    assert_eq!(
        db.get_logs(task.id)
            .unwrap()
            .iter()
            .find(|row| row.id == note.id)
            .unwrap()
            .content,
        "更新 pending 原文"
    );
    db.delete_log(note.id).unwrap();

    db.enqueue_task(enqueue(task.id)).unwrap();
    db.with_conn(|conn| {
        conn.execute_batch(
            "CREATE TEMP TRIGGER f2_fail_log BEFORE INSERT ON task_logs
         WHEN NEW.log_type='work' BEGIN SELECT RAISE(ABORT,'synthetic log failure'); END",
        )
        .map_err(display_error)
    })
    .unwrap();
    let before = db.mcp_basis().unwrap();
    let before_logs = db.get_logs(task.id).unwrap().len();
    assert!(db
        .process_round(task.id)
        .unwrap_err()
        .contains("synthetic log failure"));
    assert_eq!(db.mcp_basis().unwrap(), before);
    assert_eq!(db.get_logs(task.id).unwrap().len(), before_logs);
    db.with_conn(|conn| {
        conn.execute_batch("DROP TRIGGER f2_fail_log")
            .map_err(display_error)
    })
    .unwrap();
    db.process_round(task.id).unwrap();
    let retry = log_events(&db, before.commit_sequence);
    assert!(!retry.is_empty() && retry.iter().all(|operation| operation == "INSERT"));
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn timeline_legacy_history_projects_original_types_and_preserves_redaction() {
    let root = fixture("legacy");
    let db = Database::open_at(root.join("inline.db")).unwrap();
    let task = db.save_task(sample("合成历史事项")).unwrap();
    let legacy = "timeline:v1:ABCDEF0123456789ABCDEF0123456789:91:relation";
    let invalid = "timeline:v1:defective:91:merged";
    let modern = "timeline:v2:cccccccccccccccccccccccccccccccc:13";
    for (kind, text, op) in [
        (legacy, "隐去关联来源", Some(modern)),
        (legacy, "只读旧来源", None),
        (invalid, "隐去异常合并来源", None),
        ("queue", "无关联记录", None),
        ("future_unknown", "未知类型", Some(modern)),
        ("note", "pending 原备注", Some(modern)),
    ] {
        raw_log(&db, task.id, kind, text, op);
    }
    let logs = db.get_logs(task.id).unwrap();
    let by_text = |text: &str| logs.iter().find(|row| row.content == text).unwrap();
    assert_eq!(
        by_text("隐去关联来源").operation_id.as_deref(),
        Some(modern)
    );
    assert_eq!(
        by_text("只读旧来源").operation_id.as_deref(),
        Some("timeline:v1:abcdef0123456789abcdef0123456789:91")
    );
    for text in [
        "隐去异常合并来源",
        "无关联记录",
        "未知类型",
        "pending 原备注",
    ] {
        assert!(by_text(text).operation_id.is_none());
    }
    let limited = db.mcp_history(task.id, true, true, true).unwrap();
    let row = |text: &str| {
        let id = by_text(text).id;
        limited
            .iter()
            .find(|row| row["kind"] == "log" && row["id"] == id)
            .unwrap()
    };
    for text in ["隐去关联来源", "只读旧来源", "隐去异常合并来源"] {
        assert!(row(text).get("text").is_none());
        assert_eq!(row(text)["redactedFields"][0], "text");
    }
    assert_eq!(row("只读旧来源")["from"], "relation");
    assert_eq!(row("隐去异常合并来源")["from"], "unknown");
    assert_eq!(row("pending 原备注")["text"], "pending 原备注");
    let full = db.mcp_history(task.id, true, true, false).unwrap();
    assert_eq!(
        full.iter()
            .filter(|row| row["kind"] == "log" && row["from"] == "relation")
            .count(),
        2
    );
    let serialized = serde_json::to_string(&full).unwrap();
    assert!(!serialized.contains("timeline:v") && !serialized.contains("operationId"));
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn timeline_operations_survive_restore_merge_and_legacy_import() {
    let _clock = TestClock::at("2026-10-02T10:00:00+08:00");
    let root = fixture("restore");
    let source = Database::open_at(root.join("source").join("inline.db")).unwrap();
    let target = Database::open_at(root.join("target").join("inline.db")).unwrap();
    let src = source.save_task(sample("合成恢复事项")).unwrap();
    target.save_task(sample("编号占位事项")).unwrap();
    let a = "timeline:v2:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1";
    let b = "timeline:v2:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:1";
    let legacy = "timeline:v1:cccccccccccccccccccccccccccccccc:99:queue";
    for op in [a, a, b] {
        raw_log(&source, src.id, "queue", "同秒同文字", Some(op));
    }
    raw_log(&source, src.id, legacy, "旧标识原值", None);
    raw_log(&source, src.id, "queue", "未标识自动记录", None);
    source.add_log(src.id, "原文备注".into()).unwrap();
    let backup = source.create_backup("manual").unwrap();
    let backup_hash = sha(Path::new(&backup.path));
    let imported = target.import_backup(backup.path.clone()).unwrap();
    assert_eq!(
        target
            .restore_backup(imported.path.clone())
            .unwrap()
            .added_tasks,
        1
    );
    let tasks = target.list_tasks(TaskView::Queue).unwrap();
    let id = tasks
        .iter()
        .find(|task| task.title == "合成恢复事项")
        .unwrap()
        .id;
    assert_ne!(id, src.id);
    let first = target.get_logs(id).unwrap();
    assert_eq!(
        first
            .iter()
            .filter(|row| row.content == "同秒同文字")
            .count(),
        3
    );
    assert!(first
        .iter()
        .any(|row| row.log_type == "backup_number_remapped"
            && row
                .operation_id
                .as_deref()
                .is_some_and(|op| op != a && op != b)));
    let repeated = target.restore_backup(imported.path.clone()).unwrap();
    assert_eq!((repeated.added_tasks, repeated.merged_tasks), (0, 1));
    assert_eq!(target.get_logs(id).unwrap().len(), first.len());
    assert_eq!(sha(Path::new(&backup.path)), backup_hash);
    assert_eq!(sha(Path::new(&imported.path)), backup_hash);
    let merge = target.save_task(sample("合成同源移动")).unwrap();
    for _ in 0..3 {
        raw_log(&target, merge.id, "queue", "同秒同文字", Some(a));
    }
    let moved = target
        .get_logs(merge.id)
        .unwrap()
        .into_iter()
        .find(|row| row.log_type == "created")
        .unwrap()
        .operation_id;
    target
        .merge_tasks(MergeTaskInput {
            target_task_id: id,
            source_task_id: merge.id,
            deduplicate_records: true,
            trash_source: true,
        })
        .unwrap();
    let logs = target.get_logs(id).unwrap();
    assert_eq!(
        logs.iter()
            .filter(|row| row.operation_id.as_deref() == Some(a))
            .count(),
        3
    );
    assert_eq!(
        logs.iter()
            .filter(|row| row.operation_id.as_deref() == Some(b))
            .count(),
        1
    );
    assert!(logs.iter().any(|row| row.operation_id == moved));
    assert!(logs
        .iter()
        .any(|row| row.content == "原文备注" && row.operation_id.is_none()));
    target
        .with_conn(|conn| {
            assert_eq!(
                conn.query_row(
                    "SELECT log_type FROM task_logs WHERE content='旧标识原值'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
                legacy
            );
            Database::check_integrity(conn)
        })
        .unwrap();
    drop(target);
    drop(source);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn timeline_v14_migration_backs_up_and_keeps_a_failed_ddl_atomic() {
    let root = fixture("migration");
    let db = Database::open_at(root.join("inline.db")).unwrap();
    let parent = db.save_task(sample("迁移合成父事项")).unwrap();
    let child = db
        .create_subtask(subtask_sample(parent.id, "迁移合成子事项"))
        .unwrap();
    db.process_round(child.id).unwrap();
    db.add_log(child.id, "原文迁移备注".into()).unwrap();
    db.set_setting("week_start_day".into(), "sunday".into())
        .unwrap();
    db.with_conn(|conn| {
        conn.execute_batch(
            "ALTER TABLE task_logs DROP COLUMN operation_id; UPDATE schema_meta SET version=14;
         CREATE TRIGGER block_timeline_v15 BEFORE INSERT ON schema_meta WHEN NEW.version=15
         BEGIN SELECT RAISE(ABORT,'synthetic v15 migration failure'); END;",
        )
        .map_err(display_error)
    })
    .unwrap();
    drop(db);
    let snapshot = |c: &Connection| {
        let mut statement = c
            .prepare("SELECT id,task_id,log_type,content,created_at FROM task_logs ORDER BY id")
            .unwrap();
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .unwrap();
        rows.collect::<Result<Vec<_>, _>>().unwrap()
    };
    let c = Database::connect(&root.join("inline.db")).unwrap();
    let before = snapshot(&c);
    drop(c);
    assert!(Database::open_root(root.clone())
        .err()
        .unwrap()
        .contains("synthetic v15 migration failure"));
    let failed = Database::connect(&root.join("inline.db")).unwrap();
    assert_eq!(Database::schema_version(&failed).unwrap(), 14);
    assert_eq!(snapshot(&failed), before);
    assert!(failed
        .prepare("SELECT operation_id FROM task_logs")
        .is_err());
    Database::check_integrity(&failed).unwrap();
    let backup = fs::read_dir(root.join("backups"))
        .unwrap()
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains("before-migration")
        })
        .unwrap()
        .path();
    let backup_reader =
        Connection::open_with_flags(&backup, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    Database::check_integrity(&backup_reader).unwrap();
    assert_eq!(Database::schema_version(&backup_reader).unwrap(), 14);
    assert_eq!(snapshot(&backup_reader), before);
    drop(backup_reader);
    println!("F2 synthetic failed v14 backup SHA256={}", sha(&backup));
    failed
        .execute_batch("DROP TRIGGER block_timeline_v15")
        .unwrap();
    drop(failed);
    let upgraded = Database::open_root(root.clone()).unwrap();
    upgraded
        .with_conn(|conn| {
            assert_eq!(Database::schema_version(conn)?, 15);
            assert_eq!(snapshot(conn), before);
            Database::check_integrity(conn)
        })
        .unwrap();
    assert!(upgraded
        .get_logs(child.id)
        .unwrap()
        .iter()
        .all(|row| row.operation_id.is_none()));
    let current = upgraded.get_task(child.id).unwrap();
    assert_eq!(
        (current.parent_task_id, current.permanent_number.as_str()),
        (Some(parent.id), child.permanent_number.as_str())
    );
    assert_eq!(upgraded.list_work_events(child.id).unwrap().len(), 1);
    assert_eq!(upgraded.settings().unwrap()["week_start_day"], "sunday");
    drop(upgraded);
    let reopened = Database::open_root(root.clone()).unwrap();
    reopened.with_conn(Database::check_integrity).unwrap();
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn timeline_groups_parent_child_and_scheduled_effects_by_original_task() {
    let clock = TestClock::at("2026-10-02T10:00:00+08:00");
    let root = fixture("schedule");
    let db = Database::open_at(root.join("inline.db")).unwrap();
    let parent = db.save_task(sample("合成父事项")).unwrap();
    let child = db
        .create_subtask(subtask_sample(parent.id, "合成子事项"))
        .unwrap();
    let created = db.get_logs(child.id).unwrap();
    let relation = created
        .iter()
        .find(|log| log.log_type == "relation")
        .unwrap();
    assert!(created
        .iter()
        .any(|log| log.log_type == "created" && log.operation_id == relation.operation_id));
    db.complete_task(CompleteTaskInput {
        task_id: parent.id,
        include_eligible_subtasks: true,
    })
    .unwrap();
    let completed = |id| {
        let logs = db.get_logs(id).unwrap();
        let work = logs.iter().find(|log| log.log_type == "work").unwrap();
        assert!(logs
            .iter()
            .any(|log| log.log_type == "queue" && log.operation_id == work.operation_id));
        work.operation_id.clone().unwrap()
    };
    assert_ne!(completed(parent.id), completed(child.id));
    db.set_parent_task(child.id, None).unwrap();
    let after = db.get_logs(child.id).unwrap();
    assert!(after
        .iter()
        .any(|log| log.log_type == "relation" && log.operation_id != relation.operation_id));
    let mut pending = sample("合成计划一");
    pending.planned_date = Some("2026-10-05".into());
    let first = db.save_task(pending.clone()).unwrap();
    pending.title = "合成计划二".into();
    let second = db.save_task(pending.clone()).unwrap();
    pending.title = "合成提前办理".into();
    let early = db.save_task(pending).unwrap();
    db.process_round(early.id).unwrap();
    let early_logs = db.get_logs(early.id).unwrap();
    let worked = early_logs
        .iter()
        .find(|log| log.log_type == "work")
        .unwrap();
    for kind in ["scheduled_early", "queue"] {
        assert!(early_logs
            .iter()
            .any(|log| log.log_type == kind && log.operation_id == worked.operation_id));
    }
    drop(clock);
    let _clock = TestClock::at("2026-10-05T10:00:00+08:00");
    let before = db.mcp_basis().unwrap().commit_sequence;
    assert_eq!(db.activate_due_scheduled().unwrap(), 2);
    let activated = |id| {
        db.get_logs(id)
            .unwrap()
            .into_iter()
            .find(|log| log.log_type == "scheduled_activated")
            .unwrap()
            .operation_id
            .unwrap()
    };
    assert_ne!(activated(first.id), activated(second.id));
    assert_eq!(log_events(&db, before), vec!["INSERT"; 2]);
    drop(db);
    fs::remove_dir_all(root).unwrap();
}
