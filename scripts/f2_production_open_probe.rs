//! Hosted F2 production-open and rollback probe, copied into the fixed v14 and candidate test targets.
use in_line_lib::{
    database::{Database, DATABASE_SCHEMA_VERSION},
    models::{CreateSubtaskInput, TaskInput, TaskView, WorkEventInput},
};
use rusqlite::{types::Value, Connection, OpenFlags};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn input(title: &str) -> TaskInput {
    serde_json::from_value(json!({"title":title,"department":"合成部门","departments":["合成部门"],"contact":"合成人","contacts":["合成人"],"taskType":"任务处理","details":"合成迁移事实","internalNotes":"测试备注","status":"pending","priority":"normal","workload":"standard","isUrgent":false})).unwrap()
}

fn inspect(path: &Path) -> Connection {
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(
        c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    c
}

fn version(c: &Connection) -> i64 {
    c.query_row("SELECT max(version) FROM schema_meta", [], |r| r.get(0))
        .unwrap()
}

fn hash(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

fn snapshot(c: &Connection) -> Vec<(&'static str, Vec<Vec<Value>>)> {
    [
        "tasks",
        "task_logs",
        "task_queue_entries",
        "task_work_events",
        "status_history",
        "urgent_records",
        "queue_number_allocations",
        "settings",
        "daily_sequences",
        "master_values",
    ]
    .into_iter()
    .map(|table| {
        let mut columns = c
            .prepare(&format!(
                "SELECT name FROM pragma_table_info('{table}') ORDER BY cid"
            ))
            .unwrap();
        let names: Vec<_> = columns
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .filter(|name| name != "operation_id")
            .collect();
        let projection = names
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(",");
        let mut stmt = c
            .prepare(&format!("SELECT {projection} FROM {table} ORDER BY 1"))
            .unwrap();
        let count = stmt.column_count();
        let rows = stmt
            .query_map([], |row| {
                (0..count).map(|index| row.get::<_, Value>(index)).collect()
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        (table, rows)
    })
    .collect()
}

fn backups(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root.join("backups"))
        .unwrap()
        .map(Result::unwrap)
        .filter(|item| {
            item.file_name()
                .to_string_lossy()
                .ends_with("-before-migration.db")
        })
        .map(|item| item.path())
        .collect()
}

#[test]
fn f2_production_open_probe() {
    assert_eq!(env::var("GITHUB_ACTIONS").as_deref(), Ok("true"));
    assert_eq!(
        env::var("RUNNER_ENVIRONMENT").as_deref(),
        Ok("github-hosted")
    );
    let root = PathBuf::from(env::var_os("IN_LINE_MCP_TEST_DATA_ROOT").unwrap());
    let allowed = PathBuf::from(env::var_os("F2_RUN_ROOT").unwrap());
    assert!(root.starts_with(&allowed) && root != allowed);
    let path = root.join("inline.db");
    let mode = env::var("F2_PROBE_MODE").unwrap();
    match mode.as_str() {
        "seed" => {
            assert_eq!(DATABASE_SCHEMA_VERSION, 14);
            assert!(!path.exists());
            let db = Database::open().unwrap();
            let parent = db.save_task(input("F2 合成迁移父事项")).unwrap();
            let child=db.create_subtask(serde_json::from_value::<CreateSubtaskInput>(json!({
                "parentTaskId":parent.id,"title":"F2 合成迁移子事项","details":"原业务字段","isUrgent":false,"enqueueToday":true
            })).unwrap()).unwrap();
            db.process_round(child.id).unwrap();
            let event = db.list_work_events(child.id).unwrap()[0].id;
            db.void_work_event(event, true).unwrap();
            db.record_work_event(WorkEventInput {
                task_id: child.id,
                result_status: "completed".into(),
                handled_at: "2026-09-30T09:30:00+08:00".into(),
                note: "合成纠错记录".into(),
                sync_status: false,
            })
            .unwrap();
            db.add_log(child.id, "迁移前备注 pending 原文".into())
                .unwrap();
            db.set_setting("week_start_day".into(), "sunday".into())
                .unwrap();
            drop(db);
            let fixture = Connection::open(&path).unwrap();
            fixture
                .execute(
                    "UPDATE task_work_events SET correction_of=? WHERE task_id=? AND id<>?",
                    rusqlite::params![event, child.id, event],
                )
                .unwrap();
            drop(fixture);
            assert_eq!(version(&inspect(&path)), 14);
            println!("F2 actual phase1 seed schema=14 sha256={}", hash(&path));
        }
        "upgrade" => {
            assert_eq!(DATABASE_SCHEMA_VERSION, 15);
            let read = inspect(&path);
            assert_eq!(version(&read), 14);
            let expected = snapshot(&read);
            drop(read);
            let db = Database::open().unwrap();
            let saved = backups(&root);
            assert_eq!(saved.len(), 1);
            let saved_db = inspect(&saved[0]);
            assert_eq!(version(&saved_db), 14);
            assert!(saved_db
                .prepare("SELECT operation_id FROM task_logs")
                .is_err());
            assert_eq!(snapshot(&saved_db), expected);
            drop(saved_db);
            let migrated = inspect(&path);
            assert_eq!(version(&migrated), 15);
            assert_eq!(snapshot(&migrated), expected);
            assert_eq!(
                migrated
                    .query_row(
                        "SELECT count(*) FROM task_logs WHERE operation_id IS NOT NULL",
                        [],
                        |r| r.get::<_, i64>(0)
                    )
                    .unwrap(),
                0
            );
            assert_eq!(
                migrated
                    .query_row(
                        "SELECT count(*) FROM task_work_events WHERE correction_of IS NOT NULL",
                        [],
                        |r| r.get::<_, i64>(0)
                    )
                    .unwrap(),
                1
            );
            drop(migrated);
            println!(
                "F2 actual before-migration schema=14 PRAGMA ok/0 sha256={}",
                hash(&saved[0])
            );
            println!(
                "F2 matching business tables {:?}",
                expected
                    .iter()
                    .map(|(name, rows)| (*name, rows.len()))
                    .collect::<Vec<_>>()
            );
            let fresh = db.save_task(input("F2 升级后新增哨兵")).unwrap();
            drop(db);
            let reopened = Database::open().unwrap();
            assert_eq!(
                reopened.get_task(fresh.id).unwrap().title,
                "F2 升级后新增哨兵"
            );
            assert_eq!(backups(&root), saved);
            drop(reopened);
            assert_eq!(version(&inspect(&path)), 15);
            println!("F2 v15 reopen PASS sha256={}", hash(&path));
        }
        "reject" => {
            assert_eq!(DATABASE_SCHEMA_VERSION, 14);
            let checksum = hash(&path);
            let error = Database::open().err().expect("actual v14 must reject v15");
            assert!(error.contains("数据库版本高于本程序"));
            assert_eq!(version(&inspect(&path)), 15);
            assert_eq!(hash(&path), checksum);
            println!("F2 actual v14 refusal PASS sha256={checksum}");
        }
        "rollback" => {
            assert_eq!(DATABASE_SCHEMA_VERSION, 14);
            let read = inspect(&path);
            assert_eq!(version(&read), 14);
            let expected = snapshot(&read);
            drop(read);
            let db = Database::open().unwrap();
            assert!(!db
                .list_tasks(TaskView::Queue)
                .unwrap()
                .iter()
                .any(|t| t.title == "F2 升级后新增哨兵"));
            assert!(db
                .get_logs(2)
                .unwrap()
                .iter()
                .any(|log| log.content == "迁移前备注 pending 原文"));
            drop(db);
            let read = inspect(&path);
            assert_eq!(snapshot(&read), expected);
            println!(
                "F2 actual v14 rollback PASS PRAGMA ok/0; restored snapshot excludes v15 sentinel"
            );
        }
        _ => panic!("unsupported synthetic probe mode"),
    }
}
