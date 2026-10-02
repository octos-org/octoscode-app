#!/usr/bin/env python3
"""A10 — flip parity rows by key (never a hand merge): read
docs/parity-matrix.csv with csv, set `phase4_bucket_manual` /
`phase4_evidence_manual` on the rows a JSON spec names, write it back with
the same dialect, and verify 361 rows with (feature, group, capability)
unique.

usage: a10_parity.py <spec.json>
spec: [{"feature": "...", "capability_prefix": "...", "bucket": "A", "evidence": "..."}]
`--count` prints the final-bucket totals (all rows and the A10 features).
"""
import csv
import json
import pathlib
import sys
from collections import Counter

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
CSV = ROOT / "docs" / "parity-matrix.csv"
A10 = ["control", "peers", "fleet", "product-controls", "autonomy", "skills", "models", "context",
       "review", "research", "btw"]


def final(r):
    return r["phase4_bucket_manual"] or r["phase4_bucket"]


def load():
    with open(CSV, newline="") as f:
        rd = csv.DictReader(f)
        return rd.fieldnames, list(rd)


def save(fields, rows):
    with open(CSV, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fields, lineterminator="\r\n")
        w.writeheader()
        w.writerows(rows)


def check(rows):
    assert len(rows) == 361, len(rows)
    keys = [(r["feature"], r["group"], r["capability"]) for r in rows]
    assert len(set(keys)) == len(keys), "duplicate (feature, group, capability)"


def counts(rows):
    allc = Counter(final(r) for r in rows)
    mine = Counter(final(r) for r in rows if r["feature"] in A10)
    return dict(allc), dict(mine)


if __name__ == "__main__":
    fields, rows = load()
    if sys.argv[1] == "--count":
        print(counts(rows))
        sys.exit(0)
    spec = json.load(open(sys.argv[1]))
    for s in spec:
        hits = [r for r in rows if r["feature"] == s["feature"] and r["capability"].startswith(s["capability_prefix"])]
        assert len(hits) == 1, f"{s['feature']} / {s['capability_prefix']!r}: {len(hits)} rows"
        r = hits[0]
        r["phase4_bucket_manual"] = s["bucket"]
        r["phase4_evidence_manual"] = s["evidence"]
        print(f"{s['bucket']}  [{r['feature']}] {r['capability'][:90]}")
    check(rows)
    save(fields, rows)
    print("totals", counts(rows))
