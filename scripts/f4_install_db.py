import hashlib
import json
import sqlite3
import sys
from pathlib import Path

db_path = Path(sys.argv[2])
mode = sys.argv[1]
schema = int(sys.argv[3])
ID = 41001
DAY = "2026-10-03"
STAMP = DAY + "T09:00:00+08:00"
key = "f4_post_upgrade"
writing = mode in ("seed", "sentinel", "checkpoint")
connection = sqlite3.connect(db_path if writing else db_path.resolve().as_uri() + "?mode=ro", uri=not writing, timeout=5)
connection.execute("PRAGMA foreign_keys=ON")
assert connection.execute("SELECT version FROM schema_meta").fetchone() == (schema,)
if mode == "seed":
    connection.execute(
        """INSERT INTO tasks(id,permanent_number,daily_sequence,ticket_date,department,contact,task_type,
                              title,details,created_at,updated_at)
               VALUES(?, ?, 1, ?, 'F4 LAB', 'Synthetic', 'Synthetic', 'F4 parent', 'fixture', ?, ?)""",
        (ID, "20261003-01", DAY, STAMP, STAMP)
    )
    connection.execute(
        """INSERT INTO tasks(id,permanent_number,daily_sequence,ticket_date,department,contact,task_type,
                              title,details,created_at,updated_at,parent_task_id,subtask_sort_order)
               VALUES(?, ?, 2, ?, 'F4 LAB', 'Synthetic', 'Synthetic', 'F4 child', 'fixture', ?, ?, ?, 1)""",
        (ID + 1, "20261003-02", DAY, STAMP, STAMP, ID)
    )
    connection.execute("INSERT INTO task_logs(task_id,log_type,content,created_at) VALUES(?,'note','F4 note',?)", (ID, STAMP))
    connection.execute("INSERT INTO status_history(task_id,new_status,reason,created_at) VALUES(?,'pending','F4 history',?)", (ID, STAMP))
    connection.execute("INSERT INTO task_queue_entries(task_id,queue_date,daily_sequence,enqueued_at,created_at,updated_at) VALUES(?,?,1,?,?,?)", (ID, DAY, STAMP, STAMP, STAMP))
    if schema >= 14:
        connection.execute("UPDATE tasks SET planned_date=?,ticket_color='#112233' WHERE id=?", (DAY, ID))
        connection.execute("UPDATE tasks SET planned_date=?,is_scheduled=1 WHERE id=?", (DAY, ID + 1))
        for task, seq in ((ID, 1), (ID + 1, 2)):
            connection.execute("INSERT INTO queue_number_allocations(task_id,permanent_number,queue_date,daily_sequence,allocated_at) VALUES(?,?,?,?,?)",
                               (task, f"20261003-{seq:02d}", DAY, seq, STAMP))
    connection.execute("INSERT INTO daily_sequences(ticket_date,last_sequence) VALUES(?,2)", (DAY,))
elif mode == "sentinel":
    connection.execute("INSERT INTO settings(key,value) VALUES(?, 'data created after upgrade')", (key,))
if writing:
    connection.commit()
    assert connection.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone()[0] == 0
if mode != "version":
    assert connection.execute("PRAGMA integrity_check").fetchall() == [("ok",)]
    assert connection.execute("PRAGMA foreign_key_check").fetchall() == []
    tasks = connection.execute("SELECT id,permanent_number,title,parent_task_id FROM tasks WHERE id IN (?,?) ORDER BY id", (ID, ID+1)).fetchall()
    assert tasks == [(ID, "20261003-01", "F4 parent", None), (ID+1, "20261003-02", "F4 child", ID)]
    assert connection.execute("SELECT count(*) FROM task_logs WHERE task_id=? AND content='F4 note'", (ID,)).fetchone() == (1,)
    assert connection.execute("SELECT count(*) FROM status_history WHERE task_id=? AND reason='F4 history'", (ID,)).fetchone() == (1,)
    assert connection.execute("SELECT count(*) FROM task_queue_entries WHERE task_id=? AND daily_sequence=1", (ID,)).fetchone() == (1,)
    assert connection.execute("SELECT last_sequence FROM daily_sequences WHERE ticket_date=?", (DAY,)).fetchone()[0] >= 2
    if schema >= 14:
        assert connection.execute("SELECT count(*) FROM queue_number_allocations WHERE task_id IN (?,?)", (ID,ID+1)).fetchone() == (2,)
    marker = connection.execute("SELECT value FROM settings WHERE key=?", (key,)).fetchone()
    if mode in ("sentinel", "preserved"):
        assert marker == ("data created after upgrade",)
    elif mode in ("verify", "seed"):
        assert marker is None
    flags = dict(integrity="ok", foreignKeyViolations=0, ids=[ID, ID+1], schema=schema, postUpgrade=marker is not None)
    print(json.dumps(flags))
connection.close()
