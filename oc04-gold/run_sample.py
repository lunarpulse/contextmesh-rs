#!/usr/bin/env python3
"""run_sample.py — LOCAL-ONLY runner: HMAC key gen + corpus/pilot manifests."""
import json
import os
import secrets
import stat
import subprocess
import sys

REPO = "/home/cosmo/contextmesh-rs"
KEY_FILE = os.path.join(REPO, ".oc04-hmac-key")
SNAP = os.path.join(
    REPO,
    "_bmad-output/implementation-artifacts/oc-real-data-replay/private/oc04-frame-snapshot.json",
)

# 1) key (local-only, gitignored)
if not os.path.exists(KEY_FILE):
    with open(KEY_FILE, "w") as f:
        f.write(secrets.token_hex(32))
    os.chmod(KEY_FILE, stat.S_IRUSR | stat.S_IWUSR)
    # ensure gitignore entry
    gi = os.path.join(REPO, ".gitignore")
    with open(gi) as f:
        lines = f.read().splitlines()
    if ".oc04-hmac-key" not in lines:
        with open(gi, "a") as f:
            f.write("\n.oc04-hmac-key\n")
    print("HMAC key generated (local-only, gitignored)")
else:
    print("HMAC key exists")

key = open(KEY_FILE).read().strip()

# 2) manifests (deterministic given seed+snapshot; key affects HMAC ids only)
cmd = [
    sys.executable, os.path.join(REPO, "oc04-gold/sample.py"),
    "--snapshot", SNAP,
    "--size", "192", "--pilot", "16", "--seed", "20260820",
    "--key-hex", key,
    "--out", os.path.join(REPO, "oc04-gold/sampling-manifest.json"),
    "--pilot-out", os.path.join(REPO, "oc04-gold/pilot-manifest.json"),
]
r = subprocess.run(cmd, capture_output=True, text=True)
print(r.stdout, r.stderr)
if r.returncode != 0:
    sys.exit(r.returncode)

m = json.load(open(os.path.join(REPO, "oc04-gold/sampling-manifest.json")))
p = json.load(open(os.path.join(REPO, "oc04-gold/pilot-manifest.json")))
print("=== corpus manifest ===")
for k in ("version", "plan_version", "corpus_size", "frame_median_input_tokens",
          "frame_counts", "strata", "strata_families", "oversupply"):
    print(f"{k} = {json.dumps(m[k])}")
print("sessions:", len(m["sessions"]),
      "excluded:", len(m["excluded_sessions"]),
      "alternates:", {k: len(v) for k, v in m["alternates_per_stratum"].items()})
print("=== pilot manifest ===")
print("size:", p["corpus_size"], "strata:", p["strata"], "sessions:", len(p["sessions"]))
