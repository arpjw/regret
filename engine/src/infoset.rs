//! Information sets — the indistinguishability classes for the acting player.
//!
//! An `InfoSet` is a (card-bucket, abstract action history) pair. Its `key()`
//! method returns a 64-bit hash used as the index into the regret /
//! strategy-sum tables. `InfoSetData` is the per-infoset state that CFR+
//! accumulates across iterations.

use crate::game::AbstractAction;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InfoSet {
    pub card_bucket: u32,
    pub action_history: Vec<AbstractAction>,
}

impl InfoSet {
    pub fn new(card_bucket: u32, action_history: Vec<AbstractAction>) -> Self {
        Self {
            card_bucket,
            action_history,
        }
    }

    /// Stable 64-bit key combining the bucket and action history.
    pub fn key(&self) -> u64 {
        let mut h = DefaultHasher::new();
        self.card_bucket.hash(&mut h);
        for a in &self.action_history {
            a.hash(&mut h);
        }
        h.finish()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InfoSetData {
    pub regret_sum: Vec<f64>,
    pub strategy_sum: Vec<f64>,
}

impl InfoSetData {
    pub fn new(num_actions: usize) -> Self {
        Self {
            regret_sum: vec![0.0; num_actions],
            strategy_sum: vec![0.0; num_actions],
        }
    }

    /// Current strategy via regret matching with CFR+ flooring.
    pub fn current_strategy(&self) -> Vec<f64> {
        let n = self.regret_sum.len();
        let mut out = vec![0.0; n];
        let mut total = 0.0;
        for (i, r) in self.regret_sum.iter().enumerate() {
            let pos = r.max(0.0);
            out[i] = pos;
            total += pos;
        }
        if total > 0.0 {
            for v in out.iter_mut() {
                *v /= total;
            }
        } else {
            let u = 1.0 / n as f64;
            for v in out.iter_mut() {
                *v = u;
            }
        }
        out
    }

    /// Time-averaged strategy — the Nash approximant.
    pub fn average_strategy(&self) -> Vec<f64> {
        let n = self.strategy_sum.len();
        let total: f64 = self.strategy_sum.iter().sum();
        if total > 0.0 {
            self.strategy_sum.iter().map(|v| v / total).collect()
        } else {
            vec![1.0 / n as f64; n]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_when_no_regret() {
        let d = InfoSetData::new(4);
        let s = d.current_strategy();
        for v in s {
            assert!((v - 0.25).abs() < 1e-9);
        }
    }

    #[test]
    fn negative_regret_floors_to_zero() {
        let mut d = InfoSetData::new(3);
        d.regret_sum = vec![-1.0, 2.0, 1.0];
        let s = d.current_strategy();
        assert!((s[0] - 0.0).abs() < 1e-9);
        assert!((s[1] - 2.0 / 3.0).abs() < 1e-9);
        assert!((s[2] - 1.0 / 3.0).abs() < 1e-9);
    }
}
