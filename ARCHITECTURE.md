# Architecture

This document is the technical reference for the Regret codebase. It covers the solver engine, abstraction layers, API server, and frontend in full detail.

---

## 1. Solver engine (`engine/`)

Written in Rust. The engine is a self-contained crate with no runtime dependencies beyond the standard library and a small set of utility crates (`serde`, `rand`, `dashmap`).

### 1.1 Game representation

**`game.rs`**

A poker game state is fully described by:

```rust
pub struct GameState {
    pub hole_cards: [Card; 2],        // acting player's private cards
    pub board: Vec<Card>,             // public cards (0–5)
    pub pot: f64,                     // total pot size in bb
    pub stacks: [f64; 2],            // remaining stacks [IP, OOP]
    pub street: Street,              // Preflop | Flop | Turn | River
    pub history: ActionHistory,      // sequence of actions this street
    pub active_player: Player,       // whose turn it is
}

pub enum Street { Preflop, Flop, Turn, River }

pub enum Action {
    Fold,
    Check,
    Call,
    Bet(f64),   // size in bb
    Raise(f64), // size in bb
}
```

The game tree is built lazily — nodes are expanded on first visit during CFR traversal rather than pre-allocated. This keeps memory usage proportional to the explored portion of the tree.

**Terminal nodes** are detected by: (a) one player folds, (b) one player is all-in and all streets have been dealt, or (c) both players check the river.

### 1.2 Information sets

**`infoset.rs`**

An information set (infoset) is the key abstraction in CFR. It represents the set of game states that are indistinguishable from the perspective of the acting player — i.e., states that share the same hole cards, public board, and action history.

```rust
pub struct InfoSet {
    pub card_bucket: u32,     // abstracted hand strength bucket
    pub action_history: Vec<AbstractAction>,  // discretized action sequence
}

impl InfoSet {
    pub fn key(&self) -> u64 {
        // fast hash combining card_bucket and action_history
        // used as key in the regret/strategy tables
    }
}
```

Infosets are keyed by a 64-bit hash and stored in a `DashMap<u64, InfoSetData>` for concurrent access during parallel traversal.

```rust
pub struct InfoSetData {
    pub regret_sum: Vec<f64>,    // cumulative regrets per action
    pub strategy_sum: Vec<f64>, // cumulative strategy (for average)
}
```

### 1.3 CFR+ algorithm

**`cfr.rs`**

CFR+ is the primary algorithm. It differs from vanilla CFR in one key way: negative cumulative regrets are floored to zero at each iteration rather than carried forward. This dramatically accelerates convergence in practice.

```rust
pub struct CFRSolver {
    pub infosets: DashMap<u64, InfoSetData>,
    pub game_tree: GameTree,
    pub abstraction: AbstractionConfig,
    pub iterations: u64,
}

impl CFRSolver {
    pub fn run(&mut self, iterations: u64) -> ConvergenceHistory {
        for t in 0..iterations {
            // Alternating updates: player 0 on even iters, player 1 on odd
            self.traverse(Player::IP, t);
            self.traverse(Player::OOP, t);

            if t % 50 == 0 {
                let exploit = self.compute_exploitability();
                self.history.record(t, exploit);
            }
        }
        self.history.clone()
    }

    fn traverse(&mut self, updating_player: Player, t: u64) -> f64 {
        // Recursive game tree traversal
        // Returns counterfactual value at this node
        // Updates regrets for the updating_player's infosets
    }

    fn regret_match(&self, regrets: &[f64]) -> Vec<f64> {
        // CFR+: floor regrets at 0 before normalizing
        let positive: Vec<f64> = regrets.iter().map(|r| r.max(0.0)).collect();
        let sum: f64 = positive.iter().sum();
        if sum > 0.0 {
            positive.iter().map(|r| r / sum).collect()
        } else {
            vec![1.0 / regrets.len() as f64; regrets.len()] // uniform
        }
    }
}
```

**Exploitability** is computed every 50 iterations as a convergence diagnostic. It measures how much either player could gain by deviating unilaterally from the current strategy — a value of 0 means Nash equilibrium. In practice, solutions below 0.5% exploitability are considered effectively solved for most purposes.

**Average strategy** (not the final iterate) is the Nash approximant. The `strategy_sum` accumulates weighted strategies across iterations; the average is taken at the end.

### 1.4 Hand evaluator

**`eval.rs`**

Uses a two-level lookup table approach for 7-card hand evaluation:

1. Enumerate all 7,462 distinct 5-card hand equivalence classes
2. Build a lookup table mapping any 5-card combination → rank (0–7461)
3. For 7-card hands, enumerate all C(7,5) = 21 subsets and take the minimum rank

This evaluates any 7-card hand in ~21 table lookups, fast enough for the inner loop of CFR traversal.

```rust
pub struct HandEvaluator {
    rank_table: HashMap<u32, u16>,  // card bitmask → hand rank
}

impl HandEvaluator {
    pub fn evaluate(&self, cards: &[Card; 7]) -> HandRank {
        // enumerate 21 5-card subsets, return best rank
    }
}
```

### 1.5 Abstraction layer

**`abstraction.rs`**

Without abstraction, HUNL has on the order of 10^18 infosets — computationally intractable. Two abstraction types are applied:

**Card abstraction** buckets hands by equity percentile. Rather than tracking exact hole cards, the solver works with equity buckets. For a given board, each hand is evaluated against a random sample of opponent hands and assigned a bucket (0–N) based on its equity percentile.

```rust
pub struct CardAbstraction {
    pub num_buckets: u32,  // 10 (fast) to 200 (precise)
}

impl CardAbstraction {
    pub fn bucket(&self, hole: &[Card; 2], board: &[Card]) -> u32 {
        let equity = self.monte_carlo_equity(hole, board, 1000);
        (equity * self.num_buckets as f64) as u32
    }
}
```

