#!/usr/bin/env python3
"""Render the four paper figures from results.json.

Style is tuned to match the LaTeX paper:
  - serif font (Times via mathtext fallback if the TeX install is missing)
  - clean white background
  - light gray horizontal gridlines only
  - line width 1.5, markers at data points (unfilled)
  - legend inside the plot
  - 300 DPI PDF
"""
import json
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parent
RESULTS = ROOT / "results.json"
FIGURES = ROOT.parent / "figures"
FIGURES.mkdir(exist_ok=True)


def apply_style():
    plt.rcParams.update({
        "font.family": "serif",
        "font.serif": ["Times New Roman", "Times", "DejaVu Serif"],
        "mathtext.fontset": "stix",
        "axes.facecolor": "white",
        "figure.facecolor": "white",
        "axes.edgecolor": "#222222",
        "axes.linewidth": 0.8,
        "axes.grid": True,
        "axes.grid.axis": "y",
        "grid.color": "#cccccc",
        "grid.linestyle": "-",
        "grid.linewidth": 0.5,
        "axes.axisbelow": True,
        "xtick.direction": "out",
        "ytick.direction": "out",
        "xtick.major.size": 4,
        "ytick.major.size": 4,
        "xtick.color": "#222222",
        "ytick.color": "#222222",
        "legend.frameon": True,
        "legend.framealpha": 1.0,
        "legend.edgecolor": "#888888",
        "legend.fontsize": 9,
        "axes.titlesize": 11,
        "axes.labelsize": 10,
        "savefig.dpi": 300,
        "pdf.fonttype": 42,
    })


COLOURS = ["#0b5d8a", "#c8553d", "#3f8a4d", "#7a4ea8", "#cc8800", "#404040"]
MARKERS = ["o", "s", "D", "^", "v", "P"]


def load() -> dict:
    with RESULTS.open() as fh:
        return json.load(fh)


def figure_1(results: dict) -> None:
    runs = results["experiment_1"]
    extended = results.get("experiment_1_extended", [])
    extended_by_label = {r["label"]: r for r in extended}
    fig, ax = plt.subplots(figsize=(5.5, 3.6))
    for i, run in enumerate(runs):
        # Primary: the spec'd 1000-iter run, in full colour with markers.
        xs = [p["iteration"] for p in run["points"]]
        ys = [p["exploitability"] for p in run["points"]]
        ax.plot(
            xs,
            ys,
            label=run["label"],
            color=COLOURS[i],
            marker=MARKERS[i],
            markersize=4,
            markerfacecolor="none",
            markeredgewidth=1.2,
            linewidth=1.5,
        )
        # Faded continuation: the 3000-iter extension after iteration 1000,
        # plotted unlabeled to show asymptotic behaviour without cluttering
        # the legend.
        ext = extended_by_label.get(run["label"])
        if ext is not None:
            ext_xs = [p["iteration"] for p in ext["points"] if p["iteration"] > 1000]
            ext_ys = [p["exploitability"] for p in ext["points"] if p["iteration"] > 1000]
            if ext_xs:
                ax.plot(
                    ext_xs,
                    ext_ys,
                    color=COLOURS[i],
                    linewidth=1.0,
                    linestyle=(0, (4, 2)),
                    alpha=0.55,
                )
    ax.set_yscale("log")
    ax.set_xlabel("Iteration $t$")
    ax.set_ylabel("Exploitability (bb)")
    ax.set_title("CFR vs. CFR$^+$ on the K=50, 3-size HUNL flop subgame")
    if extended:
        ax.axvline(1000, color="#999999", linewidth=0.6, linestyle=":")
        ax.text(
            1010,
            ax.get_ylim()[0] * 1.4 if ax.get_ylim()[0] > 0 else 1e-2,
            "spec'd budget $T{=}1000$",
            fontsize=7,
            color="#666666",
            rotation=90,
            verticalalignment="bottom",
        )
    ax.legend(loc="upper right")
    fig.tight_layout()
    out = FIGURES / "fig1_cfr_vs_cfrplus.pdf"
    fig.savefig(out)
    plt.close(fig)
    print(f"wrote {out}")


