//! Card and action abstraction layers.
//!
//! `CardAbstraction` buckets hole-card combinations by equity percentile via
//! Monte Carlo rollouts. `ActionAbstraction` discretizes the continuous bet
//! space into a fixed set of pot-fraction sizes plus all-in.

use crate::eval::HandEvaluator;
use crate::game::{Card, Suit};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AbstractionConfig {
    pub card_buckets: u32,
    pub bet_sizes: Vec<f64>,
    /// Monte Carlo rollouts per (hole, board) equity estimation.
    pub equity_rollouts: u32,
    /// RNG seed for deterministic bucketing.
    pub seed: u64,
}

impl Default for AbstractionConfig {
    fn default() -> Self {
        Self {
            card_buckets: 50,
            bet_sizes: vec![0.25, 0.50, 0.75],
            equity_rollouts: 1000,
            seed: 0xC0DE_FACE_C0FF_EE42,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CardAbstraction {
    pub num_buckets: u32,
    pub rollouts: u32,
    pub seed: u64,
    evaluator: HandEvaluator,
}

impl CardAbstraction {
    pub fn new(num_buckets: u32, rollouts: u32, seed: u64) -> Self {
        Self {
            num_buckets,
            rollouts,
            seed,
            evaluator: HandEvaluator::new(),
        }
    }

    /// Equity (win + 0.5 * tie) of `hole` versus a uniform random villain
    /// holding two cards drawn from the remaining deck, after a uniform
    /// run-out of any missing community cards.
    pub fn monte_carlo_equity(&self, hole: &[Card; 2], board: &[Card]) -> f64 {
        let known: Vec<Card> = hole.iter().chain(board.iter()).copied().collect();
        let deck: Vec<Card> = full_deck()
            .into_iter()
            .filter(|c| !known.contains(c))
            .collect();
        let needed = 5 - board.len();
        let mut rng = ChaCha8Rng::seed_from_u64(self.seed ^ stable_hash(hole, board));
        let mut wins = 0.0f64;
        let mut total = 0u32;
        let mut buf = deck.clone();
        for _ in 0..self.rollouts {
            buf.shuffle(&mut rng);
            // villain takes two cards, run-out takes `needed` more
            let villain = [buf[0], buf[1]];
            let runout = &buf[2..2 + needed];
            let mut my7 = Vec::with_capacity(7);
            my7.extend_from_slice(hole);
            my7.extend_from_slice(board);
            my7.extend_from_slice(runout);
            let mut their7 = Vec::with_capacity(7);
            their7.extend_from_slice(&villain);
            their7.extend_from_slice(board);
            their7.extend_from_slice(runout);
            let me = self.evaluator.evaluate(&my7);
            let opp = self.evaluator.evaluate(&their7);
            if me < opp {
                wins += 1.0;
            } else if me == opp {
                wins += 0.5;
            }
            total += 1;
        }
        wins / total as f64
    }

    /// Assigns this `(hole, board)` to an equity-percentile bucket in
    /// `0..num_buckets`. Bucket `0` is the weakest, `num_buckets - 1` the
    /// strongest.
    pub fn bucket(&self, hole: &[Card; 2], board: &[Card]) -> u32 {
        let eq = self.monte_carlo_equity(hole, board);
        let b = (eq * self.num_buckets as f64).floor() as i64;
        b.clamp(0, self.num_buckets as i64 - 1) as u32
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionAbstraction {
    /// Discrete bet sizes as fractions of the current pot. All-in is appended
    /// implicitly by the game tree when stacks are insufficient for the
    /// requested fraction.
    pub bet_sizes: Vec<f64>,
}

impl ActionAbstraction {
    pub fn new(bet_sizes: Vec<f64>) -> Self {
        Self { bet_sizes }
    }

    pub fn default_v1() -> Self {
        Self::new(vec![0.25, 0.50, 0.75])
    }

    pub fn num_sizes(&self) -> usize {
        self.bet_sizes.len()
    }

    /// Returns the absolute bet amount in chips for size index `i` against
    /// the given pot. The caller is responsible for capping by stack.
    pub fn size_for(&self, pot: f64, i: usize) -> f64 {
        pot * self.bet_sizes[i]
    }
}

fn full_deck() -> Vec<Card> {
    let mut d = Vec::with_capacity(52);
    for r in 2..=14u8 {
        for s in [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades] {
            d.push(Card::new(r, s));
        }
    }
    d
}

fn stable_hash(hole: &[Card; 2], board: &[Card]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    hole.hash(&mut h);
    board.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Card;

    #[test]
    fn pocket_aces_beats_seven_two() {
        // Use a small rollout count for test speed; the equity gap is large.
        let abs = CardAbstraction::new(10, 200, 7);
        let board = vec![];
        let aa = [Card::parse("As").unwrap(), Card::parse("Ah").unwrap()];
        let seven_two = [Card::parse("7c").unwrap(), Card::parse("2d").unwrap()];
        let eq_aa = abs.monte_carlo_equity(&aa, &board);
        let eq_72 = abs.monte_carlo_equity(&seven_two, &board);
        assert!(eq_aa > 0.75, "AA equity should be > 0.75, got {}", eq_aa);
        assert!(eq_72 < 0.5, "72o equity should be < 0.5, got {}", eq_72);
        assert!(abs.bucket(&aa, &board) >= abs.bucket(&seven_two, &board));
    }

    #[test]
    fn action_abstraction_default_has_three_sizes() {
        let a = ActionAbstraction::default_v1();
        assert_eq!(a.num_sizes(), 3);
        assert_eq!(a.size_for(100.0, 0), 25.0);
        assert_eq!(a.size_for(100.0, 1), 50.0);
        assert_eq!(a.size_for(100.0, 2), 75.0);
    }
}
