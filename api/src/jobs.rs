//! Solver job orchestration.
//!
//! A `SolverJob` owns a CFR+ solver running on a Tokio task. It publishes
//! progress events (`ConvergencePoint`) on a broadcast channel that SSE
//! subscribers tee from, and stashes the final infoset → average strategy
//! map plus the `HunlFlop` game instance on completion so the
//! `/strategy/:infoset` and `/strategy_for_hand` endpoints can serve them.

use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use regret_engine::{
    AbstractAction, CFRSolver, Card, ConvergenceHistory, HunlFlop, HunlFlopConfig, Suit,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

/// JSON shape of a `POST /solve` body.
#[derive(Clone, Debug, Deserialize)]
pub struct SolveRequest {
    /// Optional human label echoed back in logs; not used by the solver.
    #[serde(default)]
    #[allow(dead_code)]
    pub matchup: Option<String>,
    pub stack_depth: f64,
    pub board: Vec<String>,
    pub bet_sizes: Vec<f64>,
    pub card_buckets: u32,
    pub max_iterations: u64,
    #[serde(default = "default_starting_pot")]
    pub starting_pot: f64,
    #[serde(default = "default_max_raises")]
    pub max_raises_per_street: u8,
    #[serde(default = "default_equity_rollouts")]
    pub equity_rollouts: u32,
}

fn default_starting_pot() -> f64 {
    6.0
}

fn default_max_raises() -> u8 {
    3
}

fn default_equity_rollouts() -> u32 {
    500
}

impl SolveRequest {
    pub fn to_config(&self) -> Result<HunlFlopConfig, String> {
        if self.board.len() != 3 {
            return Err("board must have exactly 3 cards".into());
        }
        let mut parsed = [Card::parse("Ah").unwrap(); 3];
        for (i, s) in self.board.iter().enumerate() {
            parsed[i] = Card::parse(s).ok_or_else(|| format!("invalid card: {s}"))?;
        }
        if parsed[0] == parsed[1] || parsed[0] == parsed[2] || parsed[1] == parsed[2] {
            return Err("board cards must be distinct".into());
        }
        if !self.stack_depth.is_finite() || self.stack_depth <= 0.0 {
            return Err("stack_depth must be finite and positive".into());
        }
        if !self.starting_pot.is_finite() || self.starting_pot <= 0.0 {
            return Err("starting_pot must be finite and positive".into());
        }
        if self.bet_sizes.len() > u8::MAX as usize {
            return Err("bet_sizes cannot have more than 255 entries".into());
        }
        if self
            .bet_sizes
            .iter()
            .any(|size| !size.is_finite() || *size <= 0.0)
        {
            return Err("bet_sizes must be finite and positive".into());
        }
        if self.card_buckets == 0 {
            return Err("card_buckets must be positive".into());
        }
        if self.equity_rollouts == 0 {
            return Err("equity_rollouts must be positive".into());
        }
        if self.max_iterations == 0 {
            return Err("max_iterations must be positive".into());
        }
        Ok(HunlFlopConfig {
            board: parsed,
            starting_pot: self.starting_pot,
            starting_stack: self.stack_depth,
            bet_sizes: self.bet_sizes.clone(),
            card_buckets: self.card_buckets,
            equity_rollouts: self.equity_rollouts,
            max_raises_per_street: self.max_raises_per_street,
            abstraction_seed: 0x5EED,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Running,
    Done,
    Error { message: String },
}

#[derive(Clone, Debug, Serialize)]
pub struct ProgressEvent {
    pub iteration: u64,
    pub exploitability: f64,
    pub elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
}

pub struct JobHandle {
    pub status: RwLock<JobStatus>,
    pub progress: broadcast::Sender<ProgressEvent>,
    pub history: RwLock<ConvergenceHistory>,
    pub strategies: RwLock<HashMap<u64, Vec<f64>>>,
    /// Shared game instance — populated once the solver task installs it so
    /// the API can resolve hand labels to bucket+infoset keys using the same
    /// `CardAbstraction` and hashing the solver used.
    pub game: RwLock<Option<Arc<HunlFlop>>>,
}

impl JobHandle {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            status: RwLock::new(JobStatus::Pending),
            progress: tx,
            history: RwLock::new(ConvergenceHistory::default()),
            strategies: RwLock::new(HashMap::new()),
            game: RwLock::new(None),
        }
    }
}

pub type JobMap = DashMap<String, Arc<JobHandle>>;

pub fn spawn_solver(jobs: Arc<JobMap>, config: HunlFlopConfig, max_iterations: u64) -> String {
    let job_id = Uuid::new_v4().to_string()[..8].to_string();
    let handle = Arc::new(JobHandle::new());
    jobs.insert(job_id.clone(), handle.clone());

    let id_for_task = job_id.clone();
    tokio::spawn(async move {
        run_solver(handle, config, max_iterations, id_for_task).await;
    });

    job_id
}

async fn run_solver(
    handle: Arc<JobHandle>,
    config: HunlFlopConfig,
    iterations: u64,
    id: String,
) {
    *handle.status.write().await = JobStatus::Running;
    tracing::info!(job = %id, "solver starting");

    let handle_clone = handle.clone();
    let result = tokio::task::spawn_blocking(move || run_solver_blocking(handle_clone, config, iterations))
        .await;

    match result {
        Ok(Ok(_)) => {
            *handle.status.write().await = JobStatus::Done;
            tracing::info!(job = %id, "solver done");
        }
        Ok(Err(e)) => {
            *handle.status.write().await = JobStatus::Error { message: e.clone() };
            tracing::error!(job = %id, error = %e, "solver errored");
        }
        Err(e) => {
            let msg = format!("solver task panicked: {e}");
            *handle.status.write().await = JobStatus::Error { message: msg.clone() };
            tracing::error!(job = %id, error = %msg, "solver panicked");
        }
    }
}

fn run_solver_blocking(
    handle: Arc<JobHandle>,
    config: HunlFlopConfig,
    iterations: u64,
) -> Result<(), String> {
    let game = Arc::new(HunlFlop::new(config));
    // Publish the game now so concurrent /strategy_for_hand requests can
    // resolve hands using the shared bucket cache even before the solver
    // finishes — they'll just see uniform strategies until completion.
    {
        let mut g = handle.game.blocking_write();
        *g = Some(game.clone());
    }

    let mut solver = CFRSolver::new(game);
    let start = std::time::Instant::now();

    let report_every: u64 = 50;
    let mut next_report = report_every;
    for t in 1..=iterations {
        cfr_one_iter(&mut solver);
        if t == next_report || t == iterations {
            let expl = solver.exploitability();
            let elapsed_ms = start.elapsed().as_millis() as u64;
            solver.history.record(t, expl, elapsed_ms);
            let evt = ProgressEvent {
                iteration: t,
                exploitability: expl,
                elapsed_ms,
                done: if t == iterations { Some(true) } else { None },
            };
            let _ = handle.progress.send(evt);
            next_report += report_every;
        }
    }

    // Snapshot all average strategies for /strategy lookups.
    let mut strategies = HashMap::new();
    for entry in solver.data.iter() {
        let key = *entry.key();
        let data = entry.value();
        strategies.insert(key, data.average_strategy());
    }
    {
        let mut s = handle.strategies.blocking_write();
        *s = strategies;
    }
    {
        let mut h = handle.history.blocking_write();
        *h = solver.history.clone();
    }
    Ok(())
}

/// One CFR+ iteration (alternating updates over both players).
fn cfr_one_iter(solver: &mut CFRSolver<Arc<HunlFlop>>) {
    // Use the engine's run() helper for a single iteration, then drop its
    // recorded point so the api reports on its own cadence.
    let pre = solver.history.points.len();
    solver.run(1);
    if solver.history.points.len() > pre {
        solver.history.points.truncate(pre);
    }
}

// ---------------------------------------------------------------------------
// Hand label & action history parsing
// ---------------------------------------------------------------------------

/// Parses a hand label like `AA`, `AKs`, or `72o` into a canonical hole pair
/// that does not overlap with `board`. Suit choice is arbitrary among the
/// remaining free suits — what matters for bucketing is rank + suitedness.
pub fn parse_hand_label(label: &str, board: &[Card; 3]) -> Result<[Card; 2], String> {
    let label = label.trim();
    let bytes = label.as_bytes();
    if bytes.len() != 2 && bytes.len() != 3 {
        return Err(format!("expected 2- or 3-char hand label, got `{label}`"));
    }

    let r1 = parse_rank(bytes[0])
        .ok_or_else(|| format!("invalid rank `{}` in `{label}`", bytes[0] as char))?;
    let r2 = parse_rank(bytes[1])
        .ok_or_else(|| format!("invalid rank `{}` in `{label}`", bytes[1] as char))?;

    let kind = if bytes.len() == 3 {
        match bytes[2] {
            b's' | b'S' => HandKind::Suited,
            b'o' | b'O' => HandKind::Offsuit,
            other => return Err(format!("expected `s` or `o`, got `{}`", other as char)),
        }
    } else {
        if r1 != r2 {
            return Err(format!(
                "2-char label `{label}` must be a pair (e.g. `AA`); use `{label}s`/`{label}o` for non-pairs"
            ));
        }
        HandKind::Pair
    };

    let free_r1 = free_suits(r1, board);
    let free_r2 = free_suits(r2, board);
    if free_r1.is_empty() || free_r2.is_empty() {
        return Err(format!("no free suits for one of the ranks in `{label}`"));
    }

    match kind {
        HandKind::Pair => {
            // r1 == r2: need two distinct free suits.
            if free_r1.len() < 2 {
                return Err(format!("not enough free suits for pair `{label}`"));
            }
            Ok([Card::new(r1, free_r1[0]), Card::new(r1, free_r1[1])])
        }
        HandKind::Suited => {
            // Need a common free suit.
            let shared = free_r1
                .iter()
                .copied()
                .find(|s| free_r2.contains(s))
                .ok_or_else(|| format!("no common free suit for suited `{label}`"))?;
            Ok([Card::new(r1, shared), Card::new(r2, shared)])
        }
        HandKind::Offsuit => {
            // Need distinct suits.
            let s1 = free_r1[0];
            let s2 = free_r2
                .iter()
                .copied()
                .find(|s| *s != s1)
                .ok_or_else(|| format!("no distinct free suit for offsuit `{label}`"))?;
            Ok([Card::new(r1, s1), Card::new(r2, s2)])
        }
    }
}

enum HandKind {
    Pair,
    Suited,
    Offsuit,
}

fn parse_rank(b: u8) -> Option<u8> {
    match b {
        b'2'..=b'9' => Some(b - b'0'),
        b'T' | b't' => Some(10),
        b'J' | b'j' => Some(11),
        b'Q' | b'q' => Some(12),
        b'K' | b'k' => Some(13),
        b'A' | b'a' => Some(14),
        _ => None,
    }
}

fn free_suits(rank: u8, board: &[Card; 3]) -> Vec<Suit> {
    const ALL: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];
    ALL.into_iter()
        .filter(|s| !board.iter().any(|c| c.rank == rank && c.suit == *s))
        .collect()
}

/// Parses an action history string into a vector of `AbstractAction`s.
///
/// Format: dot-separated tokens. Empty string yields the empty history.
///   - `check`, `fold`, `call`
///   - `betN`, `raiseN` where `N` is the size-bucket index (0..=255). Use
///     `betA` / `raiseA` for the all-in sentinel (255).
pub fn parse_action_history(s: &str) -> Result<Vec<AbstractAction>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(Vec::new());
    }
    s.split('.').map(parse_one_action).collect()
}

