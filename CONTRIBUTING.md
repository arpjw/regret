# Contributing to Regret

Thanks for your interest in contributing. Regret is an open research project — contributions to the solver, frontend, and paper are all welcome.

---

## What we need

The highest-value contributions right now, roughly in priority order:

**Engine**
- Parallel CFR (external sampling) — the single-threaded solver is the main performance bottleneck
- Turn and river subgame solving — currently only flop is implemented
- Preflop solving with bucketed card abstraction
- Imperfect recall abstraction (potential significant speedup, theoretically interesting)
- Better equity estimation — replace Monte Carlo with a neural net equity model

**API**
- Job persistence across server restarts (currently in-memory only)
- Rate limiting and job queuing for the public instance
- Strategy profile compression (current JSON serialization is large)

**Frontend**
- Turn/river range explorer once multi-street solving lands
- CSV and image export for convergence curves (for paper figures)
- Mobile layout for the range grid
- Dark mode

**Paper**
- Reviewer feedback and additional experiments
- Extended related work coverage
- Additional board textures in the abstraction error analysis

---

## Getting started

### Fork and clone

```bash
git clone https://github.com/monolith-systematic/regret
cd regret
```

### Engine

```bash
cd engine
cargo build
cargo test
```

All tests should pass before opening a PR. The test suite covers the hand evaluator, regret matching, and a small toy game (Kuhn poker) with a known Nash equilibrium.

### Frontend

```bash
cd web
pnpm install
pnpm dev
```

The frontend runs against the local API by default (`http://localhost:8080`). Set `NEXT_PUBLIC_API_URL` to point elsewhere.

---

## Code standards

### Rust

- Run `cargo fmt` before committing
- Run `cargo clippy` and address all warnings — PRs with clippy warnings will not be merged
- Public functions and structs require doc comments
- New algorithmic code requires unit tests. For solver changes, include a Kuhn poker regression test verifying the known Nash equilibrium is still reached

### TypeScript

- `pnpm lint` must pass
- Components are functional with hooks; no class components
- Props are typed with interfaces, not inline type literals
- No `any` — use proper types or `unknown` with a guard

### General

- Commit messages are imperative present tense: "Add parallel CFR traversal", not "Added" or "Adding"
- One logical change per commit
- PRs should include a description of what changed and why, and a note on how to test it

---

## Pull request process

1. Open an issue first for anything non-trivial — this avoids duplicate work and lets us discuss the approach before you build it
2. Fork the repo, create a branch named `feature/your-thing` or `fix/your-thing`
3. Make your changes with tests
4. Open a PR against `main` with a clear description
5. Address review comments — we aim to review within a few days

For paper contributions (new experiments, extended analysis), open an issue with a brief proposal before writing. We want to make sure new experiments fit the paper's scope and that we have the computational budget to reproduce them.

---

## Reporting issues

Use GitHub Issues. For bugs, include:

- What you were trying to do
- What you expected to happen
- What actually happened
- Your OS, Rust version (`rustc --version`), and Node version (`node --version`)
- Relevant log output

For solver correctness issues (wrong Nash approximation, unexpected exploitability values), include the full game configuration you were running.

---

## Research collaboration

If you're working on related problems — game abstraction, CFR variants, poker AI, or connections to online learning theory — and want to collaborate on the paper or extend the framework, reach out directly. Open an issue tagged `research` with a brief description of what you're working on.

---

## License

By contributing, you agree that your contributions will be licensed under the project's MIT license.
