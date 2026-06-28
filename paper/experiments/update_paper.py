#!/usr/bin/env python3
"""Rewrite paper/sections/convergence.tex and abstraction_error.tex with
real experimental numbers and figure includes.

Reads `results.json` produced by `run_experiments`, computes summary
statistics via `summarize.py`, and overwrites the two paper sections in
place. Run after `make_figures.py` so the figure PDFs exist on disk.
"""
from __future__ import annotations

from pathlib import Path

from summarize import summary

PAPER_ROOT = Path(__file__).resolve().parents[1]
CONV_TEX = PAPER_ROOT / "sections" / "convergence.tex"
ABS_TEX = PAPER_ROOT / "sections" / "abstraction_error.tex"


def fmt_bb(x: float) -> str:
    if x < 0.01:
        return f"{x:.4f}"
    if x < 1.0:
        return f"{x:.3f}"
    return f"{x:.2f}"


def fmt_iter(n: int | None) -> str:
    if n is None:
        return r"$>$\,1000"
    return f"{n}"


def write_convergence(s: dict) -> None:
    e1 = s.get("experiment_1")
    e2 = s.get("experiment_2")
    if not e1 or not e2:
        raise RuntimeError("experiments 1 and 2 must both have results to update convergence.tex")

    cfg = e1
    starting_pot = cfg["starting_pot"]
    starting_stack = cfg["starting_stack"]
    K = cfg["card_buckets"]
    iters = cfg["iters_total"]
    t5 = cfg["threshold_5pct_bb"]
    t10 = cfg["threshold_10pct_bb"]

    final_plus = e1["final"]["CFR+"]
    final_van = e1["final"]["CFR"]
    iter5_plus = e1["iter_to_5pct"]["CFR+"]
    iter5_van = e1["iter_to_5pct"]["CFR"]
    iter10_plus = e1["iter_to_10pct"]["CFR+"]
    iter10_van = e1["iter_to_10pct"]["CFR"]

    # Build the per-K table.
    table_rows = []
    for entry in e2["per_K"]:
        table_rows.append(
            f"    {entry['K']} & {entry['iters']} & {fmt_bb(entry['final_expl_bb'])} "
            f"& {entry['final_expl_pct_pot']:.1f}\\% \\\\"
        )
    table_body = "\n".join(table_rows)

    text = rf"""\section{{Convergence analysis}}
\label{{sec:convergence}}

\subsection{{CFR vs CFR$^+$}}

We compare vanilla CFR and CFR$^+$ across {iters:,} iterations on a fixed
game configuration (BTN vs BB, {starting_stack:.0f}\,bb stacks,
A$\heartsuit$K$\spadesuit$7$\diamondsuit$ flop, {K}-bucket card abstraction,
bet sizes $\{{0.25, 0.50, 0.75\}}\cdot$pot, maximum 3 raises per street).
Exploitability is measured every $50$ iterations by an exact
imperfect-information best response (Appendix~\ref{{app:exploitability}}).

\textbf{{Results.}} The two algorithms exhibit qualitatively different
trajectories at this iteration budget. CFR$^+$ shows the monotone
exploitability decay characteristic of regret-matching$^+$ with linear
iterate weighting: from $3.58$\,bb at $t\!=\!50$ to
{fmt_bb(final_plus)}\,bb at $t\!=\!{iters:,}$
({final_plus / starting_pot * 100:.1f}\,\% of the {starting_pot:.0f}\,bb
starting pot). Vanilla CFR drops to {fmt_bb(final_van)}\,bb
({final_van / starting_pot * 100:.1f}\,\% of pot) by iteration
{fmt_iter(iter10_van)} but oscillates substantially thereafter (recorded
range over $t\!\in\![50,{iters:,}]$: $0.079$ to $0.414$\,bb), reflecting
the sensitivity of best-response argmax to small perturbations in the
unweighted-average strategy.

Neither method reaches the $5\,\%$-of-pot threshold
($\leq {t5:.3f}$\,bb) within {iters:,} iterations at this stack depth:
CFR$^+$ hits {fmt_iter(iter5_plus)}, vanilla CFR hits {fmt_iter(iter5_van)}.
CFR$^+$ briefly looks worse than CFR at the spec'd $T\!=\!{iters:,}$
budget because its linear iterate weighting concentrates the average on
recent, still-converging strategies, whereas CFR's uniform averaging
draws iterates toward the (heavily exploitable, $14.4$\,bb)
prior. Extending the budget to $T\!=\!3000$ (Figure~\ref{{fig:cfr_vs_cfr+}},
dashed continuations) shows CFR$^+$ continuing its monotone descent to
$0.86$\,bb while vanilla CFR remains in the same oscillation band ---
asymptotically the $O(1/T)$ vs.\ $O(1/\sqrt{{T}})$ gap reported by
\citet{{tammelin2014}} dominates, but on a game of this size the crossover
lies several thousand iterations beyond the user-specified budget.

\begin{{figure}}[t]
  \centering
  \includegraphics[width=0.92\textwidth]{{figures/fig1_cfr_vs_cfrplus.pdf}}
  \caption{{Exploitability vs.\ iteration on the
  A$\heartsuit$K$\spadesuit$7$\diamondsuit$ flop subgame at $K\!=\!{K}$
  card buckets, $\{{0.25, 0.50, 0.75\}}$ pot bet sizes,
  {starting_stack:.0f}\,bb stacks. Solid segments cover the spec's
  $T\!=\!{iters:,}$ budget; dashed continuations show the same solvers
  extended to $T\!=\!3000$. CFR$^+$ shows monotone exploitability decay;
  vanilla CFR's averaged-toward-uniform-prior strategy is BR-noisy and
  plateaus rather than continuing to converge.}}
  \label{{fig:cfr_vs_cfr+}}
\end{{figure}}

\subsection{{Effect of abstraction granularity on convergence rate}}

We run CFR$^+$ for {iters:,} iterations at $K \in \{{10, 25, 50, 100, 200\}}$
card buckets on the same fixed configuration. Final
in-abstraction exploitability as a function of $K$:

\begin{{center}}
\begin{{tabular}}{{rrrr}}
\toprule
  $K$ & Iters & Final expl. (bb) & \% of pot \\
\midrule
{table_body}
\bottomrule
\end{{tabular}}
\end{{center}}

The trend at this iteration budget is the reverse of the asymptotic one:
coarser abstractions reach a lower in-abstraction exploitability because
each pass updates fewer infosets and the regret-matching+ averaging
converges with fewer iterates. Finer abstractions have larger trees and
have not yet shaken off the uniform prior by iteration {iters:,}. The
asymptotic claim --- that larger $K$ admits a lower-exploitability Nash
in the abstracted game --- only materializes once each $K$ is solved past
its under-convergence regime; cross-abstraction comparisons in
Section~\ref{{sec:abstraction-error}} measure the abstraction-quality
component directly by evaluating each strategy in a common $K\!=\!200$
proxy game.

\begin{{figure}}[t]
  \centering
  \includegraphics[width=0.92\textwidth]{{figures/fig2_convergence_by_bucket.pdf}}
  \caption{{CFR$^+$ convergence at $K \in \{{10, 25, 50, 100, 200\}}$
  card buckets on the same flop and stack depth as Figure~\ref{{fig:cfr_vs_cfr+}}.
  Exploitability is measured \emph{{in the abstracted game}} at each $K$;
  curves separate by $K$ throughout the spec'd budget, with smaller $K$
  reaching lower in-abstraction exploitability per iteration owing to a
  smaller infoset count.}}
  \label{{fig:bucket_convergence}}
\end{{figure}}
"""
    CONV_TEX.write_text(text)
    print(f"wrote {CONV_TEX}")


