//! HUNL flop subgame implementing the `Game` trait.
//!
//! The subgame is parameterized by a fixed flop, starting pot, stack depth,
//! and abstraction config. Chance is collapsed into a single "deal hole
//! buckets" node at the root that branches over the joint distribution of
//! `(bucket_p0, bucket_p1)` pairs computed from the residual deck.
//!
//! Because card abstraction is an equivalence relation on hole pairs,
//! enumerating by bucket pair is mathematically equivalent to enumerating
//! by card pair for the abstracted game — the per-bucket-pair showdown
//! value is the average of the underlying card-pair showdown values, and
//! every infoset is indexed only by `(bucket, history, player)`. The
//! payoff for this aggregation is a ~10,000× chance fan-out reduction
//! (1.27M card pairs → K² bucket pairs).
//!
//! Action abstraction discretizes the continuous bet/raise space into a
//! configured set of pot-fraction sizes plus all-in. Raises per street are
//! capped to keep the tree tractable.

use crate::abstraction::{ActionAbstraction, CardAbstraction};
use crate::cfr::Game;
use crate::eval::{HandEvaluator, HandRank};
use crate::game::{AbstractAction, Card, Suit};
use serde::{Deserialize, Serialize};
use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HunlFlopConfig {
    pub board: [Card; 3],
    pub starting_pot: f64,
    pub starting_stack: f64,
    pub bet_sizes: Vec<f64>,
    pub card_buckets: u32,
    pub equity_rollouts: u32,
    pub max_raises_per_street: u8,
    pub abstraction_seed: u64,
}

