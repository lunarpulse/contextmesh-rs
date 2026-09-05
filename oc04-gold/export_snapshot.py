#!/usr/bin/env python3
"""export_snapshot.py — LOCAL-ONLY pre-label state export for sample.py.

Reads the live session DB and emits a redacted state JSON containing ONLY
pre-label aggregate fields (no title, no content, no paths):
  id (raw — HMAC happens inside sample.py, key stays local),
  started_at, terminated, input_tokens, message_count, parent_session_id.
The output file stays in the repo-ignored private/ directory.
"""
import json
import sqlite3

DB = "/home/cosmo/.hermes/state.db"
OUT = "_bmad-output/implementation-artifacts/oc-real-data-replay/private/oc04-frame-snapshot.json"

db = sqlite3.connect(f"file:{DB}?mode=ro", uri=True)
rows = db.execute(
    "SELECT id, started_at, ended_at, input_tokens, message_count, parent_session_id FROM sessions"
).fetchall()

sessions = [
    {
        "id": r[0],
        "started_at": float(r[1] or 0.0),
        "terminated": r[2] is not None,
        "input_tokens": int(r[3] or 0),
        "message_count": int(r[4] or 0),
        "parent_session_id": r[5],
    }
    for r in rows
]

doc = {"cutoff_epoch": 1787280000.0, "sessions": sessions}
with open(OUT, "w", encoding="utf-8") as fh:
    json.dump(doc, fh)
print(f"exported {len(sessions)} sessions -> {OUT}")
