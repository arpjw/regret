#!/usr/bin/env python3
"""Merge per-experiment JSON files into a single results.json.

`exp1.json` holds the spec'd 1000-iter run; `exp1_long.json` holds the
3000-iter follow-up used in the prose/footnote to discuss asymptotic
trajectory beyond the spec's budget.
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SOURCES = [
    # (file, source_key_in_file, output_key)
    ("results.json", "experiment_1", "experiment_1"),
    ("exp1.json",    "experiment_1", "experiment_1"),
    ("exp2.json",    "experiment_2", "experiment_2"),
    ("exp3.json",    "experiment_3", "experiment_3"),
    ("exp4.json",    "experiment_4", "experiment_4"),
    ("exp1_long.json", "experiment_1", "experiment_1_extended"),
]
OUT = ROOT / "results.json"


def main() -> int:
    merged: dict = {}
    for fname, src_key, out_key in SOURCES:
        path = ROOT / fname
        if not path.exists():
            print(f"info: {path} missing — skipping", file=sys.stderr)
            continue
        try:
            with path.open() as fh:
                data = json.load(fh)
        except json.JSONDecodeError:
            print(f"warning: {path} is not valid JSON — skipping", file=sys.stderr)
            continue
        if data.get(src_key) is not None:
            merged[out_key] = data[src_key]
    with OUT.open("w") as fh:
        json.dump(merged, fh, indent=2)
    print(f"wrote {OUT} with keys: {list(merged.keys())}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
