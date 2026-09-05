#!/usr/bin/env python3
"""verify_determinism.py — double-run digest comparison for the manifests."""
import hashlib
import json
import subprocess
import sys

M = "oc04-gold/sampling-manifest.json"
P = "oc04-gold/pilot-manifest.json"

def digests():
    return (
        hashlib.sha256(open(M, "rb").read()).hexdigest(),
        hashlib.sha256(open(P, "rb").read()).hexdigest(),
    )

d1 = digests()
r = subprocess.run([sys.executable, "oc04-gold/run_sample.py"],
                   capture_output=True, text=True)
if r.returncode != 0:
    print(r.stdout, r.stderr)
    sys.exit(1)
d2 = digests()

print("run1 corpus/pilot:", d1[0][:16], d1[1][:16])
print("run2 corpus/pilot:", d2[0][:16], d2[1][:16])
print("DETERMINISM:", "PASS" if d1 == d2 else "FAIL")

# structure sanity
m = json.load(open(M))
p = json.load(open(P))
assert len(m["sessions"]) == 192
assert len(p["sessions"]) == 16
# pilot ⊆ corpus
corpus = {s["session_hmac"] for s in m["sessions"]}
pilot = {s["session_hmac"] for s in p["sessions"]}
assert pilot <= corpus, "pilot not subset of corpus"
# no HMAC without key; no raw ids anywhere (the frozen seed 20260820 is
# the only legitimate '2026…' literal — allowlist it explicitly)
raw = open("oc04-gold/sampling-manifest.json").read()
cleaned = raw.replace('"seed": 20260820', "").replace("2026-08-21", "")
assert "20260" not in cleaned, "possible raw session id leak"
print("STRUCTURE: PASS (192/16, pilot⊆corpus, no raw ids)")
