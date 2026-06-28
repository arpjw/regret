//! Quick CFR vs CFR+ probe at various scales. Used to validate that the
//! CFR+/Linear-CFR+ implementation does eventually dominate vanilla CFR on
//! HUNL flop subgames, and to identify the iteration count at which the
//! crossover happens.

use regret_engine::{CFRSolver, Card, CfrVariant, HunlFlop, HunlFlopConfig};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let k: u32 = args
        .first()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);
    let stack: f64 = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20.0);
    let iters: u64 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000);

    let config = HunlFlopConfig {
        board: [
            Card::parse("Ah").unwrap(),
            Card::parse("Ks").unwrap(),
            Card::parse("7d").unwrap(),
        ],
        starting_pot: 6.0,
        starting_stack: stack,
        bet_sizes: vec![0.25, 0.5, 0.75],
        card_buckets: k,
        equity_rollouts: 500,
        max_raises_per_street: 3,
        abstraction_seed: 0xC0FFEE,
    };
    println!("HUNL flop  K={k}  stack={stack}bb  iters={iters}");

    // Baseline: exploitability of the literally uniform strategy (no CFR
    // iterations at all — average_strategy falls through to 1/n for every
    // infoset).
    let baseline = CFRSolver::with_variant(HunlFlop::new(config.clone()), CfrVariant::Plus);
    let baseline_expl = baseline.exploitability();
    println!("uniform-strategy exploitability: {baseline_expl:.4}");

    let mut plus = CFRSolver::with_variant(HunlFlop::new(config.clone()), CfrVariant::Plus);
    plus.run(iters);
    let mut vanilla = CFRSolver::with_variant(HunlFlop::new(config), CfrVariant::Vanilla);
    vanilla.run(iters);

    println!("\niter\tCFR+\tCFR");
    for (a, b) in plus.history.points.iter().zip(vanilla.history.points.iter()) {
        println!("{}\t{:.4}\t{:.4}", a.iteration, a.exploitability, b.exploitability);
    }
}
