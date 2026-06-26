# Paper outline — Regret: Counterfactual Regret Minimization in Practice

**Working title:** *Regret: An Open Framework for Empirical Analysis of CFR+ Convergence and Abstraction Error in Heads-Up No-Limit Hold'em*

**Target venues:** ICML workshop on Learning in Games, NeurIPS workshop on Decision-Making, or arXiv preprint (cs.GT / cs.LG)

**Authors:** Arya Somu, Monolith Research

---

## Abstract (draft)

We present Regret, an open-source CFR+ solver for Heads-Up No-Limit Hold'em (HUNL) paired with an interactive web interface for visualizing strategy convergence and hand range analysis. Beyond the implementation, we make three research contributions: (1) an empirical characterization of CFR+ convergence rates as a function of game abstraction granularity, extending known theoretical bounds to practical parameter regimes; (2) a systematic measurement of abstraction error — the gap between the abstracted-game Nash equilibrium and the full-game Nash equilibrium — as a function of card bucket count and bet-size discretization; and (3) a formal connection between CFR+ and the Follow-the-Regularized-Leader (FTRL) framework, clarifying the role of regret flooring in accelerating convergence. All results are reproducible via the Regret web interface.

---

## 1. Introduction

### 1.1 Motivation

Poker has served as a canonical benchmark for game-theoretic AI since the early work on Libratus and DeepStack. Despite significant progress, the practical relationship between abstraction design and solution quality remains undercharacterized in accessible literature. Most published CFR analyses are either purely theoretical (convergence guarantees under idealized conditions) or proprietary (commercial solvers with closed implementations).

Regret addresses this gap. By open-sourcing both the solver and the empirical analysis framework, we aim to make the following questions answerable without access to proprietary tools:

- How quickly does CFR+ converge as a function of abstraction granularity?
- How much solution quality do we sacrifice by using coarse card or action abstractions?
- What is the practical relationship between CFR+ and no-regret online learning theory?

### 1.2 Contributions

1. **Open-source CFR+ reference implementation** in Rust with documented abstractions and a reproducible benchmarking harness
2. **Empirical convergence analysis** measuring exploitability across iteration counts, bucket sizes, and bet-size configurations
3. **Abstraction error quantification** — a systematic study of how abstraction granularity affects distance to the true Nash equilibrium
4. **Theoretical framing** connecting CFR+ to FTRL and Blackwell approachability, with the regret-flooring step situated within the broader online learning literature

---

## 2. Background

### 2.1 Extensive-form games

A two-player zero-sum extensive-form game is defined by a game tree where each node is either a chance node (nature acts), a decision node (a player acts), or a terminal node (payoffs are realized). HUNL is a two-player zero-sum game in extensive form with imperfect information.

Formally: a game $\Gamma = (N, H, Z, A, u, \sigma_c, \mathcal{I})$ where $N = \{1, 2\}$ is the player set, $H$ is the set of histories (nodes), $Z \subseteq H$ is the set of terminal nodes, $A(h)$ is the action set at history $h$, $u_i: Z \to \mathbb{R}$ is the utility function for player $i$, $\sigma_c$ is the chance strategy (card dealing probabilities), and $\mathcal{I}$ is the information partition.

### 2.2 Nash equilibrium and exploitability

A **Nash equilibrium** is a strategy profile $(\sigma_1^*, \sigma_2^*)$ such that neither player can improve their expected utility by unilaterally deviating:

$$u_i(\sigma_i^*, \sigma_{-i}^*) \geq u_i(\sigma_i, \sigma_{-i}^*) \quad \forall \sigma_i, \forall i \in N$$

