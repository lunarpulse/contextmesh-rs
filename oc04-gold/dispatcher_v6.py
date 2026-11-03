#!/usr/bin/env python3
"""dispatcher_v6.py — OC-04 protocol v6 re-labeling (codebook v2.0, basis field).

Usage: dispatcher_v6.py <engine glm|codex6.1|deepseek> <suffix> <worker_i> <worker_n>
Reads packets_prod/annot_A/*.jsonl (unchanged packet universe, 192 sessions),
writes packets_prod/labels_v6_<suffix>/. Skips existing outputs (resumable).

Output line: {"event_index":i,"label":"...","basis":"5-15 words, R-9"}
"""
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, "/home/cosmo/contextmesh-rs/oc04-gold")
from label_dispatcher import call_deepseek  # noqa: E402  (deepseek transport only)

REPO = Path("/home/cosmo/contextmesh-rs")
PKT = REPO / "oc04-gold" / "packets_prod"
CODEBOOK_V2 = (REPO / "oc04-gold" / "codebook-v2.md").read_text()
LABELS = ("required", "supporting", "dead_end", "irrelevant", "uncertain")

INSTRUCTIONS = f"""You are annotator in a blind gold-labeling study (OC-04 v6).
Apply this codebook EXACTLY, decision tree first-match-wins:

{CODEBOOK_V2}

For EACH input line (JSON with event_index, role, tool_name, text), output
one JSON line: {{"event_index": <int>, "label": "<one of {LABELS}>",
"basis": "<rules>"}}
basis rules: required → "TYPE: phrase" (5-15 words total) where TYPE is one
of answer_text|file_output|command_result|verification|unblock and the
phrase says what later line consumed it; supporting/dead_end/irrelevant →
5-15 words stating the consumer-test outcome; uncertain → exactly one of
insufficient-context|ambiguous-role|ambiguous-consumer. NO task content
(rule R-9) — never copy wording from the events.
Output ONLY the JSON lines, in input order, no markdown fences.
`uncertain` is per-event, a last resort — never mark a whole session
uncertain. The session's stated task: use the FIRST user message in the
provided batch; label each event by whether the session needed it to reach
the correct outcome (B5 standard).
"""

DEP_TYPES = ("answer_text", "file_output", "command_result",
             "verification", "unblock")
UNCERTAIN_BASIS = ("insufficient-context", "ambiguous-role",
                   "ambiguous-consumer")


def parse_v6(out: str, n_in: int):
    rows = []
    for line in out.splitlines():
        line = line.strip().strip("`")
        if not line.startswith("{"):
            continue
        try:
            r = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not isinstance(r.get("event_index"), int):
            continue
        if r.get("label") not in LABELS:
            continue
        b = str(r.get("basis", "")).strip()
        nw = len(b.split())
        if r["label"] == "uncertain":
            if b not in UNCERTAIN_BASIS:
                continue
        elif not (5 <= nw <= 15):
            continue
        elif r["label"] == "required":
            if not b.startswith(DEP_TYPES) or ":" not in b:
                continue
        rows.append({"event_index": r["event_index"], "label": r["label"],
                     "basis": b})
    if len(rows) != n_in:
        return None
    return rows


def run_codex6(model, lines, n_in):
    r = subprocess.run(
        ["codex", "exec", "-s", "read-only", "--skip-git-repo-check",
         "-m", model, "-"],
        input=INSTRUCTIONS + "\nINPUT EVENTS:\n" + lines,
        capture_output=True, text=True, timeout=1200,
    )
    if r.returncode != 0:
        return None
    return parse_v6(r.stdout, n_in)


def run_glm(lines, n_in):
    r = subprocess.run(
        [sys.executable,
         "/home/cosmo/.hermes/cache/scratch/glm_call.py"],
        input=INSTRUCTIONS + "\nINPUT EVENTS:\n" + lines,
        capture_output=True, text=True, timeout=900)
    if r.returncode != 0:
        return None
    return parse_v6(r.stdout, n_in)


def run_deepseek(lines, n_in):
    # ds_call.py (validated in smoke) — the former inline `python3 -c`
    # payload had a SyntaxError (`}).\n    encode()`), which is why the
    # first C6 dispatch pass returned 0/192.
    r = subprocess.run(
        [sys.executable, "/home/cosmo/.hermes/cache/scratch/ds_call.py"],
        input=INSTRUCTIONS + "\nINPUT EVENTS:\n" + lines,
        capture_output=True, text=True, timeout=900,
        env=None)
    if r.returncode != 0:
        return None
    return parse_v6(r.stdout, n_in)


RUNNERS = {"codex6.1": lambda l, n: run_codex6("gpt-6.1-sol", l, n),
           "glm": run_glm, "deepseek": run_deepseek}

# transport-level retry: identical prompt, up to ATTEMPTS calls, for
# transient row-drops observed in the GLM smoke (session 020329a0, 1st call
# dropped one row; identical retry returned 13/13).
ATTEMPTS = 3


def main():
    engine, suffix, wi, wn = (sys.argv[1], sys.argv[2],
                              int(sys.argv[3]), int(sys.argv[4]))
    if engine not in RUNNERS:
        sys.exit(f"unknown engine {engine}")
    runner = RUNNERS[engine]
    OUT = PKT / f"labels_v6_{suffix}"
    OUT.mkdir(exist_ok=True)
    sessions = sorted((PKT / "annot_A").glob("*.jsonl"))
    mine = sessions[wi::wn]
    print(f"v6 worker {wi+1}/{wn}: {len(mine)} sessions -> {OUT}", flush=True)
    done = 0
    for sf in mine:
        out_file = OUT / sf.name
        if out_file.exists() and out_file.read_text().strip():
            done += 1
            continue
        lines = sf.read_text().strip()
        n_in = len(lines.splitlines())
        parsed = None
        for attempt in range(1, ATTEMPTS + 1):
            try:
                parsed = runner(lines, n_in)
            except subprocess.TimeoutExpired:
                parsed = None
            if parsed:
                if attempt > 1:
                    print(f"  {sf.name[:12]}: ok on attempt {attempt}",
                          flush=True)
                break
        if parsed:
            out_file.write_text(
                "\n".join(json.dumps(p) for p in parsed) + "\n")
            done += 1
            print(f"  ok {sf.name[:12]} ({done}/{len(mine)})", flush=True)
        else:
            print(f"  FAIL {sf.name[:12]} after {ATTEMPTS} attempts",
                  flush=True)
    total = len(list(OUT.glob("*.jsonl")))
    print(f"V6 WORKER {wi+1}/{wn} DONE: {done}/{len(mine)}; "
          f"dir total {total}", flush=True)


if __name__ == "__main__":
    main()
