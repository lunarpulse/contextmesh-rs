#!/usr/bin/env python3
"""build_packets.py — blind labeling packets from pilot-union-manifest.json.

Per annotator (A, B) and per session (keyed by session_hmac):
  packets/annot_{A,B}/<session_hmac>.jsonl   candidate events only
      {"event_index": <import_index>, "role":…, "tool_name":…, "text":…}
  packets/coverage/<session_hmac>.jsonl      NON-candidate events (§5.4 probe)
  packets/labels_template/{A,B}.jsonl        rows to fill: session_hmac,
      event_index, label (empty), reason_code (empty)

Annotator A gets sessions sorted by hmac ASC, B DESC (presentation-order
de-alignment). Event text within a session stays in transcript order for
BOTH annotators (labels are per-event; comprehension needs context order).
Arm membership / rank / scores never leave the manifest. Raw ids stay local.
"""
import hashlib
import hmac
import json
import os
import sqlite3

REPO = "/home/cosmo/contextmesh-rs"
KEY = bytes.fromhex(open(f"{REPO}/.oc04-hmac-key").read().strip())
DOMAIN = b"oc04-gold/session/v1"

union = json.load(open(f"{REPO}/oc04-gold/pilot-union-manifest.json"))
db = sqlite3.connect("file:/home/cosmo/.hermes/state.db?mode=ro", uri=True)
raw_ids = [r[0] for r in db.execute("SELECT id FROM sessions")]
hmac_to_raw = {
    hmac.new(KEY, DOMAIN + rid.encode(), hashlib.sha256).hexdigest(): rid
    for rid in raw_ids
}

P = f"{REPO}/oc04-gold/packets"
for sub in ("annot_A", "annot_B", "coverage", "labels_template"):
    os.makedirs(f"{P}/{sub}", exist_ok=True)

sess_order = sorted(union["sessions"], key=lambda s: s["session_hmac"])
templates = {"A": [], "B": []}
total_cand = total_cov = 0

for annot, sessions in (("A", sess_order), ("B", list(reversed(sess_order)))):
    for sess in sessions:
        sh = sess["session_hmac"]
        rid = hmac_to_raw[sh]
        rows = list(db.execute(
            "SELECT role, content, tool_name FROM messages "
            "WHERE session_id = ? AND active = 1 ORDER BY timestamp, id", (rid,)))
        rows = [(r, c, t) for (r, c, t) in rows if c]  # NULL/empty skip (importer rule)

        cand_lines, cov_lines = [], []
        candidate_idx = {e["import_index"] for e in sess["events"]}
        for i, (role, content, tool) in enumerate(rows):
            line = json.dumps(
                {"event_index": i, "role": role, "tool_name": tool, "text": content},
                ensure_ascii=False,
            )
            if i in candidate_idx:
                cand_lines.append(line)
            else:
                cov_lines.append(line)
        with open(f"{P}/annot_{annot}/{sh}.jsonl", "w") as f:
            f.write("\n".join(cand_lines) + "\n")
        with open(f"{P}/coverage/{sh}.jsonl", "w") as f:
            f.write("\n".join(cov_lines) + "\n")
        for line in cand_lines:
            e = json.loads(line)
            templates[annot].append(json.dumps({
                "session_hmac": sh, "event_index": e["event_index"],
                "label": "", "reason_code": "",
            }, ensure_ascii=False))
        if annot == "A":
            total_cand += len(cand_lines)
            total_cov += len(cov_lines)

for annot in ("A", "B"):
    with open(f"{P}/labels_template/{annot}.jsonl", "w") as f:
        f.write("\n".join(templates[annot]) + "\n")

# integrity check: A and B must see IDENTICAL candidate sets
for sess in union["sessions"]:
    sh = sess["session_hmac"]
    a = open(f"{P}/annot_A/{sh}.jsonl").read()
    b = open(f"{P}/annot_B/{sh}.jsonl").read()
    assert a == b, f"annotator packet mismatch {sh}"
    n = len(a.strip().splitlines()) if a.strip() else 0
    assert n == sess["union_count"], f"{sh}: packet {n} != union {sess['union_count']}"

print(f"packets: {len(sess_order)} sessions, candidates={total_cand}, coverage={total_cov}")
print(f"labels: A={len(templates['A'])} B={len(templates['B'])} (must be equal)")
print(f"A/B candidate sets identical: True")
