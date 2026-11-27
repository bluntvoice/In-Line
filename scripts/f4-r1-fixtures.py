"""Synthetic historical fixtures and read-only verification for hosted GUI."""
import json
import sqlite3
import sys
from datetime import date, datetime, time, timedelta, timezone
from pathlib import Path

db_path, mode, *args = sys.argv[1:]
assert (mode == "backup-source" and Path(db_path).name == "import-source.db") or db_path.lower().replace("\\", "/").endswith("/in-line/inline.db")
assert mode in {"timeline-seed", "timeline-read", "stats-seed", "business-read", "backup-source"}
mutates = mode in {"timeline-seed", "stats-seed"}
db = sqlite3.connect(Path(db_path).resolve().as_uri() + ("?mode=rw" if mutates else "?mode=ro"), uri=True)
db.row_factory = sqlite3.Row
db.execute("PRAGMA foreign_keys=ON")
assert db.execute("select version from schema_meta").fetchone()[0] == 15

def task(title):
    rows = db.execute("select * from tasks where title=?", (title,)).fetchall()
    assert len(rows) == 1, (title, len(rows))
    return rows[0]

def stamp(day):
    return datetime.combine(day, time(12), tzinfo=timezone.utc).isoformat().replace("+00:00", "Z")

if mode == "backup-source":
    a = task("F4 source")
    assert a["internal_notes"] == "original source note" and db.execute("select count(*) from tasks").fetchone()[0] == 1
    print(json.dumps({"sourceValidated": True, "tasks": 1}))
elif mode == "timeline-seed":
    a = task("F4 R1 timeline A")
    assert db.execute("select count(*) from tasks").fetchone()[0] == 2
    assert not db.execute("select 1 from task_logs where content like 'R1 %'").fetchone()
    seed = []
    for version, digit in ((1, "a"), (2, "b")):
        key = f"timeline:v{version}:{digit * 32}:1"
        seed.extend([
            (a["id"], f"{key}:created" if version == 1 else "created", f"R1 v{version} 主动作", "2024-05-06T12:31:00Z", None if version == 1 else key),
            (a["id"], f"{key}:queue" if version == 1 else "queue", f"R1 v{version} 队列原文", "2024-05-06T12:32:00Z", None if version == 1 else key),
        ])
    seed.append((a["id"], "audit", "R1 独立业务审计", "2024-05-06T12:33:00Z", seed[-1][-1]))
    with db:
        db.executemany("insert into task_logs(task_id,log_type,content,created_at,operation_id) values(?,?,?,?,?)", seed)
elif mode == "timeline-read":
    a = task("F4 R1 timeline A")
    rows = [dict(row) for row in db.execute(
        "select log_type,content,created_at,operation_id from task_logs where task_id=? order by id", (a["id"],)
    )]
    work_ids = {row["operation_id"] for row in rows if row["log_type"] == "work"}
    assert None not in work_ids and len(work_ids) == 1
    work = [row for row in rows if row["operation_id"] in work_ids]
    assert {row["log_type"] for row in work} == {"work", "queue"} and len(work) == 2, work
    notes = [row["content"] for row in rows if row["log_type"] == "note"]
    assert notes == args, notes
    for version in (1, 2):
        assert sum(row["log_type"] != "audit" and (row["operation_id"] or row["log_type"]).startswith(f"timeline:v{version}:") and row["content"].startswith("R1 ") for row in rows) == 2
    assert sum(row["log_type"] == "audit" for row in rows) == 1
    print(json.dumps(rows, ensure_ascii=False))
elif mode == "stats-seed":
    assert db.execute("select count(*) from tasks").fetchone()[0] == 3
    today = date.fromisoformat(args[0])
    for suffix, day in (("New A", today), ("New B", today), ("Prior", today - timedelta(days=7))):
        row = task(f"F4 R1 {suffix}")
        with db:
            result = db.execute("update task_work_events set handled_at=? where task_id=? and voided_at is null", (stamp(day), row["id"]))
            assert result.rowcount == 1
    print(json.dumps([dict(row) for row in db.execute("select title, handled_at, result_status from task_work_events join tasks on tasks.id=task_id order by tasks.id")]))
else:
    title, state, data = args
    a = task(title)
    status = a["status"]
    queue = [dict(x) for x in db.execute("select queue_date,daily_sequence,closed_at,close_reason from task_queue_entries where task_id=? order by id", (a["id"],))]
    work = [dict(x) for x in db.execute("select result_status,handled_at from task_work_events where task_id=? and voided_at is null order by id", (a["id"],))]
    logs = [x[0] for x in db.execute("select log_type from task_logs where task_id=?", (a["id"],))]
    alloc = [dict(x) for x in db.execute("select * from queue_number_allocations where task_id=?", (a["id"],))]
    active = [x for x in queue if not x["closed_at"]]
    if state == "edited":
        assert a["internal_notes"] == data and len(active) == 1 and "updated" in logs
    elif state == "deferred":
        assert status == "processed" and not active and work[-1]["result_status"] == "processed"
    elif state == "enqueued":
        assert len(active) == 1 and "queue" in logs
    elif state == "completed":
        assert status == "completed" and not active and work[-1]["result_status"] == "completed"
    elif state == "reopened":
        assert status == "pending" and len(active) == 1 and any(e["result_status"] == "completed" for e in work)
    elif state == "parent":
        parent = task(data)
        assert a["parent_task_id"] == parent["id"] and a["internal_notes"] != parent["internal_notes"]
    elif state == "unlinked":
        assert a["parent_task_id"] is None
    elif state == "urgent":
        assert a["is_urgent"] and a["urgent_requester"] == "R1 synthetic requester" and a["urgent_reason"] == "R1 deadline check"
        assert a["requested_deadline"], "Deadline not persisted"
    elif state == "scheduled":
        assert a["planned_date"] == data and not active and alloc
    elif state == "early":
        assert len(active) == 1 and "scheduled_early" in logs and len(alloc) >= 2
    elif state == "trash":
        assert a["deleted_at"] and not active
    elif state == "restored":
        assert not a["deleted_at"] and len(active) == 1 and "restored" in logs
    elif state == "merged":
        source = task(data)
        assert source["deleted_at"] and "merged" in logs and len(active) == 1
    else:
        raise AssertionError(state)
    print(json.dumps({"id": a["id"], "number": a["permanent_number"], "status": status, "parent": a["parent_task_id"], "queue": queue, "work": work, "logTypes": logs, "allocations": alloc}, ensure_ascii=False))
assert db.execute("PRAGMA integrity_check").fetchone()[0] == "ok"
assert not db.execute("PRAGMA foreign_key_check").fetchall()
