#!/usr/bin/env python3
"""verify_labels.py — integrity check on the delegated label files."""
import json
from collections import Counter
from pathlib import Path

PKT = Path("/home/cosmo/contextmesh-rs/oc04-gold/packets")
LABELS = {"required", "supporting", "dead_end", "irrelevant", "uncertain"}

for annot in ("labels_B", "labels_C"):
    d = PKT / annot
    total = 0
    dist = Counter()
    for f in sorted(d.glob("*.jsonl")):
        ids = []
        for line in f.read_text().strip().splitlines():
            obj = json.loads(line)
            assert obj["label"] in LABELS, f"{annot}/{f.name}: bad label {obj['label']}"
            ids.append(obj["event_index"])
            dist[obj["label"]] += 1
            total += 1
        # packet count must match
        pkt = len((PKT / "annot_B" / f.name).read_text().strip().splitlines())
        assert len(ids) == pkt, f"{annot}/{f.name}: {len(ids)} != packet {pkt}"
        assert len(ids) == len(set(ids)), f"{annot}/{f.name}: dup event_index"
    print(f"{annot}: {total} labels, distribution: {dict(dist)}")
print("integrity OK")