impl Default for HunlFlopConfig {
    fn default() -> Self {
        Self {
            board: [
                Card::parse("Ah").unwrap(),
                Card::parse("Ks").unwrap(),
                Card::parse("7d").unwrap(),
            ],
            starting_pot: 6.0,
            starting_stack: 97.0,
            bet_sizes: vec![0.50, 1.0],
            card_buckets: 8,
            equity_rollouts: 200,
            max_raises_per_street: 3,
            abstraction_seed: 0x5EED,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LastAct {
    None,
    Check,
    Bet(f64),
    Raise(f64),
    Call,
}

#[derive(Clone, Debug)]
pub struct HunlFlopState {
    /// `(bucket_p0, bucket_p1)` once chance has resolved.
    pub buckets: Option<(u32, u32)>,
    pub stacks: [f64; 2],
    pub committed: [f64; 2],
    pub facing: f64,
    pub to_act: usize,
    pub history: Vec<AbstractAction>,
    pub raises_used: u8,
    pub last: LastAct,
    pub terminal: Option<Terminal>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Terminal {
    Fold { folder: usize },
    Showdown,
}

/// Precomputed game-level tables: bucket per hole, joint bucket-pair
/// probabilities, and a `(p0_win, tie, p0_lose)` matrix for showdown EV.
struct Precomputed {
    bucket_of_hole: HashMap<[Card; 2], u32>,
    bucket_pair_probs: Vec<((u32, u32), f64)>,
    winrate: Vec<Vec<(f64, f64, f64)>>, // [b0][b1] -> (p0w, tie, p0l)
}

pub struct HunlFlop {
    pub config: HunlFlopConfig,
    pub action_abs: ActionAbstraction,
    pub card_abs: CardAbstraction,
    evaluator: HandEvaluator,
    tables: OnceLock<Arc<Precomputed>>,
}

impl HunlFlop {
    pub fn new(config: HunlFlopConfig) -> Self {
        let action_abs = ActionAbstraction::new(config.bet_sizes.clone());
        let card_abs = CardAbstraction::new(
            config.card_buckets,
            config.equity_rollouts,
            config.abstraction_seed,
        );
        Self {
            config,
            action_abs,
            card_abs,
            evaluator: HandEvaluator::new(),
            tables: OnceLock::new(),
        }
    }

    fn remaining_deck(&self) -> Vec<Card> {
        let mut d = Vec::with_capacity(49);
        for r in 2..=14u8 {
            for s in [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades] {
                let c = Card::new(r, s);
                if !self.config.board.contains(&c) {
                    d.push(c);
                }
            }
        }
        d
    }

    fn all_holes(&self) -> Vec<[Card; 2]> {
        let deck = self.remaining_deck();
        let mut out = Vec::with_capacity(deck.len() * (deck.len() - 1) / 2);
        for i in 0..deck.len() {
            for j in i + 1..deck.len() {
                out.push([deck[i], deck[j]]);
            }
        }
        out
    }

    fn tables(&self) -> Arc<Precomputed> {
        self.tables
            .get_or_init(|| Arc::new(self.compute_tables()))
            .clone()
    }

    fn compute_tables(&self) -> Precomputed {
        let k = self.config.card_buckets as usize;
        let holes = self.all_holes();
        let bucket_of: Vec<u32> = holes
            .iter()
            .map(|h| self.card_abs.bucket(h, &self.config.board))
            .collect();

        // Pre-evaluate each hole's 5-card flop rank once.
        let board = self.config.board;
        let ranks: Vec<HandRank> = holes
            .iter()
            .map(|h| {
                let cards: [Card; 5] = [h[0], h[1], board[0], board[1], board[2]];
                self.evaluator.evaluate(&cards)
            })
            .collect();

        let mut bucket_of_hole = HashMap::with_capacity(holes.len());
        for (h, &b) in holes.iter().zip(bucket_of.iter()) {
            let canon = canon_hole(h);
            bucket_of_hole.insert(canon, b);
        }

        // Aggregate ordered hole-pair counts and showdown results per
        // (b0, b1). Ordered = (h0, h1) treated as distinct from (h1, h0)
        // because they correspond to different deals to (p0, p1).
        let mut counts = vec![vec![0u64; k]; k];
        let mut p0_wins = vec![vec![0u64; k]; k];
        let mut ties = vec![vec![0u64; k]; k];

        for i in 0..holes.len() {
            for j in 0..holes.len() {
                if i == j {
                    continue;
                }
                let h0 = &holes[i];
                let h1 = &holes[j];
                if h0[0] == h1[0] || h0[0] == h1[1] || h0[1] == h1[0] || h0[1] == h1[1] {
                    continue;
                }
                let b0 = bucket_of[i] as usize;
                let b1 = bucket_of[j] as usize;
                counts[b0][b1] += 1;
                let r0 = ranks[i];
                let r1 = ranks[j];
                if r0 < r1 {
                    p0_wins[b0][b1] += 1;
                } else if r0 == r1 {
                    ties[b0][b1] += 1;
                }
            }
        }

        let total: u64 = counts.iter().flatten().sum();
        debug_assert!(total > 0, "no valid bucket pairs — board exhausts deck?");
        let mut bucket_pair_probs = Vec::with_capacity(k * k);
        let mut winrate = vec![vec![(0.0f64, 0.0f64, 0.0f64); k]; k];
        for b0 in 0..k {
            for b1 in 0..k {
                let c = counts[b0][b1];
                if c == 0 {
                    continue;
                }
                let cf = c as f64;
                let p = cf / total as f64;
                bucket_pair_probs.push(((b0 as u32, b1 as u32), p));
                let p0w = p0_wins[b0][b1] as f64 / cf;
                let tie = ties[b0][b1] as f64 / cf;
                let p0l = 1.0 - p0w - tie;
                winrate[b0][b1] = (p0w, tie, p0l);
            }
        }

        Precomputed {
            bucket_of_hole,
            bucket_pair_probs,
            winrate,
        }
    }

    /// Equity-percentile bucket for a hole on this subgame's board.
    /// Triggers a one-time precomputation of bucket assignments + winrate
    /// matrix on first call.
    pub fn bucket_for(&self, hole: &[Card; 2]) -> u32 {
        let t = self.tables();
        let canon = canon_hole(hole);
        *t.bucket_of_hole
            .get(&canon)
            .expect("hole not in residual deck for this board")
    }

    /// Stable infoset key matching what `<HunlFlop as Game>::infoset_key`
    /// computes for the same `(bucket, history, player)` triple.
    pub fn infoset_key_from_components(
        bucket: u32,
        history: &[AbstractAction],
        player: usize,
    ) -> u64 {
        let mut h = DefaultHasher::new();
        bucket.hash(&mut h);
        for a in history {
            a.hash(&mut h);
        }
        (player as u8).hash(&mut h);
        h.finish()
    }

    fn pot_now(&self, s: &HunlFlopState) -> f64 {
        self.config.starting_pot + s.committed[0] + s.committed[1]
    }

    fn legal_action_set(&self, s: &HunlFlopState) -> Vec<ActionChoice> {
        let stack = s.stacks[s.to_act];
        let mut out = Vec::new();
        if s.facing > 0.0 {
            out.push(ActionChoice::Fold);
            out.push(ActionChoice::Call);
            if s.raises_used < self.config.max_raises_per_street && stack > s.facing {
                let pot_after_call = self.pot_now(s) + s.facing;
                for (i, frac) in self.config.bet_sizes.iter().enumerate() {
                    let added = s.facing + frac * pot_after_call;
                    if added > s.facing && added < stack {
                        out.push(ActionChoice::Raise {
                            size_idx: i as u8,
                            added,
                        });
                    }
                }
                out.push(ActionChoice::AllIn { added: stack });
            }
        } else {
            out.push(ActionChoice::Check);
            if s.raises_used < self.config.max_raises_per_street && stack > 0.0 {
                let pot = self.pot_now(s);
                for (i, frac) in self.config.bet_sizes.iter().enumerate() {
                    let amt = frac * pot;
                    if amt > 0.0 && amt < stack {
                        out.push(ActionChoice::Bet {
                            size_idx: i as u8,
                            amount: amt,
                        });
                    }
                }
                out.push(ActionChoice::AllIn { added: stack });
            }
        }
        out
    }

    fn apply(&self, s: &HunlFlopState, choice: &ActionChoice) -> HunlFlopState {
        let mut ns = s.clone();
        let actor = ns.to_act;
        let opp = 1 - actor;
        match *choice {
            ActionChoice::Fold => {
                ns.terminal = Some(Terminal::Fold { folder: actor });
                ns.history.push(AbstractAction::Fold);
            }
            ActionChoice::Check => {
                ns.history.push(AbstractAction::Check);
                ns.last = LastAct::Check;
                if matches!(s.last, LastAct::Check) {
                    ns.terminal = Some(Terminal::Showdown);
                } else {
                    ns.to_act = opp;
                }
            }
            ActionChoice::Call => {
                let pay = ns.facing.min(ns.stacks[actor]);
                ns.stacks[actor] -= pay;
                ns.committed[actor] += pay;
                ns.history.push(AbstractAction::Call);
                ns.last = LastAct::Call;
                ns.facing = 0.0;
                ns.terminal = Some(Terminal::Showdown);
            }
            ActionChoice::Bet { size_idx, amount } => {
                let amt = amount.min(ns.stacks[actor]);
                ns.stacks[actor] -= amt;
                ns.committed[actor] += amt;
                ns.facing = (ns.committed[actor] - ns.committed[opp]).max(0.0);
                ns.history.push(AbstractAction::Bet(size_idx));
                ns.last = LastAct::Bet(amt);
                ns.raises_used += 1;
                ns.to_act = opp;
            }
            ActionChoice::Raise { size_idx, added } => {
                let amt = added.min(ns.stacks[actor]);
                ns.stacks[actor] -= amt;
                ns.committed[actor] += amt;
                ns.facing = (ns.committed[actor] - ns.committed[opp]).max(0.0);
                ns.history.push(AbstractAction::Raise(size_idx));
                ns.last = LastAct::Raise(amt);
                ns.raises_used += 1;
                ns.to_act = opp;
            }
            ActionChoice::AllIn { added } => {
                let amt = added.min(ns.stacks[actor]);
                ns.stacks[actor] -= amt;
                ns.committed[actor] += amt;
                ns.facing = (ns.committed[actor] - ns.committed[opp]).max(0.0);
                let was_facing = matches!(s.last, LastAct::Bet(_) | LastAct::Raise(_));
                ns.history.push(if was_facing {
                    AbstractAction::Raise(u8::MAX)
                } else {
                    AbstractAction::Bet(u8::MAX)
                });
                ns.last = LastAct::Raise(amt);
                ns.raises_used += 1;
                ns.to_act = opp;
                if ns.stacks[opp] == 0.0 || ns.facing == 0.0 {
                    ns.terminal = Some(Terminal::Showdown);
                }
            }
        }
        ns
    }
}

fn canon_hole(hole: &[Card; 2]) -> [Card; 2] {
    if hole[0].index() < hole[1].index() {
        *hole
    } else {
        [hole[1], hole[0]]
    }
}

#[derive(Clone, Debug)]
enum ActionChoice {
    Fold,
    Check,
    Call,
    Bet { size_idx: u8, amount: f64 },
    Raise { size_idx: u8, added: f64 },
    AllIn { added: f64 },
}

impl Game for HunlFlop {
    type State = HunlFlopState;

    fn initial_state(&self) -> Self::State {
        HunlFlopState {
            buckets: None,
            stacks: [self.config.starting_stack, self.config.starting_stack],
            committed: [0.0, 0.0],
            facing: 0.0,
            to_act: 1, // OOP acts first on flop
            history: Vec::new(),
            raises_used: 0,
            last: LastAct::None,
            terminal: None,
        }
    }

    fn is_terminal(&self, s: &Self::State) -> bool {
        s.terminal.is_some()
    }

    fn is_chance(&self, s: &Self::State) -> bool {
        s.buckets.is_none()
    }

    fn chance_outcomes(&self, s: &Self::State) -> Vec<(Self::State, f64)> {
        debug_assert!(s.buckets.is_none());
        let t = self.tables();
        t.bucket_pair_probs
            .iter()
            .map(|&(buckets, prob)| {
                let mut ns = s.clone();
                ns.buckets = Some(buckets);
                (ns, prob)
            })
            .collect()
    }

    fn current_player(&self, s: &Self::State) -> usize {
        s.to_act
    }

    fn num_actions(&self, s: &Self::State) -> usize {
        self.legal_action_set(s).len()
    }

    fn next_state(&self, s: &Self::State, action_idx: usize) -> Self::State {
        let acts = self.legal_action_set(s);
        self.apply(s, &acts[action_idx])
    }

    fn utility(&self, s: &Self::State, player: usize) -> f64 {
        let (b0, b1) = s.buckets.expect("utility called pre-chance");
        let pot = self.pot_now(s);
        let raw = match s.terminal.as_ref().expect("terminal") {
            Terminal::Fold { folder } => {
                if *folder == player {
                    -s.committed[player]
                } else {
                    pot - s.committed[player]
                }
            }
            Terminal::Showdown => {
                let t = self.tables();
                let (p0w, tie, p0l) = t.winrate[b0 as usize][b1 as usize];
                let p_win = if player == 0 { p0w } else { p0l };
                let share = p_win * pot + tie * (pot / 2.0);
                share - s.committed[player]
            }
        };
        // Centre so utilities sum to zero in this constant-sum subgame.
        raw - self.config.starting_pot / 2.0
    }

    fn infoset_key(&self, s: &Self::State, player: usize) -> u64 {
        let (b0, b1) = s.buckets.expect("infoset key pre-chance");
        let bucket = if player == 0 { b0 } else { b1 };
        HunlFlop::infoset_key_from_components(bucket, &s.history, player)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfr::CFRSolver;

    fn tiny_config() -> HunlFlopConfig {
        HunlFlopConfig {
            board: [
                Card::parse("Ah").unwrap(),
                Card::parse("Ks").unwrap(),
                Card::parse("7d").unwrap(),
            ],
            starting_pot: 6.0,
            starting_stack: 10.0,
            bet_sizes: vec![1.0],
            card_buckets: 2,
            equity_rollouts: 20,
            max_raises_per_street: 1,
            abstraction_seed: 1,
        }
    }

    #[test]
    fn hunl_state_terminates() {
        let game = HunlFlop::new(tiny_config());
        let s0 = game.initial_state();
        assert!(game.is_chance(&s0));
        let outcomes = game.chance_outcomes(&s0);
        assert!(!outcomes.is_empty());
        let (mut s, _) = outcomes[0].clone();
        assert!(!game.is_terminal(&s));
        s = game.next_state(&s, 0); // OOP check
        s = game.next_state(&s, 0); // IP check
        assert!(game.is_terminal(&s));
        assert_eq!(s.terminal, Some(Terminal::Showdown));
    }

    #[test]
    fn hunl_runs_a_few_cfr_iterations() {
        // Now fast enough to run in the default test suite: the chance
        // fan-out is K² = 4 instead of 1.27M.
        let game = HunlFlop::new(tiny_config());
        let mut solver = CFRSolver::new(game);
        solver.run(20);
        assert!(!solver.history.points.is_empty());
        let expl = solver.exploitability();
        assert!(expl.is_finite());
    }

    #[test]
    fn bucket_pair_probs_sum_to_one() {
        let game = HunlFlop::new(tiny_config());
        let t = game.tables();
        let total: f64 = t.bucket_pair_probs.iter().map(|(_, p)| *p).sum();
        assert!(
            (total - 1.0).abs() < 1e-9,
            "bucket pair probs sum to {total}"
        );
    }
}
