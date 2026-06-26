//! Full-config HUNL flop solve test at K = 50 buckets, 3 bet sizes.
//!
//! Lives in `tests/` (an integration target) rather than `src/`, so a
//! routine `cargo test --lib` skips it; `cargo test` (default, all
//! targets) and `cargo test --release` include it. Profiling at this
//! config shows ~3 min in release mode for the full 500 iterations + 10
//! best-response evaluations — see `tests/hunl_profile.rs` for the
//! breakdown. The threshold is intentionally loose; the assertion's
//! purpose is to catch regressions where CFR+ stops converging at all,
//! not to pin down a tight optimality bound.

use std::time::Instant;

use regret_engine::{CFRSolver, Card, HunlFlop, HunlFlopConfig};

#[test]
fn hunl_k50_full_solve_converges() {
    // 20 bb stacks: shallower than full 100 bb but matches the audit's
    // section-6 production config, which is what the < 20% / 500-iter
    // budget was calibrated against. Deep-stack (100 bb) solves at K=50
    // converge to ~28% of pot at the same iteration count; if you need
    // that regime, bump iters or accept a looser threshold.
    let config = HunlFlopConfig {
        board: [
            Card::parse("Ah").unwrap(),
            Card::parse("Ks").unwrap(),
            Card::parse("7d").unwrap(),
        ],
        starting_pot: 6.0,
        starting_stack: 20.0,
        bet_sizes: vec![0.25, 0.5, 0.75],
        card_buckets: 50,
        equity_rollouts: 500,
        max_raises_per_street: 3,
        abstraction_seed: 0xC0FFEE,
    };

    let starting_pot = config.starting_pot;
    let game = HunlFlop::new(config);
    let mut solver = CFRSolver::new(game);

    let t0 = Instant::now();
    solver.run(500);
    let elapsed = t0.elapsed();
    let expl = solver.exploitability();
    let expl_pct_of_pot = expl / starting_pot;

    println!(
        "[hunl_k50_full_solve] 500 iters in {:?}, exploitability = {:.4} bb ({:.2}% of pot)",
        elapsed,
        expl,
        expl_pct_of_pot * 100.0
    );

    // Loose threshold per the spec: exploitability below 20% of the
    // starting pot. CFR+ on the audit-equivalent K=10 / 2-size config
    // reaches ~7% of pot at 200 iters; K=50 / 3 sizes / 500 iters has
    // plenty of headroom.
    assert!(
        expl_pct_of_pot < 0.20,
        "exploitability {:.4} bb is >= 20% of pot ({:.4} bb)",
        expl,
        0.20 * starting_pot
    );

    // Wall-clock sanity check: catch the case where a regression silently
    // re-introduces card-pair chance enumeration (which would take hours).
    assert!(
        elapsed <= std::time::Duration::from_secs(600),
        "500-iter solve took {:?} — far over the 5-min target; profile with \
         `cargo test -p regret-engine --release --test hunl_profile -- \
         --ignored --nocapture`",
        elapsed
    );
}
