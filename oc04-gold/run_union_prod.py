#!/usr/bin/env python3
"""run_union_prod.py — LOCAL-ONLY production union runner (192 sessions).
Same frozen pipeline as run_union.py; reads sampling-manifest.json (v5.4)."""
import hashlib
import hmac
import json
import os
import sqlite3
import subprocess

REPO = "/home/cosmo/contextmesh-rs"
KEY_HEX = open(f"{REPO}/.oc04-hmac-key").read().strip()
KEY = bytes.fromhex(KEY_HEX)
SESSION_DOMAIN = b"oc04-gold/session/v1"

db = sqlite3.connect("file:/home/cosmo/.hermes/state.db?mode=ro", uri=True)
hmac_to_raw = {
    hmac.new(KEY, SESSION_DOMAIN + rid.encode(), hashlib.sha256).hexdigest(): rid
    for (rid,) in db.execute("SELECT id FROM sessions")
}
man = json.load(open(f"{REPO}/oc04-gold/sampling-manifest.json"))
assert man["corpus_size"] == len(man["sessions"]) == 192
raw_session_ids = []
for s in man["sessions"]:
    rid = hmac_to_raw.get(s["session_hmac"])
    assert rid, f"manifest HMAC not mappable — key drift ({s['session_hmac'][:12]})"
    raw_session_ids.append(rid)
print(f"resolved {len(raw_session_ids)}/192 raw session ids")

env = dict(os.environ)
env.update({
    "OC04_PILOT_DB": "/home/cosmo/.hermes/state.db",
    "OC04_PILOT_IDS": json.dumps(raw_session_ids),
    "OC04_PILOT_KEY": KEY_HEX,
    "OC04_UNION_OUT": f"{REPO}/oc04-gold/production-union-manifest.json",
})
r = subprocess.run(
    ["cargo", "test", "-p", "contextmesh-salience", "--test", "oc04_pilot_union",
     "--", "--nocapture", "--test-threads=1"],
    cwd=REPO, env=env, capture_output=True, text=True,
)
tail = "\n".join((r.stdout + r.stderr).splitlines()[-4:])
print(tail)
