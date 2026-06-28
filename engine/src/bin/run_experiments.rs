//! Drives the four paper experiments and emits a single `results.json` plus
//! a per-experiment JSON for each so the matplotlib plotting script has
//! everything it needs without re-running the solver.
//!
//! Usage:
//!     cargo run --release -p regret-engine --bin run_experiments -- \
//!         --out paper/experiments/results.json \
//!         --experiments 1,2,3,4
//!
//! Each experiment can also be run in isolation (e.g. `--experiments 2`),
//! which is useful because experiment 2 with K=200 / 100 bb dominates wall
//! clock and benefits from being launched in the background.

use regret_engine::{
    proxy, CFRSolver, Card, CfrVariant, HunlFlop, HunlFlopConfig,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

// -----------------------------------------------------------------------------
// Shared types
// -----------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConvergencePoint {
    iteration: u64,
    exploitability: f64,
    elapsed_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConvergenceRun {
    label: String,
    config: ConfigSummary,
    points: Vec<ConvergencePoint>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConfigSummary {
    board: [String; 3],
    starting_pot: f64,
    starting_stack: f64,
    bet_sizes: Vec<f64>,
    card_buckets: u32,
    equity_rollouts: u32,
    max_raises_per_street: u8,
    abstraction_seed: u64,
}

impl From<&HunlFlopConfig> for ConfigSummary {
    fn from(c: &HunlFlopConfig) -> Self {
        Self {
            board: [
                format!("{}", c.board[0]),
                format!("{}", c.board[1]),
                format!("{}", c.board[2]),
            ],
            starting_pot: c.starting_pot,
            starting_stack: c.starting_stack,
            bet_sizes: c.bet_sizes.clone(),
            card_buckets: c.card_buckets,
            equity_rollouts: c.equity_rollouts,
            max_raises_per_street: c.max_raises_per_street,
            abstraction_seed: c.abstraction_seed,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AbstractionErrorPoint {
    label: String,
    card_buckets: u32,
    board: String,
    iterations: u64,
    abs_exploitability: f64,
    proxy_exploitability: f64,
    abstraction_error: f64,
    elapsed_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ActionAbstractionPoint {
    bet_sizes: Vec<String>,
    num_sizes: usize,
    iterations: u64,
    abs_exploitability: f64,
    proxy_exploitability: f64,
    abstraction_error: f64,
    elapsed_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct AllResults {
    experiment_1: Option<Vec<ConvergenceRun>>,
    experiment_2: Option<Vec<ConvergenceRun>>,
    experiment_3: Option<Vec<AbstractionErrorPoint>>,
    experiment_4: Option<Vec<ActionAbstractionPoint>>,
}

// -----------------------------------------------------------------------------
// Fixed configs
// -----------------------------------------------------------------------------

fn board_dry() -> [Card; 3] {
    [
        Card::parse("Ah").unwrap(),
        Card::parse("Ks").unwrap(),
        Card::parse("7d").unwrap(),
    ]
}
fn board_semiwet() -> [Card; 3] {
    [
        Card::parse("Jh").unwrap(),
        Card::parse("9h").unwrap(),
        Card::parse("8d").unwrap(),
    ]
}
fn board_wet() -> [Card; 3] {
    [
        Card::parse("9h").unwrap(),
        Card::parse("8h").unwrap(),
        Card::parse("7d").unwrap(),
    ]
}

fn base_config(board: [Card; 3], k: u32, sizes: Vec<f64>) -> HunlFlopConfig {
    HunlFlopConfig {
        board,
        starting_pot: 6.0,
        starting_stack: 100.0,
        bet_sizes: sizes,
        card_buckets: k,
        equity_rollouts: 500,
        max_raises_per_street: 3,
        abstraction_seed: 0xC0FFEE,
    }
}

// -----------------------------------------------------------------------------
// Experiment 1: CFR vs CFR+
// -----------------------------------------------------------------------------

fn experiment_1(iterations: u64) -> Vec<ConvergenceRun> {
    let config = base_config(board_dry(), 50, vec![0.25, 0.5, 0.75]);
    let summary = ConfigSummary::from(&config);

    let mut runs = Vec::new();
    for (label, variant) in [
        ("CFR+", CfrVariant::Plus),
        ("CFR", CfrVariant::Vanilla),
    ] {
        eprintln!("[exp1] running {label} for {iterations} iters ...");
        let t0 = Instant::now();
        let game = HunlFlop::new(config.clone());
        let mut solver = CFRSolver::with_variant(game, variant);
        solver.run(iterations);
        eprintln!(
            "[exp1] {label}: {} points, final expl = {:.4}, wall = {:?}",
            solver.history.points.len(),
            solver.history.points.last().map(|p| p.exploitability).unwrap_or(f64::NAN),
            t0.elapsed(),
        );
        runs.push(ConvergenceRun {
            label: label.to_string(),
            config: summary.clone(),
            points: solver
                .history
                .points
                .iter()
                .map(|p| ConvergencePoint {
                    iteration: p.iteration,
                    exploitability: p.exploitability,
                    elapsed_ms: p.elapsed_ms,
                })
                .collect(),
        });
    }
    runs
}

// -----------------------------------------------------------------------------
// Experiment 2: CFR+ at varying K
// -----------------------------------------------------------------------------

fn experiment_2(iterations: u64, ks: &[u32]) -> Vec<ConvergenceRun> {
    let mut runs = Vec::new();
    for &k in ks {
        let config = base_config(board_dry(), k, vec![0.25, 0.5, 0.75]);
        let summary = ConfigSummary::from(&config);
        eprintln!("[exp2] K={k} running for {iterations} iters ...");
        let t0 = Instant::now();
        let game = HunlFlop::new(config);
        let mut solver = CFRSolver::with_variant(game, CfrVariant::Plus);
        solver.run(iterations);
        eprintln!(
            "[exp2] K={k}: final expl = {:.4}, wall = {:?}",
            solver.history.points.last().map(|p| p.exploitability).unwrap_or(f64::NAN),
            t0.elapsed(),
        );
        runs.push(ConvergenceRun {
            label: format!("K={k}"),
            config: summary,
            points: solver
                .history
                .points
                .iter()
                .map(|p| ConvergencePoint {
                    iteration: p.iteration,
                    exploitability: p.exploitability,
                    elapsed_ms: p.elapsed_ms,
                })
                .collect(),
        });
    }
    runs
}

// -----------------------------------------------------------------------------
// Experiment 3: card-abstraction error across boards
// -----------------------------------------------------------------------------

fn experiment_3(
    ks: &[u32],
    proxy_k: u32,
    boards: &[(&str, [Card; 3])],
    target_expl_frac: f64,
    max_iterations: u64,
) -> Vec<AbstractionErrorPoint> {
    let mut out = Vec::new();
    for (board_label, board) in boards {
        for &k in ks {
            let config = base_config(*board, k, vec![0.25, 0.5, 0.75]);
            let target_expl = target_expl_frac * config.starting_pot;
            eprintln!(
                "[exp3] board={board_label} K={k}: solving to expl<{:.4} (cap {max_iterations} iters) ...",
                target_expl
            );
            let t0 = Instant::now();
            let game = HunlFlop::new(config.clone());
            let mut solver = CFRSolver::with_variant(game, CfrVariant::Plus);
            let (iters_used, abs_expl) =
                solve_to_threshold(&mut solver, target_expl, max_iterations);
            eprintln!(
                "[exp3] board={board_label} K={k}: solved in {iters_used} iters, abs_expl={abs_expl:.4} ({:?}); evaluating in K={proxy_k} proxy ...",
                t0.elapsed()
            );
            let proxy_config = base_config(*board, proxy_k, vec![0.25, 0.5, 0.75]);
            let t1 = Instant::now();
            let proxy_expl = proxy::cross_card_exploitability(&solver, proxy_config);
            eprintln!(
                "[exp3] board={board_label} K={k}: proxy_expl={proxy_expl:.4} (BR took {:?})",
                t1.elapsed()
            );
            out.push(AbstractionErrorPoint {
                label: format!("K={k}"),
                card_buckets: k,
                board: board_label.to_string(),
                iterations: iters_used,
                abs_exploitability: abs_expl,
                proxy_exploitability: proxy_expl,
                abstraction_error: (proxy_expl - abs_expl).max(0.0),
                elapsed_ms: t0.elapsed().as_millis() as u64,
            });
        }
    }
    out
}

fn solve_to_threshold(
    solver: &mut CFRSolver<HunlFlop>,
    target: f64,
    cap: u64,
) -> (u64, f64) {
    // Run in 50-iter blocks and break once the latest recorded exploitability
    // is below `target`. Returns (iterations_used, final_exploitability).
    let block = 50u64;
    let mut total = 0u64;
    let mut last_expl = f64::INFINITY;
    while total < cap {
        let next = block.min(cap - total);
        solver.run(next);
        total += next;
        last_expl = solver
            .history
            .points
            .last()
            .map(|p| p.exploitability)
            .unwrap_or(last_expl);
        if last_expl <= target {
            break;
        }
    }
    (total, last_expl)
}

// -----------------------------------------------------------------------------
// Experiment 4: action-abstraction error
// -----------------------------------------------------------------------------

fn experiment_4(
    size_configs: &[Vec<f64>],
    proxy_sizes: Vec<f64>,
    target_expl_frac: f64,
    max_iterations: u64,
) -> Vec<ActionAbstractionPoint> {
    let mut out = Vec::new();
    for sizes in size_configs {
        let config = base_config(board_dry(), 50, sizes.clone());
        let target_expl = target_expl_frac * config.starting_pot;
        eprintln!(
            "[exp4] sizes={sizes:?}: solving to expl<{:.4} (cap {max_iterations}) ...",
            target_expl
        );
        let t0 = Instant::now();
        let game = HunlFlop::new(config.clone());
        let mut solver = CFRSolver::with_variant(game, CfrVariant::Plus);
        let (iters_used, abs_expl) =
            solve_to_threshold(&mut solver, target_expl, max_iterations);
        eprintln!(
            "[exp4] sizes={sizes:?}: solved in {iters_used} iters, abs_expl={abs_expl:.4} ({:?}); evaluating in proxy {proxy_sizes:?} ...",
            t0.elapsed()
        );
        let proxy_config = base_config(board_dry(), 50, proxy_sizes.clone());
        let t1 = Instant::now();
        let proxy_expl = proxy::cross_action_exploitability(&solver, proxy_config);
        eprintln!(
            "[exp4] sizes={sizes:?}: proxy_expl={proxy_expl:.4} (BR took {:?})",
            t1.elapsed()
        );
        out.push(ActionAbstractionPoint {
            bet_sizes: sizes
                .iter()
                .map(|x| format!("{:.2}", x))
                .collect(),
            num_sizes: sizes.len(),
            iterations: iters_used,
            abs_exploitability: abs_expl,
            proxy_exploitability: proxy_expl,
            abstraction_error: (proxy_expl - abs_expl).max(0.0),
            elapsed_ms: t0.elapsed().as_millis() as u64,
        });
    }
    out
}

// -----------------------------------------------------------------------------
// Driver
// -----------------------------------------------------------------------------

fn parse_args() -> (PathBuf, Vec<u32>, BTreeMap<String, String>) {
    let mut out = PathBuf::from("paper/experiments/results.json");
    let mut experiments: Vec<u32> = vec![1, 2, 3, 4];
    let mut overrides: BTreeMap<String, String> = BTreeMap::new();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                out = PathBuf::from(&args[i + 1]);
                i += 2;
            }
            "--experiments" => {
                experiments = args[i + 1]
                    .split(',')
                    .map(|s| s.trim().parse::<u32>().expect("experiments are ints"))
                    .collect();
                i += 2;
            }
            "--ks" => {
                overrides.insert("ks".into(), args[i + 1].clone());
                i += 2;
            }
            "--iters" => {
                overrides.insert("iters".into(), args[i + 1].clone());
                i += 2;
            }
            "--max-iters" => {
                overrides.insert("max_iters".into(), args[i + 1].clone());
                i += 2;
            }
            other => panic!("unknown arg: {other}"),
        }
    }
    (out, experiments, overrides)
}

fn ks_override(overrides: &BTreeMap<String, String>, default: Vec<u32>) -> Vec<u32> {
    overrides
        .get("ks")
        .map(|s| s.split(',').map(|x| x.trim().parse().unwrap()).collect())
        .unwrap_or(default)
}

fn iters_override(overrides: &BTreeMap<String, String>, key: &str, default: u64) -> u64 {
    overrides
        .get(key)
        .map(|s| s.parse().unwrap())
        .unwrap_or(default)
}

fn main() {
    let (out_path, experiments, overrides) = parse_args();
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let iterations = iters_override(&overrides, "iters", 1000);
    let max_iterations = iters_override(&overrides, "max_iters", 1000);

    // Load previous partial results if present, so individual experiments
    // can be appended without losing prior work.
    let mut results: AllResults = if out_path.exists() {
        let text = std::fs::read_to_string(&out_path).unwrap_or_default();
        serde_json::from_str(&text).unwrap_or_default()
    } else {
        AllResults::default()
    };

    for exp in &experiments {
        match *exp {
            1 => {
                let r = experiment_1(iterations);
                results.experiment_1 = Some(r);
                save_partial(&out_path, &results);
            }
            2 => {
                let ks = ks_override(&overrides, vec![10, 25, 50, 100, 200]);
                let r = experiment_2(iterations, &ks);
                results.experiment_2 = Some(r);
                save_partial(&out_path, &results);
            }
            3 => {
                let ks = ks_override(&overrides, vec![10, 25, 50, 100]);
                let boards = [
                    ("dry", board_dry()),
                    ("semi-wet", board_semiwet()),
                    ("wet", board_wet()),
                ];
                let r = experiment_3(&ks, 200, &boards, 0.01, max_iterations);
                results.experiment_3 = Some(r);
                save_partial(&out_path, &results);
            }
            4 => {
                let size_configs: Vec<Vec<f64>> = vec![
                    vec![0.5],
                    vec![0.25, 0.75],
                    vec![0.25, 0.5, 0.75],
                    vec![0.25, 0.5, 0.75, 1.0],
                    vec![0.25, 0.5, 0.75, 1.0, 2.0],
                ];
                // The 5-size config is treated as the proxy ({0.25, 0.5,
                // 0.75, 1.0, 2.0} — 2.0× pot is our discrete stand-in for
                // all-in since the game tree's AllIn action is added
                // automatically on top of the configured sizes whenever it
                // doesn't already coincide with one of them).
                let proxy = vec![0.25, 0.5, 0.75, 1.0, 2.0];
                let r = experiment_4(&size_configs, proxy, 0.02, max_iterations);
                results.experiment_4 = Some(r);
                save_partial(&out_path, &results);
            }
            other => eprintln!("unknown experiment {other}, skipping"),
        }
    }

    save_partial(&out_path, &results);
    eprintln!("[run_experiments] wrote {}", out_path.display());
}

fn save_partial(out: &Path, results: &AllResults) {
    let text = serde_json::to_string_pretty(results).expect("serialize");
    std::fs::write(out, text).expect("write results");
}