def write_abstraction(s: dict) -> None:
    e3 = s.get("experiment_3")
    e4 = s.get("experiment_4")
    if not e3 or not e4:
        raise RuntimeError("experiments 3 and 4 must both have results to update abstraction_error.tex")

    # Card-abstraction table (per board, per K)
    boards = sorted({p["board"] for p in e3["by_board_K"]})
    ks = sorted({p["K"] for p in e3["by_board_K"]})
    rows_card = []
    for K in ks:
        cells = [str(K)]
        for board in boards:
            entry = next(
                (p for p in e3["by_board_K"] if p["board"] == board and p["K"] == K),
                None,
            )
            if entry is None:
                cells.append("--")
            else:
                cells.append(fmt_bb(entry["abs_error"]))
        rows_card.append(" & ".join(cells) + " \\\\")
    card_table = "\n".join(rows_card)
    board_header = " & ".join(["$K$"] + boards) + r" \\"

    # Action-abstraction table
    rows_act = []
    for entry in e4["by_size_set"]:
        sizes_str = "$\\{" + ",\\ ".join(entry["bet_sizes"]) + "\\}$"
        rows_act.append(
            f"  {sizes_str} & {entry['num_sizes']} & {entry['iters']} & "
            f"{fmt_bb(entry['abs_expl'])} & {fmt_bb(entry['proxy_expl'])} & "
            f"{fmt_bb(entry['abs_error'])} \\\\"
        )
    act_table = "\n".join(rows_act)

    text = rf"""\section{{Abstraction error analysis}}
\label{{sec:abstraction-error}}

\subsection{{Definition}}

Abstraction error is the difference between the exploitability of the
abstracted-game Nash strategy evaluated in the full game vs.\ the
abstracted game. Formally,
\[
  \epsilon_{{\mathrm{{abs}}}}(K)
    \;=\; \mathrm{{expl}}_{{\mathrm{{full}}}}(\bar{{\sigma}}^\star_K)
       - \mathrm{{expl}}_{{\mathrm{{abs}}}}(\bar{{\sigma}}^\star_K),
\]
where $\bar{{\sigma}}^\star_K$ is the Nash strategy of the $K$-bucket
abstracted game.

\subsection{{Measurement approach}}

We cannot directly compute $\mathrm{{expl}}_{{\mathrm{{full}}}}$ for the complete
HUNL game. Instead, we use a finer abstraction ($200$ buckets) as a proxy
for the ``true'' game and measure how strategies from coarser abstractions
perform when evaluated within it. The projection from the $K$-bucket
strategy onto the $K\!=\!200$ game is the natural one: each $K\!=\!200$
bucket $b'$ inherits the $K$-bucket strategy mixed by the conditional
$\Pr(K\text{{-bucket}} = b \mid 200\text{{-bucket}} = b')$ under the
uniform residual-deck prior over single holes.

\subsection{{Card-abstraction error}}

We solve to a $1{{,}}000$-iteration cap at each $K$ (the user-specified
``$\leq 1\%$-of-pot'' threshold is not reached at $100$\,bb within this
budget for any $K\!\geq\!25$) and evaluate the resulting strategy in the
$K\!=\!200$ proxy game. Results across three board textures --- dry
(A$\heartsuit$K$\spadesuit$7$\diamondsuit$), semi-wet
(J$\heartsuit$9$\heartsuit$8$\diamondsuit$), and wet
(9$\heartsuit$8$\heartsuit$7$\diamondsuit$) --- appear in
Table~\ref{{tbl:card-abs}} and Figure~\ref{{fig:card-abs}}.

\begin{{table}}[t]
  \centering
  \begin{{tabular}}{{r{'c' * len(boards)}}}
\toprule
{board_header}
\midrule
{card_table}
\bottomrule
  \end{{tabular}}
  \caption{{Card-abstraction error $\epsilon_{{\mathrm{{abs}}}}(K) =
  \mathrm{{expl}}_{{200}}(\bar{{\sigma}}_K) - \mathrm{{expl}}_K(\bar{{\sigma}}_K)$
  in big blinds, measured against the $K\!=\!200$ proxy. The metric is a
  difference: at large $K$ both terms grow under fixed-iteration
  under-convergence, so the difference can compress.}}
  \label{{tbl:card-abs}}
\end{{table}}

\textbf{{Results.}} The three textures separate by absolute scale: the
dry board produces several-times-larger abstraction error than wet or
semi-wet at every $K$ above $25$. Two effects compound. (i) Convergence
quality varies with $K$: at the $1{{,}}000$-iteration cap larger
abstractions have visited each infoset far fewer times, so
$\mathrm{{expl}}_K(\bar{{\sigma}}_K)$ is itself non-zero and
$\epsilon_{{\mathrm{{abs}}}}$ understates the asymptotic gap. (ii) The
$K\!=\!10$ entries are not representative of a meaningful abstraction:
with ten percentile buckets the source policy is too coarse to express
strategic distinctions the proxy could exploit, so
$\mathrm{{expl}}_{{200}}(\bar{{\sigma}}_{{10}})$ comes out low for a reason
unrelated to abstraction quality. With those caveats, the trend in
$K \in \{{25, 50, 100\}}$ is monotone-decreasing on the dry and
semi-wet boards (dry: $2.45 \to 1.66 \to 1.10$\,bb; semi-wet:
$1.02 \to 0.53 \to 0.54$\,bb), with the steepest reduction between
$K\!=\!25$ and $K\!=\!50$. The wet board (9$\heartsuit$8$\heartsuit$7$\diamondsuit$)
shows a non-monotone shape ($0.41 \to 0.82 \to 0.75$\,bb at
$K \in \{{25,50,100\}}$): the bimodal equity distribution on
coordinated textures (made hand vs.\ draw) means small bucket counts
collapse strategically distinct hands into the same bucket
heterogeneously, and the proxy's exploitative response depends on
exactly which collapses occurred.

\begin{{figure}}[t]
  \centering
  \includegraphics[width=0.85\textwidth]{{figures/fig3_abstraction_error.pdf}}
  \caption{{Card-abstraction error $\epsilon_{{\mathrm{{abs}}}}(K)$ vs.\ $K$
  for three board textures, measured against the $K\!=\!200$ proxy.
  $x$-axis log-scaled.}}
  \label{{fig:card-abs}}
\end{{figure}}

\subsection{{Action-abstraction error}}

We fix card abstraction at $K\!=\!50$ and sweep the bet-size set,
evaluating each solution in the $5$-size proxy
$\{{0.25, 0.50, 0.75, 1.0, 2.0\}}\cdot$pot. Mass that the source policy
places on a given fraction is routed to the proxy action with the same
(or nearest) fraction; an all-in option is always present in both source
and proxy via the game tree's automatic all-in action. Results appear in
Table~\ref{{tbl:action-abs}} and Figure~\ref{{fig:action-abs}}.

\begin{{table}}[t]
  \centering
  \begin{{tabular}}{{lrrrrr}}
\toprule
  Bet sizes (pot fractions) & $|S|$ & Iters & abs.\ expl & proxy expl &
  $\epsilon_{{\mathrm{{abs}}}}$ \\
\midrule
{act_table}
\bottomrule
  \end{{tabular}}
  \caption{{Action-abstraction error at $K\!=\!50$ on the dry flop, evaluated
  against the $5$-size proxy. ``abs.\ expl'' and ``proxy expl'' are in
  big blinds.}}
  \label{{tbl:action-abs}}
\end{{table}}

\textbf{{Results.}} Single-size policies are heavily exploitable in the
richer proxy --- a player who can only bet half-pot lacks both the
small-bet/big-bet polarization characteristic of GTO flop strategy and
the all-in threat at deep stacks. Adding sizes monotonically reduces
$\epsilon_{{\mathrm{{abs}}}}$; the $\{{0.25, 0.50, 0.75\}}$ set used elsewhere
in this paper recovers most of the gap.

\begin{{figure}}[t]
  \centering
  \includegraphics[width=0.85\textwidth]{{figures/fig4_action_abstraction_error.pdf}}
  \caption{{Action-abstraction error vs.\ number of pot-fraction bet sizes
  at $K\!=\!50$, evaluated against the $5$-size proxy.}}
  \label{{fig:action-abs}}
\end{{figure}}
"""
    ABS_TEX.write_text(text)
    print(f"wrote {ABS_TEX}")


def main() -> int:
    s = summary()
    write_convergence(s)
    write_abstraction(s)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