def figure_2(results: dict) -> None:
    runs = results["experiment_2"]
    fig, ax = plt.subplots(figsize=(5.2, 3.4))
    for i, run in enumerate(runs):
        xs = [p["iteration"] for p in run["points"]]
        ys = [p["exploitability"] for p in run["points"]]
        ax.plot(
            xs,
            ys,
            label=run["label"],
            color=COLOURS[i],
            marker=MARKERS[i],
            markersize=4,
            markerfacecolor="none",
            markeredgewidth=1.2,
            linewidth=1.5,
        )
    ax.set_yscale("log")
    ax.set_xlabel("Iteration $t$")
    ax.set_ylabel("Exploitability (bb)")
    ax.set_title("CFR$^+$ convergence by card-bucket granularity")
    ax.legend(loc="upper right", ncol=2)
    fig.tight_layout()
    out = FIGURES / "fig2_convergence_by_bucket.pdf"
    fig.savefig(out)
    plt.close(fig)
    print(f"wrote {out}")


def figure_3(results: dict) -> None:
    points = results["experiment_3"]
    # Group by board.
    boards = {}
    for p in points:
        boards.setdefault(p["board"], []).append(p)
    fig, ax = plt.subplots(figsize=(5.2, 3.4))
    for i, (board, pts) in enumerate(sorted(boards.items())):
        pts.sort(key=lambda x: x["card_buckets"])
        xs = [p["card_buckets"] for p in pts]
        ys = [p["abstraction_error"] for p in pts]
        ax.plot(
            xs,
            ys,
            label=board,
            color=COLOURS[i],
            marker=MARKERS[i],
            markersize=5,
            markerfacecolor="none",
            markeredgewidth=1.2,
            linewidth=1.5,
        )
    ax.set_xscale("log")
    ax.set_xlabel("Card buckets $K$")
    ax.set_ylabel("Abstraction error (bb)")
    ax.set_title("Card-abstraction error vs. $K$ (K=200 proxy)")
    ax.legend(loc="upper right", title="Board texture")
    fig.tight_layout()
    out = FIGURES / "fig3_abstraction_error.pdf"
    fig.savefig(out)
    plt.close(fig)
    print(f"wrote {out}")


def figure_4(results: dict) -> None:
    points = results["experiment_4"]
    fig, ax = plt.subplots(figsize=(5.2, 3.4))
    xs = [p["num_sizes"] for p in points]
    ys = [p["abstraction_error"] for p in points]
    labels = [", ".join(p["bet_sizes"]) for p in points]
    ax.plot(
        xs,
        ys,
        color=COLOURS[0],
        marker=MARKERS[0],
        markersize=5,
        markerfacecolor="none",
        markeredgewidth=1.2,
        linewidth=1.5,
    )
    for x, y, lab in zip(xs, ys, labels):
        ax.annotate(
            f"$\\{{{lab}\\}}$",
            xy=(x, y),
            xytext=(6, 4),
            textcoords="offset points",
            fontsize=7,
            color="#333333",
        )
    ax.set_xlabel("Number of pot-fraction bet sizes $|S|$")
    ax.set_ylabel("Abstraction error (bb)")
    ax.set_title("Action-abstraction error vs. bet-size set (K=50)")
    ax.set_xticks(sorted(set(xs)))
    ax.set_xlim(0.6, max(xs) + 0.6)
    fig.tight_layout()
    out = FIGURES / "fig4_action_abstraction_error.pdf"
    fig.savefig(out)
    plt.close(fig)
    print(f"wrote {out}")


def main() -> None:
    apply_style()
    results = load()
    if results.get("experiment_1"):
        figure_1(results)
    if results.get("experiment_2"):
        figure_2(results)
    if results.get("experiment_3"):
        figure_3(results)
    if results.get("experiment_4"):
        figure_4(results)


if __name__ == "__main__":
    main()
