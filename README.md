# Regret

A CFR+ Nash equilibrium solver for Heads-Up No-Limit Hold'em, with a live convergence dashboard and interactive range explorer.

Built as both a research tool and an open-source reference implementation. The accompanying paper analyzes CFR+ convergence rates, the effect of card and action abstraction on solution quality, and the connection between counterfactual regret minimization and no-regret online learning theory.

---

## What it does

- Solves HUNL poker subgames to approximate Nash equilibrium using CFR+
- Streams convergence progress in real time (exploitability per iteration)
- Visualizes solved strategy profiles as interactive 13×13 hand range heatmaps
- Lets you compare CFR vs CFR+ convergence empirically across game configurations
- Exports strategy profiles for use in the paper's empirical analysis

---

## Project structure

```
regret/
├── engine/          # Rust CFR+ solver
│   ├── src/
│   │   ├── cfr.rs           # CFR+ algorithm and regret matching
│   │   ├── game.rs          # Game tree and state representation
│   │   ├── infoset.rs       # Information set abstraction
│   │   ├── eval.rs          # Fast 7-card hand evaluator (lookup table)
│   │   └── abstraction.rs   # Card and action abstraction layers
│   └── Cargo.toml
├── api/             # Axum HTTP + SSE server
│   ├── src/
│   │   └── main.rs
│   └── Cargo.toml
├── web/             # Next.js frontend
│   ├── components/
│   │   ├── RangeHeatmap.tsx
│   │   ├── ConvergenceChart.tsx
│   │   ├── SolverConfig.tsx
│   │   └── ActionDistribution.tsx
│   ├── pages/
│   │   ├── index.tsx        # Solver view
│   │   ├── explorer.tsx     # Range explorer
│   │   └── lab.tsx          # Convergence lab
│   └── package.json
├── paper/           # LaTeX source
│   ├── main.tex
│   ├── sections/
│   └── figures/
└── README.md
```

---

## Quickstart

### Requirements

- Rust 1.75+
- Node.js 18+
- pnpm (or npm)

### Run the solver engine and API

```bash
cd engine
cargo build --release

cd ../api
cargo run --release
# API running at http://localhost:8080
```

### Run the frontend

```bash
cd web
pnpm install
pnpm dev
# UI at http://localhost:3000
```

---

## API overview

| Method | Endpoint | Description |
|---|---|---|
| `POST` | `/solve` | Start a solver run with game parameters |
| `GET` | `/solve/:id/stream` | SSE stream of convergence progress |
| `GET` | `/strategy/:infoset` | Action distribution for a specific infoset |
| `GET` | `/convergence/:id` | Full convergence history for a completed run |

---

## Game scope

The current implementation targets **Heads-Up No-Limit Hold'em** with:

- Discretized bet sizes: 25%, 50%, 75% pot, pot, all-in (configurable)
- Card abstraction: equity-percentile bucketing (10–200 buckets, configurable)
- Stack depths: 20–200bb
- Streets: flop subgame solving (turn and river planned)

Full preflop and multi-street solving are on the roadmap. See `ARCHITECTURE.md` for abstraction details and known limitations.

---

## Paper

The research paper lives in `/paper`. It covers:

- Formal connection between CFR+ and no-regret online learning
- Theoretical convergence bounds vs. empirical results from this implementation
- Abstraction error analysis: how card and action abstraction degrades solution quality
- CFR vs CFR+ empirical comparison across game configurations

Every figure in the paper is reproducible via the web UI.

---

## Contributing

See `CONTRIBUTING.md`.

---

## License

MIT
