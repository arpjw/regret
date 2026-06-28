#!/usr/bin/env python3
"""Extract paper-ready summary numbers from results.json.

Prints a Markdown-style report and exposes a `summary()` helper used by the
LaTeX prose-update script.
"""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
RESULTS = ROOT / "results.json"


def first_iter_below(points: list[dict], threshold: float) -> int | None:
    """Smallest iteration whose exploitability is <= threshold, else None."""
    for p in points:
        if p["exploitability"] <= threshold:
            return int(p["iteration"])
    return None


def summary() -> dict:
    with RESULTS.open() as fh:
        data = json.load(fh)
    out: dict = {}

    if data.get("experiment_1"):
        runs = {r["label"]: r for r in data["experiment_1"]}
        cfg = next(iter(runs.values()))["config"]
        starting_pot = cfg["starting_pot"]
        # 5% / 10% of pot thresholds
        t5 = 0.05 * starting_pot
        t10 = 0.10 * starting_pot
        ex1 = {
            "starting_pot": starting_pot,
            "starting_stack": cfg["starting_stack"],
            "card_buckets": cfg["card_buckets"],
            "iters_total": runs["CFR+"]["points"][-1]["iteration"],
            "final": {
                label: runs[label]["points"][-1]["exploitability"]
                for label in runs
            },
            "iter_to_5pct": {
                label: first_iter_below(runs[label]["points"], t5)
                for label in runs
            },
            "iter_to_10pct": {
                label: first_iter_below(runs[label]["points"], t10)
                for label in runs
            },
            "threshold_5pct_bb": t5,
            "threshold_10pct_bb": t10,
        }
        out["experiment_1"] = ex1

    if data.get("experiment_2"):
        runs = data["experiment_2"]
        cfg = runs[0]["config"]
        starting_pot = cfg["starting_pot"]
        ex2 = {
            "starting_pot": starting_pot,
            "per_K": [
                {
                    "K": r["config"]["card_buckets"],
                    "final_expl_bb": r["points"][-1]["exploitability"],
                    "final_expl_pct_pot": r["points"][-1]["exploitability"] / starting_pot * 100,
                    "iters": r["points"][-1]["iteration"],
                }
                for r in runs
            ],
        }
        out["experiment_2"] = ex2

    if data.get("experiment_3"):
        pts = data["experiment_3"]
        ex3 = {
            "by_board_K": [
                {
                    "board": p["board"],
                    "K": p["card_buckets"],
                    "iters": p["iterations"],
                    "abs_expl": p["abs_exploitability"],
                    "proxy_expl": p["proxy_exploitability"],
                    "abs_error": p["abstraction_error"],
                }
                for p in pts
            ],
        }
        out["experiment_3"] = ex3

    if data.get("experiment_4"):
        pts = data["experiment_4"]
        ex4 = {
            "by_size_set": [
                {
                    "bet_sizes": p["bet_sizes"],
                    "num_sizes": p["num_sizes"],
                    "iters": p["iterations"],
                    "abs_expl": p["abs_exploitability"],
                    "proxy_expl": p["proxy_exploitability"],
                    "abs_error": p["abstraction_error"],
                }
                for p in pts
            ],
        }
        out["experiment_4"] = ex4

    return out


def pretty(s: dict) -> str:
    return json.dumps(s, indent=2)


if __name__ == "__main__":
    print(pretty(summary()))