fn parse_one_action(tok: &str) -> Result<AbstractAction, String> {
    match tok {
        "check" => Ok(AbstractAction::Check),
        "fold" => Ok(AbstractAction::Fold),
        "call" => Ok(AbstractAction::Call),
        _ => {
            if let Some(rest) = tok.strip_prefix("bet") {
                Ok(AbstractAction::Bet(parse_size_idx(rest)?))
            } else if let Some(rest) = tok.strip_prefix("raise") {
                Ok(AbstractAction::Raise(parse_size_idx(rest)?))
            } else {
                Err(format!("unknown action token `{tok}`"))
            }
        }
    }
}

fn parse_size_idx(s: &str) -> Result<u8, String> {
    if s == "A" || s == "a" {
        return Ok(u8::MAX);
    }
    s.parse::<u8>()
        .map_err(|e| format!("invalid size index `{s}`: {e}"))
}

/// Active player on the flop given the number of actions already taken.
/// The HunlFlop initial state has OOP (player 1) to act first; players
/// alternate thereafter.
pub fn active_player_after(history_len: usize) -> usize {
    if history_len % 2 == 0 {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regret_engine::Card;

    fn board() -> [Card; 3] {
        [
            Card::parse("Ah").unwrap(),
            Card::parse("Ks").unwrap(),
            Card::parse("7d").unwrap(),
        ]
    }

    fn valid_request() -> SolveRequest {
        SolveRequest {
            matchup: None,
            stack_depth: 50.0,
            board: vec!["Ah".into(), "Ks".into(), "7d".into()],
            bet_sizes: vec![0.5, 1.0],
            card_buckets: 4,
            max_iterations: 50,
            starting_pot: 6.0,
            max_raises_per_street: 1,
            equity_rollouts: 30,
        }
    }

    #[test]
    fn reject_invalid_solve_requests_before_spawning() {
        let mut req = valid_request();
        assert!(req.to_config().is_ok());

        req.board[1] = "Ah".into();
        assert_eq!(req.to_config().unwrap_err(), "board cards must be distinct");
        req = valid_request();

        req.stack_depth = 0.0;
        assert!(req.to_config().is_err());
        req.stack_depth = f64::INFINITY;
        assert!(req.to_config().is_err());
        req = valid_request();

        req.starting_pot = -1.0;
        assert!(req.to_config().is_err());
        req = valid_request();

        req.bet_sizes = vec![0.5, f64::NAN];
        assert!(req.to_config().is_err());
        req.bet_sizes = vec![0.5; 256];
        assert!(req.to_config().is_err());
        req = valid_request();

        req.card_buckets = 0;
        assert!(req.to_config().is_err());
        req = valid_request();

        req.equity_rollouts = 0;
        assert!(req.to_config().is_err());
        req = valid_request();

        req.max_iterations = 0;
        assert!(req.to_config().is_err());
    }

    #[test]
    fn parse_aa_skips_board_suit() {
        let hole = parse_hand_label("AA", &board()).unwrap();
        // Ah is on the board, so neither A should be hearts.
        assert!(hole.iter().all(|c| c.rank == 14));
        assert!(hole.iter().all(|c| c.suit != Suit::Hearts));
        assert_ne!(hole[0].suit, hole[1].suit);
    }

    #[test]
    fn parse_aks_chooses_shared_free_suit() {
        let hole = parse_hand_label("AKs", &board()).unwrap();
        assert_eq!(hole[0].rank, 14);
        assert_eq!(hole[1].rank, 13);
        assert_eq!(hole[0].suit, hole[1].suit);
        // Ah and Ks are on the board so the shared suit can't be h or s.
        assert!(matches!(hole[0].suit, Suit::Clubs | Suit::Diamonds));
    }

    #[test]
    fn parse_offsuit_pair_uses_distinct_suits() {
        let hole = parse_hand_label("72o", &board()).unwrap();
        assert_eq!(hole[0].rank, 7);
        assert_eq!(hole[1].rank, 2);
        assert_ne!(hole[0].suit, hole[1].suit);
    }

    #[test]
    fn parse_history_round_trip() {
        let h = parse_action_history("check.bet0.call").unwrap();
        assert_eq!(h.len(), 3);
        assert!(matches!(h[0], AbstractAction::Check));
        assert!(matches!(h[1], AbstractAction::Bet(0)));
        assert!(matches!(h[2], AbstractAction::Call));
    }

    #[test]
    fn empty_history_is_empty_vec() {
        assert!(parse_action_history("").unwrap().is_empty());
    }

    #[test]
    fn active_player_flips_each_action() {
        assert_eq!(active_player_after(0), 1); // OOP first
        assert_eq!(active_player_after(1), 0);
        assert_eq!(active_player_after(2), 1);
    }

    #[test]
    fn resolver_returns_seeded_strategy_for_aa_on_ak7() {
        // Integration test: seed a job's strategies with a known non-uniform
        // distribution at the (AA, AK7 board, empty history, player=OOP)
        // infoset, and verify the resolver finds it.
        let cfg = HunlFlopConfig {
            board: board(),
            starting_pot: 6.0,
            starting_stack: 50.0,
            bet_sizes: vec![0.5, 1.0],
            card_buckets: 4,
            equity_rollouts: 30,
            max_raises_per_street: 1,
            abstraction_seed: 1,
        };
        let game = Arc::new(HunlFlop::new(cfg));

        let aa = parse_hand_label("AA", &board()).unwrap();
        let bucket = game.bucket_for(&aa);
        let key = HunlFlop::infoset_key_from_components(bucket, &[], 1);

        // AA on AK7 has very high equity; the bucket should be in the top
        // half of the 4-bucket abstraction (>= 2).
        assert!(
            bucket >= 2,
            "AA on AK7 should bucket into the top half; got bucket {bucket}"
        );

        // Seed the strategy map with a known non-uniform distribution.
        let seeded: Vec<f64> = vec![0.05, 0.15, 0.30, 0.50];
        let handle = Arc::new(JobHandle::new());
        {
            let mut s = handle.strategies.blocking_write();
            s.insert(key, seeded.clone());
            let mut g = handle.game.blocking_write();
            *g = Some(game.clone());
            let mut st = handle.status.blocking_write();
            *st = JobStatus::Done;
        }

        // Now perform the same resolution the API endpoint will do.
        let resolved_game = handle.game.blocking_read().clone().unwrap();
        let resolved_bucket = resolved_game.bucket_for(&aa);
        let resolved_key =
            HunlFlop::infoset_key_from_components(resolved_bucket, &[], active_player_after(0));
        let strategies = handle.strategies.blocking_read();
        let probs = strategies.get(&resolved_key).cloned().expect("found");

        assert_eq!(probs, seeded, "resolver must return the seeded distribution");
        // Verify it is, in fact, non-uniform — guards against the resolver
        // accidentally falling back to a default.
        let mean = probs.iter().sum::<f64>() / probs.len() as f64;
        let variance: f64 = probs.iter().map(|p| (p - mean).powi(2)).sum();
        assert!(
            variance > 1e-6,
            "resolved strategy must be non-uniform: {:?}",
            probs
        );
    }
}
