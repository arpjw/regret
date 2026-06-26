//! Profile probe for the HUNL flop solver at production-scale abstraction.
//!
//! Run with:
//!     cargo test -p regret-engine --release --test hunl_profile -- --ignored --nocapture
//!
//! Reports: bucket-cache + winrate-matrix precomputation, per-iteration CFR
//! cost (averaged over a small probe), best-response cost, and a projected
//! wall-clock for a 500-iteration solve.

use std::time::Instant;

use regret_engine::{CFRSolver, Card, HunlFlop, HunlFlopConfig};

fn production_config() -> HunlFlopConfig {
    HunlFlopConfig {
        board: [
            Card::parse("Ah").unwrap(),
            Card::parse("Ks").unwrap(),
            Card::parse("7d").unwrap(),
        ],
        starting_pot: 6.0,
        starting_stack: 100.0,
        bet_sizes: vec![0.25, 0.5, 0.75],
        card_buckets: 50,
        equity_rollouts: 500,
        max_raises_per_street: 3,
        abstraction_seed: 0xC0FFEE,
    }
}

#[test]
#[ignore = "profile probe — run with --ignored --nocapture"]
fn profile_k50_3sizes() {
    println!("\n==== HUNL K=50 / 3 bet sizes profile ====");
    let config = production_config();
    println!(
        "config: K={}, bet_sizes={:?}, max_raises={}, stack={}, pot={}, equity_rollouts={}",
        config.card_buckets,
        config.bet_sizes,
        config.max_raises_per_street,
        config.starting_stack,
        config.starting_pot,
        config.equity_rollouts
    );

    // 1) Construction is cheap; the heavy work (bucket assignments +
    //    K² winrate matrix) is triggered lazily on first table access.
    //    Force it by querying any bucket.
    let t0 = Instant::now();
    let game = HunlFlop::new(config);
    let probe_hole = [Card::parse("As").unwrap(), Card::parse("Ad").unwrap()];
    let _bucket = game.bucket_for(&probe_hole);
    let setup_time = t0.elapsed();
    println!("precompute (bucket cache + winrate matrix): {:?}", setup_time);

    // 2) Per-iteration CFR cost averaged over a small probe.
    let mut solver = CFRSolver::new(game);
    let probe_iters: u64 = 5;
    let t1 = Instant::now();
    solver.run(probe_iters);
    let probe_total = t1.elapsed();
    let per_iter = probe_total / probe_iters as u32;
    println!(
        "first {} iters wall: {:?}  → per-iter avg: {:?}",
        probe_iters, probe_total, per_iter
    );

    // 3) Best-response cost.
    let t2 = Instant::now();
    let expl = solver.exploitability();
    let br_time = t2.elapsed();
    println!("one exploitability (BR for both players): {:?}", br_time);
    println!("exploitability after {} iters: {:.4}", probe_iters, expl);

    // 4) Project to 500 iters: CFR + one BR every 50 iters.
    let projected_cfr = per_iter * 500;
    let projected_br = br_time * 10;
    let projected_total = setup_time + projected_cfr + projected_br;
    println!("\nprojected 500-iter solve:");
    println!("  CFR        : {:?}", projected_cfr);
    println!("  10 × BR    : {:?}", projected_br);
    println!("  precompute : {:?}", setup_time);
    println!("  -----------");
    println!("  total      : {:?}", projected_total);
    println!(
        "  under 5 min: {}",
        projected_total <= std::time::Duration::from_secs(300)
    );
}
