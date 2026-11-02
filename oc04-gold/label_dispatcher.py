#!/usr/bin/env python3
"""label_dispatcher.py — OC-04 pilot blind labeling via two frontier LLMs.

Annotator B = Codex (gpt-5.6-sol via codex CLI, read-only sandbox).
Annotator C = DeepSeek frontier model (deepseek-chat via API).
Founder = annotator A (human, separate).

Each model receives: codebook.md + the annot_B packet lines (blind — no
arm/rank/scores) and must output, per event: label ∈ {required, supporting,
dead_end, irrelevant, uncertain} + a 3-8 word reason (R-9 compliant:
no task content). Outputs land in packets/labels_{B_codex,C_deepseek}.jsonl.

Batches: one session (jsonl) per request to keep context manageable.
"""
import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO = Path("/home/cosmo/contextmesh-rs")
PKT = REPO / "oc04-gold" / "packets"
CODEBOOK = (REPO / "oc04-gold" / "codebook.md").read_text()

LABELS = ("required", "supporting", "dead_end", "irrelevant", "uncertain")

INSTRUCTIONS = f"""You are annotator in a blind gold-labeling study (OC-04 4G+).
You will label transcript events of AI-agent sessions. Apply this codebook
EXACTLY, decision tree first-match-wins:

{CODEBOOK}

For EACH input line (JSON with event_index, role, tool_name, text), output
one JSON line: {{"event_index": <int>, "label": "<one of {LABELS}>",
"reason_code": "<3-8 words, NO task content (rule R-9)>"}}
Output ONLY the JSON lines, in input order, no markdown fences.
The session's stated task: use the FIRST user message in the provided batch
as the task; label each event by whether the session needed it to reach the
correct outcome (B5 standard in the codebook).
"""


def call_codex(session_file: Path, out_file: Path) -> bool:
    lines = session_file.read_text().strip()
    prompt = INSTRUCTIONS + "\nINPUT EVENTS:\n" + lines
    # prompt via stdin — codebook makes argv exceed ARG_MAX
    r = subprocess.run(
        ["codex", "exec", "-s", "read-only", "--skip-git-repo-check",
         "-m", "gpt-5.6-sol", "-"],
        input=prompt, capture_output=True, text=True, timeout=1200,
    )
    out = r.stdout
    if r.returncode != 0:
        print(f"  codex FAIL rc={r.returncode}: {r.stderr[-300:]}", flush=True)
        return False
    parsed = parse_labels(out)
    if parsed is None:
        print("  codex: no parseable labels", flush=True)
        return False
    out_file.write_text("\n".join(json.dumps(p) for p in parsed) + "\n")
    return True


def parse_labels(text: str):
    rows = []
    for line in text.splitlines():
        line = line.strip().strip("`")
        if not line.startswith("{"):
            continue
        try:
            obj = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(obj, dict) and obj.get("label") in LABELS and "event_index" in obj:
            rows.append({
                "event_index": obj["event_index"],
                "label": obj["label"],
                "reason_code": str(obj.get("reason_code", ""))[:80],
            })
    return rows or None


def call_deepseek(session_file: Path, out_file: Path) -> bool:
    from urllib import request
    key = None
    env_path = Path.home() / ".hermes" / ".env"
    for line in env_path.read_text().splitlines():
        if line.startswith("DEEPSEEK_API_KEY="):
            key = line.split("=", 1)[1].strip()
    if not key:
        print("  deepseek: no API key", flush=True)
        return False
    events = [json.loads(l) for l in session_file.read_text().strip().splitlines()]
    payload = json.dumps({
        "model": "deepseek-chat",
        "messages": [
            {"role": "system", "content": INSTRUCTIONS},
            {"role": "user", "content": "\n".join(json.dumps(e) for e in events)},
        ],
        "temperature": 0,
    }).encode()
    req = request.Request(
        "https://api.deepseek.com/chat/completions", data=payload,
        headers={"Content-Type": "application/json",
                 "Authorization": f"Bearer {key}"})
    try:
        with request.urlopen(req, timeout=1200) as resp:
            body = json.loads(resp.read())
        text = body["choices"][0]["message"]["content"]
    except Exception as e:  # noqa: BLE001
        print(f"  deepseek FAIL: {e}", flush=True)
        return False
    parsed = parse_labels(text)
    if parsed is None:
        print("  deepseek: no parseable labels", flush=True)
        return False
    out_file.write_text("\n".join(json.dumps(p) for p in parsed) + "\n")
    return True


def main():
    annotator, engine = sys.argv[1], {"B": call_codex, "C": call_deepseek}[sys.argv[2]]
    outdir = PKT / f"labels_{annotator}"
    outdir.mkdir(exist_ok=True)
    sessions = sorted((PKT / "annot_B").glob("*.jsonl"))
    ok = 0
    for i, sf in enumerate(sessions, 1):
        of = outdir / sf.name
        if of.exists() and of.read_text().strip():
            ok += 1
            continue
        print(f"[{annotator} {i}/{len(sessions)}] {sf.name[:12]}…", flush=True)
        if engine(sf, of):
            ok += 1
        time.sleep(1)
    print(f"{annotator}: {ok}/{len(sessions)} sessions labeled")


if __name__ == "__main__":
    main()
