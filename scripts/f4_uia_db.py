import sqlite3
import sys
from pathlib import Path

db = sqlite3.connect(Path(sys.argv[1]).resolve().as_uri() + "?mode=ro", uri=True)
assert db.execute("PRAGMA integrity_check").fetchall() == [("ok",)]
assert db.execute("PRAGMA foreign_key_check").fetchall() == []
rows = db.execute("select title, is_import_conflict, internal_notes from tasks order by id").fetchall()
assert rows == [
    ("F4 source", 0, "local edited note"),
    ("F4 source（冲突）", 0, ""),
    ("F4 source（冲突 2）", 1, "original source note"),
], rows
print("F4_UIA SQLite integrity=ok fk=0 tasks=3 independent-conflicts=1")
