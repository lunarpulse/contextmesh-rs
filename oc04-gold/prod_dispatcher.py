#!/usr/bin/env python3
"""prod_dispatcher.py — sharded production labeling (frozen prompt/parser).
Usage: prod_dispatcher.py <engine codex5.6|codex6.1|deepseek> <outdir-suffix> <worker_i> <worker_n>
Reads packets_prod/annot_A/*.jsonl (candidates only, blind), skips existing outputs."""
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, "/home/cosmo/contextmesh-rs/oc04-gold")
from label_dispatcher import INSTRUCTIONS, parse_labels, call_deepseek  # noqa: E402

REPO = Path("/home/cosmo/contextmesh-rs")
PKT = REPO / "oc04-gold" / "packets_prod"

engine, suffix, wi, wn = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
OUT = PKT / f"labels_{suffix}"
OUT.mkdir(exist_ok=True)

sessions = sorted((PKT / "annot_A").glob("*.jsonl"))
mine = sessions[wi::wn]
print(f"worker {wi+1}/{wn}: {len(mine)} sessions -> {OUT}", flush=True)

done = 0
for sf in mine:
    out_file = OUT / sf.name
    if out_file.exists() and out_file.read_text().strip():
        done += 1
        continue
    lines = sf.read_text().strip()
    n_in = len(lines.splitlines())
    ok = False
    if engine.startswith("codex"):
        model = "gpt-5.6-sol" if engine == "codex5.6" else "gpt-6.1-sol"
        r = subprocess.run(
            ["codex", "exec", "-s", "read-only", "--skip-git-repo-check",
             "-m", model, "-"],
            input=INSTRUCTIONS + "\nINPUT EVENTS:\n" + lines,
            capture_output=True, text=True, timeout=1200,
        )
        if r.returncode == 0:
            parsed = parse_labels(r.stdout)
            if parsed and len(parsed) == n_in:
                out_file.write_text("\n".join(json.dumps(p) for p in parsed) + "\n")
                ok = True
            else:
                print(f"  {sf.name[:12]}: parse {0 if not parsed else len(parsed)}/{n_in}", flush=True)
        else:
            print(f"  {sf.name[:12]}: rc={r.returncode} {r.stderr[-150:]}", flush=True)
    elif engine == "deepseek":
        ok = call_deepseek(sf, out_file)
        if ok:
            got = len(out_file.read_text().strip().splitlines())
            if got != n_in:
                out_file.unlink()
                ok = False
                print(f"  {sf.name[:12]}: count {got}/{n_in}", flush=True)
    if ok:
        done += 1
        print(f"  ok {sf.name[:12]} ({done}/{len(mine)})", flush=True)

total = len(list(OUT.glob("*.jsonl")))
print(f"WORKER {wi+1}/{wn} DONE: {done}/{len(mine)} this shard; dir total {total}", flush=True)