**Action abstraction** discretizes the continuous bet-size space into a fixed set of sizes. The default configuration uses {25%, 50%, 75% pot, pot, all-in}. This reduces the branching factor at each decision node from infinite to 5 (plus fold/check/call).

**Abstraction error** — both abstractions introduce approximation error: the solved strategy is Nash only within the abstracted game, not the full game. The paper's empirical analysis quantifies this error as a function of bucket count and bet-size granularity.

---

## 2. API server (`api/`)

Written in Rust using [Axum](https://github.com/tokio-rs/axum). The server manages solver jobs, streams convergence progress over SSE, and serves solved strategy profiles to the frontend.

### 2.1 Endpoints

```
POST   /solve
GET    /solve/:id/stream
GET    /strategy/:infoset
GET    /convergence/:id
```

**`POST /solve`**

Accepts game configuration, spawns a solver task on Tokio's thread pool, returns a job ID.

```json
{
  "matchup": "BTN_vs_BB",
  "stack_depth": 100,
  "board": ["Ah", "Ks", "7d"],
  "bet_sizes": [0.25, 0.50, 0.75, 1.0],
  "card_buckets": 50,
  "max_iterations": 1000
}
```

Response:
```json
{ "job_id": "a3f9c2" }
```

**`GET /solve/:id/stream`**

Server-Sent Events stream. Each event is emitted every 50 iterations:

```
data: {"iteration": 50, "exploitability": 18.4, "elapsed_ms": 312}
data: {"iteration": 100, "exploitability": 11.2, "elapsed_ms": 621}
...
data: {"iteration": 1000, "exploitability": 2.9, "elapsed_ms": 6140, "done": true}
```

**`GET /strategy/:infoset`**

Returns the solved mixed strategy for a specific infoset key. Used by the range explorer when a user hovers a hand combo.

```json
{
  "infoset": "AA_flop_AK7r",
  "actions": ["fold", "check", "bet_50", "bet_75"],
  "probabilities": [0.00, 0.10, 0.18, 0.72]
}
```

**`GET /convergence/:id`**

Returns the full convergence history for a completed job — used by the Convergence Lab to generate paper figures.

### 2.2 Job management

Jobs are stored in a `DashMap<String, JobHandle>`. Each job holds:
- Current status (running / done / error)
- A Tokio broadcast channel for SSE subscribers
- The final strategy profile once complete

---

## 3. Frontend (`web/`)

Next.js 14 + TypeScript. Three pages, four core components.

### 3.1 Pages

**`/` — Solver**

The main interactive view. Left sidebar holds game configuration inputs (matchup, stack depth, board, bet sizes, abstraction granularity, max iterations). Right panel is split: convergence chart on top, range heatmap on bottom.

On "Solve", the frontend hits `POST /solve`, then opens the SSE stream and updates the convergence chart and stat cards in real time. When the stream closes (`done: true`), the range heatmap populates with the solved strategy.

**`/explorer` — Range explorer**

Full-page 13×13 hand matrix. Users can filter by street, board texture, and position. Clicking a cell shows the full action distribution for that combo. Designed to be the tool a researcher or curious user spends time in after the solve completes.

**`/lab` — Convergence lab**

Side-by-side comparison of CFR vs CFR+ convergence curves across configurable game parameters. This page generates the paper's empirical figures. Users can run multiple solver configurations and overlay the resulting curves. Each curve is exportable as CSV.

### 3.2 Components

**`RangeHeatmap.tsx`**

Renders the 13×13 grid. Each cell is colored by action frequency: purple for bet-heavy combos, teal for check-heavy, gray for mostly folding. Intensity encodes frequency magnitude. Hover triggers a strategy fetch from `/strategy/:infoset` and renders `ActionDistribution`.

**`ConvergenceChart.tsx`**

Recharts `LineChart` consuming the SSE stream. Updates on each event. Supports overlaying multiple runs for comparison (used in the lab).

**`SolverConfig.tsx`**

The sidebar form. Controlled inputs for all solver parameters. Emits a typed `SolverConfig` object on submit.

**`ActionDistribution.tsx`**

Horizontal stacked bar showing the mixed strategy for a hovered combo. Each segment is labeled with the action name and probability. Animates on change.

### 3.3 State management

No global state library. The solver page uses `useReducer` for solver state (idle / running / done / error) and a `useRef` for the SSE `EventSource` instance. The range heatmap data lives in component state and is populated once the solve completes.

---

## 4. Data flow

```
User configures game
        ↓
POST /solve → job_id
        ↓
GET /solve/:id/stream (SSE)
        ↓
ConvergenceChart updates live
        ↓
Stream closes (done: true)
        ↓
Fetch strategy profiles per infoset
        ↓
RangeHeatmap populates
        ↓
User hovers combo → ActionDistribution renders
```

---

## 5. Performance notes

- The Rust solver runs on a background Tokio thread, non-blocking for the API
- CFR traversal is single-threaded in v1; parallel CFR (external sampling) is planned
- Hand evaluation: ~21 table lookups per 7-card hand, negligible cost vs. tree traversal
- A 1,000-iteration solve on a 50-bucket abstraction with 3 bet sizes takes approximately 3–8 seconds on a modern laptop
- Strategy profiles for a full flop subgame at 50 buckets are approximately 2–5 MB serialized

---

## 6. Known limitations

- Flop subgame solving only in v1; preflop and multi-street are planned
- Card abstraction introduces solution quality loss at low bucket counts (quantified in the paper)
- Action abstraction at 3 sizes misses some mixed-frequency spots that require finer granularity
- No imperfect recall abstraction (all abstractions are perfect recall)