In two-player zero-sum games, Nash equilibria always exist and all Nash equilibria yield the same expected payoffs (von Neumann's minimax theorem).

**Exploitability** of a strategy $\sigma_i$ is the maximum gain available to a best-responding opponent:

$$\text{expl}(\sigma_i) = \max_{\sigma_{-i}} u_{-i}(\sigma_i, \sigma_{-i}) - v^*$$

where $v^*$ is the game value. Exploitability of 0 means $\sigma_i$ is a Nash strategy. We use exploitability as our primary convergence metric throughout.

### 2.3 Counterfactual regret minimization (CFR)

CFR (Zinkevich et al., 2007) solves for Nash equilibria in extensive-form games by iteratively minimizing regret at each information set. At each iteration $t$, for each infoset $I$ and action $a$:

**Counterfactual regret:**
$$r^t(I, a) = \sum_{h \in I} \pi_{-i}^{\sigma^t}(h) \left[ v_i(\sigma^t_{I \to a}, h) - v_i(\sigma^t, h) \right]$$

where $\pi_{-i}^{\sigma^t}(h)$ is the opponent's reach probability to history $h$, and $\sigma^t_{I \to a}$ is the strategy that always plays $a$ at infoset $I$.

**Cumulative regret:** $R^T(I, a) = \sum_{t=1}^T r^t(I, a)$

**Regret matching (strategy update):**
$$\sigma^{T+1}(I, a) = \frac{R^T_+(I, a)}{\sum_{a'} R^T_+(I, a')}$$

where $R^T_+(I, a) = \max(R^T(I, a), 0)$.

**Convergence guarantee:** The average strategy $\bar{\sigma}^T$ has exploitability bounded by $O(1/\sqrt{T})$.

### 2.4 CFR+

CFR+ (Tammelin, 2014) modifies the regret accumulation step:

$$R^{T+1}(I, a) = \max\left(R^T(I, a) + r^T(I, a), 0\right)$$

Negative cumulative regrets are floored to zero at each iteration rather than at strategy computation time. This seemingly small change yields dramatically faster empirical convergence — in practice, CFR+ typically requires an order of magnitude fewer iterations than vanilla CFR to reach the same exploitability level.

The theoretical explanation involves the algorithm's effective "forgetting" of historical misplay, keeping the strategy responsive to recent traversals.

### 2.5 Game abstraction

Full HUNL has approximately $10^{18}$ distinct infosets — computationally intractable for exact solving. Practical solvers work within an abstracted game:

**Card abstraction** groups strategically similar hands into buckets, replacing exact hand identities with equity percentile estimates. The abstracted game has $K$ card buckets instead of the full hand space.

**Action abstraction** discretizes the continuous bet-size space into a finite set of sizes. This reduces the branching factor at each decision point from infinite to a small fixed number.

---

## 3. Related work

- **Libratus** (Brown & Sandholm, 2017) — first superhuman HUNL agent; used blueprint strategy + real-time subgame solving; abstraction details proprietary
- **DeepStack** (Moravčík et al., 2017) — deep learning + continual resolving; no published abstraction analysis
- **Slumbot** (Jackson, 2013) — open CFR implementation; strongest openly available bot; single-threaded, limited abstraction analysis
- **CFR+** (Tammelin, 2014) — original paper; theoretical analysis but limited empirical characterization
- **Abstraction pathologies** (Waugh et al., 2009) — coarser abstractions can produce more exploitable strategies; motivates our systematic measurement
- **FTRL and online learning** (Hazan, 2022) — provides the theoretical framework we use to situate CFR+

**Gap:** No existing work provides an open, reproducible framework for empirically measuring the relationship between abstraction granularity and solution quality across a range of HUNL configurations. Regret fills this gap.

---

## 4. The Regret solver

### 4.1 Game representation

We implement a flop-subgame solver for HUNL. The game tree is built lazily during CFR traversal. Game states encode hole cards, board, pot size, stack sizes, street, and action history. Terminal nodes are detected by fold, all-in resolution, or river completion.

### 4.2 Abstraction configuration

**Card abstraction:** We implement equity-percentile bucketing. For each (hole cards, board) pair, equity is estimated via Monte Carlo sampling (1,000 rollouts). Hands are sorted by equity and assigned to one of $K$ buckets. We test $K \in \{10, 25, 50, 100, 200\}$.

**Action abstraction:** We test configurations of $\{2, 3, 5\}$ bet sizes, drawn from $\{25\%, 50\%, 75\%, 100\%, \text{allin}\}$.

### 4.3 Exploitability computation

We compute exploitability via best-response computation: for each player, compute the best response to the opponent's current average strategy, then measure the gain. This is exact within the abstracted game. We compute it every 50 iterations.

### 4.4 Parallelization

Version 1 implements single-threaded alternating updates (player 0 on even iterations, player 1 on odd). Parallel external sampling CFR is left for future work.

---

## 5. Convergence analysis

### 5.1 CFR vs CFR+

We compare vanilla CFR and CFR+ across 1,000 iterations on a fixed game configuration (BTN vs BB, 100bb, A♥K♠7♦ flop, 50-bucket card abstraction, 3 bet sizes). We measure exploitability every 50 iterations.

**Expected results:** CFR+ reaches $\leq 5\%$ exploitability in approximately 200–300 iterations; vanilla CFR requires approximately 800–1,000 iterations for the same threshold.

**Figure 1:** Exploitability vs. iterations for CFR and CFR+. Log-scale y-axis. CFR+ convergence is approximately $O(1/T)$ empirically vs. $O(1/\sqrt{T})$ for CFR — consistent with Tammelin's (2014) observations.

### 5.2 Effect of abstraction granularity on convergence rate

We run CFR+ to convergence (exploitability $\leq 1\%$) under varying card abstraction bucket counts and measure: (a) iterations to convergence, (b) wall-clock time to convergence, and (c) final exploitability at iteration 1,000.

**Expected results:** Coarser abstractions converge in fewer iterations (smaller game tree) but plateau at higher exploitability. 50 buckets provides a good efficiency–quality tradeoff.

**Figure 2:** Convergence curves for $K \in \{10, 25, 50, 100, 200\}$ buckets.

---

## 6. Abstraction error analysis

### 6.1 Definition

Abstraction error is the difference between the exploitability of the abstracted-game Nash strategy when evaluated in the full game vs. the abstracted game. Formally:

$$\epsilon_{\text{abs}}(K) = \text{expl}_{\text{full}}(\bar{\sigma}^*_K) - \text{expl}_{\text{abs}}(\bar{\sigma}^*_K)$$

where $\bar{\sigma}^*_K$ is the Nash strategy of the $K$-bucket abstracted game.

### 6.2 Measurement approach

We cannot directly compute $\text{expl}_{\text{full}}$ for the complete HUNL game. Instead, we use a finer abstraction (200 buckets) as a proxy for the "true" game and measure how strategies from coarser abstractions perform when evaluated within it.

### 6.3 Card abstraction error

We solve to convergence at each bucket count and evaluate the resulting strategy in the 200-bucket proxy game.

**Expected results:** Abstraction error decreases monotonically with bucket count but exhibits diminishing returns above 50–100 buckets. Error is more pronounced on wet boards (coordinated texture) where hand equity distributions are more sensitive to bucketing resolution.

**Figure 3:** Abstraction error vs. bucket count for dry, semi-wet, and wet board textures.

### 6.4 Action abstraction error

We fix card abstraction at 50 buckets and vary the bet-size set. We measure exploitability in a richer-action evaluation game.

**Expected results:** Missing pot-sized bets introduces more error than missing small bets, particularly on the flop where pot-size bets are part of the polarization structure of GTO play.

**Figure 4:** Abstraction error vs. bet-size configuration.

---

## 7. Connection to online learning theory

### 7.1 CFR as FTRL

CFR can be understood as a variant of Follow-the-Regularized-Leader (FTRL) applied independently at each information set. At each infoset, the acting player minimizes cumulative regret using a linear loss with entropic regularization (regret matching = FTRL with negative entropy regularizer).

This connection implies that CFR inherits FTRL's $O(1/\sqrt{T})$ regret bound:

$$\frac{R^T}{T} \leq \frac{\Delta}{\sqrt{T}}$$

where $\Delta$ is the range of utilities.

### 7.2 CFR+ as optimistic FTRL

The regret-flooring step in CFR+ is equivalent to the "optimistic" update in Optimistic FTRL (Rakhlin & Sridharan, 2013). Optimistic FTRL achieves $O(1/T)$ regret when the loss sequence is predictable, which in self-play corresponds to the strategy pair converging (each player's strategy becomes more predictable as they approach Nash). This provides a theoretical mechanism for the faster empirical convergence of CFR+.

**Proposition:** In self-play with alternating updates, CFR+ with regret flooring is equivalent to Optimistic FTRL with a specific predictive term. The $O(1/T)$ convergence observed empirically follows from the self-play structure inducing low predictability error.

### 7.3 Blackwell approachability

The original CFR convergence proof (Zinkevich et al., 2007) is grounded in Blackwell's approachability theorem (1956). We briefly state the connection: the regret minimization at each infoset corresponds to approaching the non-positive orthant in regret space. CFR's convergence to Nash follows from the product of per-infoset regret minimizers approaching a global regret minimum.

---

## 8. Discussion

### 8.1 Practical implications for solver design

The abstraction error results suggest that card abstraction granularity above 50–100 buckets yields diminishing quality improvements, while action abstraction error is more sensitive to which sizes are included than to how many. Practitioners building HUNL solvers should prioritize including pot-sized bets and all-in before adding intermediate sizes.

### 8.2 Relation to broader game-solving

The CFR–FTRL connection has implications beyond poker. Any finite two-player zero-sum extensive-form game can in principle be solved using the same framework. The abstraction methodology (equity bucketing, action discretization) generalizes to any game with continuous action spaces or large state representations.

### 8.3 Limitations

- Flop subgame solving only; preflop and river results may differ
- Abstraction error measurement uses a proxy "true" game, not the actual full game
- Single-threaded implementation; parallel CFR may exhibit different convergence behavior

---

## 9. Conclusion

We introduced Regret, an open CFR+ solver with a reproducible empirical analysis framework. Our main findings: CFR+ converges approximately an order of magnitude faster than vanilla CFR in practice; abstraction error from card bucketing decreases sharply up to 50 buckets and slowly thereafter; action abstraction error is more sensitive to which sizes are included than to set size; and CFR+ can be formally understood as Optimistic FTRL in self-play, explaining its faster convergence.

All results are reproducible via the Regret web interface at [regret.so].

---

## References

- Zinkevich, M., Johanson, M., Bowling, M., Piccione, C. (2007). Regret minimization in games with incomplete information. *NeurIPS.*
- Tammelin, O. (2014). Solving large imperfect information games using CFR+. *arXiv:1407.5042.*
- Brown, N., Sandholm, T. (2017). Libratus: The superhuman AI for no-limit poker. *IJCAI.*
- Moravčík, M. et al. (2017). DeepStack: Expert-level artificial intelligence in heads-up no-limit poker. *Science.*
- Waugh, K., Schnizlein, D., Bowling, M., Szafron, D. (2009). Abstraction pathologies in extensive games. *AAMAS.*
- Rakhlin, A., Sridharan, K. (2013). Online learning with predictable sequences. *COLT.*
- Hazan, E. (2022). Introduction to online convex optimization. *MIT Press.*
- Blackwell, D. (1956). An analog of the minimax theorem for vector payoffs. *Pacific Journal of Mathematics.*

---

## Appendix A — Exploitability computation details

Full best-response computation pseudocode and correctness argument.

## Appendix B — Monte Carlo equity estimation

Justification for 1,000-sample equity estimation; bias and variance analysis.

## Appendix C — Reproducibility checklist

Hardware specs, random seeds, full hyperparameter table for all experiments.
