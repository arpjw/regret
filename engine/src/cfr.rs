//! CFR+ — Nash equilibrium solver for two-player zero-sum extensive-form games.
//!
//! The solver is generic over a `Game` trait so the same engine drives the
//! production HUNL flop subgame and the Kuhn-poker correctness test. CFR+
//! differs from vanilla CFR in two ways:
//!
//!   1. Negative cumulative regrets are floored to zero at each iteration.
//!   2. Updates are *alternating* — on iteration `t` we update player 0's
//!      regrets, on iteration `t+1` we update player 1's.
//!
//! Exploitability is computed every 50 iterations via a proper
//! imperfect-information best response (Lanctot-style two-pass algorithm).

use crate::infoset::InfoSetData;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Two-player zero-sum extensive-form game abstraction.
///
/// Players are `0` and `1`. Utility is returned from the **perspective of the
/// requested player** so the solver is sign-agnostic.
pub trait Game {
    type State: Clone;

    fn initial_state(&self) -> Self::State;
    fn is_terminal(&self, s: &Self::State) -> bool;
    fn is_chance(&self, s: &Self::State) -> bool;
    /// `(next_state, probability)` pairs that sum to 1 at chance nodes.
    fn chance_outcomes(&self, s: &Self::State) -> Vec<(Self::State, f64)>;
    fn current_player(&self, s: &Self::State) -> usize;
    fn num_actions(&self, s: &Self::State) -> usize;
    fn next_state(&self, s: &Self::State, action_idx: usize) -> Self::State;
    /// Utility from `player`'s perspective at a terminal state.
    fn utility(&self, s: &Self::State, player: usize) -> f64;
    /// Stable 64-bit key for the infoset visible to `player` at this state.
    fn infoset_key(&self, s: &Self::State, player: usize) -> u64;
}

/// Allow callers to share a single game instance (and its caches) between
/// the solver and downstream consumers by wrapping it in `Arc`.
impl<G: Game + ?Sized> Game for Arc<G> {
    type State = G::State;

    fn initial_state(&self) -> Self::State {
        (**self).initial_state()
    }
    fn is_terminal(&self, s: &Self::State) -> bool {
        (**self).is_terminal(s)
    }
    fn is_chance(&self, s: &Self::State) -> bool {
        (**self).is_chance(s)
    }
    fn chance_outcomes(&self, s: &Self::State) -> Vec<(Self::State, f64)> {
        (**self).chance_outcomes(s)
    }
    fn current_player(&self, s: &Self::State) -> usize {
        (**self).current_player(s)
    }
    fn num_actions(&self, s: &Self::State) -> usize {
        (**self).num_actions(s)
    }
    fn next_state(&self, s: &Self::State, action_idx: usize) -> Self::State {
        (**self).next_state(s, action_idx)
    }
    fn utility(&self, s: &Self::State, player: usize) -> f64 {
        (**self).utility(s, player)
    }
    fn infoset_key(&self, s: &Self::State, player: usize) -> u64 {
        (**self).infoset_key(s, player)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConvergencePoint {
    pub iteration: u64,
    pub exploitability: f64,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ConvergenceHistory {
    pub points: Vec<ConvergencePoint>,
}

impl ConvergenceHistory {
    pub fn record(&mut self, iteration: u64, exploitability: f64, elapsed_ms: u64) {
        self.points.push(ConvergencePoint {
            iteration,
            exploitability,
            elapsed_ms,
        });
    }

    pub fn last(&self) -> Option<&ConvergencePoint> {
        self.points.last()
    }
}

/// Which regret-update rule the solver runs.
///
/// * [`CfrVariant::Plus`] — CFR$^+$: floor negative cumulative regret to zero
///   and weight iteration-$t$ averaging contributions by $t$ (linear CFR$^+$
///   averaging, Tammelin 2014).
/// * [`CfrVariant::Vanilla`] — classical CFR: signed regret, uniform
///   averaging (each iteration's strategy contributes with weight 1).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CfrVariant {
    Plus,
    Vanilla,
}

pub struct CFRSolver<G: Game> {
    pub game: G,
    pub data: Arc<DashMap<u64, InfoSetData>>,
    pub iterations: u64,
    pub history: ConvergenceHistory,
    pub variant: CfrVariant,
}

impl<G: Game> CFRSolver<G> {
    pub fn new(game: G) -> Self {
        Self::with_variant(game, CfrVariant::Plus)
    }

    pub fn with_variant(game: G, variant: CfrVariant) -> Self {
        Self {
            game,
            data: Arc::new(DashMap::new()),
            iterations: 0,
            history: ConvergenceHistory::default(),
            variant,
        }
    }

    /// Strategy by regret matching at infoset `key` with `n` actions.
    fn current_strategy(&self, key: u64, n: usize) -> Vec<f64> {
        let mut entry = self.data.entry(key).or_insert_with(|| InfoSetData::new(n));
        // Defensive: action counts must be consistent across visits.
        if entry.regret_sum.len() != n {
            entry.regret_sum = vec![0.0; n];
            entry.strategy_sum = vec![0.0; n];
        }
        entry.current_strategy()
    }

    /// Time-averaged strategy at infoset `key` with `n` actions.
    pub fn average_strategy(&self, key: u64, n: usize) -> Vec<f64> {
        match self.data.get(&key) {
            Some(d) => d.average_strategy(),
            None => vec![1.0 / n as f64; n],
        }
    }

    /// Single recursive CFR+ traversal updating regrets for `updating_player`.
    fn cfr(&self, state: &G::State, updating_player: usize, p0: f64, p1: f64) -> f64 {
        if self.game.is_terminal(state) {
            return self.game.utility(state, updating_player);
        }
        if self.game.is_chance(state) {
            let mut v = 0.0;
            for (next, prob) in self.game.chance_outcomes(state) {
                v += prob * self.cfr(&next, updating_player, p0, p1);
            }
            return v;
        }

        let actor = self.game.current_player(state);
        let n = self.game.num_actions(state);
        let key = self.game.infoset_key(state, actor);
        let strategy = self.current_strategy(key, n);

        let mut action_values = vec![0.0; n];
        let mut node_value = 0.0;
        for a in 0..n {
            let next = self.game.next_state(state, a);
            let (np0, np1) = if actor == 0 {
                (p0 * strategy[a], p1)
            } else {
                (p0, p1 * strategy[a])
            };
            action_values[a] = self.cfr(&next, updating_player, np0, np1);
            node_value += strategy[a] * action_values[a];
        }

        if actor == updating_player {
            let counterfactual = if actor == 0 { p1 } else { p0 };
            let reach = if actor == 0 { p0 } else { p1 };
            // Linear CFR+ averaging weight: contributions from iteration t
            // count as t (1-indexed). This is the standard CFR+ averaging
            // scheme from Tammelin (2014); without it, exploitability
            // decays an order of magnitude more slowly on small games like
            // Kuhn. Vanilla CFR uses uniform averaging.
            let weight = match self.variant {
                CfrVariant::Plus => (self.iterations + 1) as f64,
                CfrVariant::Vanilla => 1.0,
            };
            let mut entry = self.data.get_mut(&key).expect("infoset present");
            for a in 0..n {
                let regret = counterfactual * (action_values[a] - node_value);
                let new_regret = entry.regret_sum[a] + regret;
                entry.regret_sum[a] = match self.variant {
                    CfrVariant::Plus => new_regret.max(0.0),
                    CfrVariant::Vanilla => new_regret,
                };
                entry.strategy_sum[a] += weight * reach * strategy[a];
            }
        }

        node_value
    }

    /// Run CFR+ for `iterations` iterations. Returns the accumulated
    /// convergence history (also stored on `self.history`).
    pub fn run(&mut self, iterations: u64) -> &ConvergenceHistory {
        let start = std::time::Instant::now();
        for t in 0..iterations {
            let init = self.game.initial_state();
            self.cfr(&init, 0, 1.0, 1.0);
            self.cfr(&init, 1, 1.0, 1.0);
            self.iterations += 1;

            if self.iterations % 50 == 0 || self.iterations == iterations {
                let expl = self.exploitability();
                let elapsed = start.elapsed().as_millis() as u64;
                self.history.record(self.iterations, expl, elapsed);
            }
            let _ = t;
        }
        &self.history
    }

    /// Exploitability of the current average strategy profile, in the same
    /// units as `utility` (typically big blinds). At Nash this is 0.
    pub fn exploitability(&self) -> f64 {
        let v0 = self.best_response_value(0);
        let v1 = self.best_response_value(1);
        (v0 + v1) / 2.0
    }

    /// Best-response value for `br_player` against the opponent's current
    /// average strategy. Implements the standard imperfect-information
    /// best-response algorithm (Heinrich & Silver 2015, §4.1):
    ///
    ///   1. Forward-walk: collect, for each `br_player` infoset, the set of
    ///      reachable decision histories and their opponent reach
    ///      probabilities.
    ///   2. Backward DP: process infosets in order of decreasing depth;
    ///      assign the BR action at each infoset as the argmax of action
    ///      values aggregated over its histories.
    ///   3. Replay the tree under the resulting BR strategy to obtain the
    ///      total expected value.
    pub fn best_response_value(&self, br_player: usize) -> f64 {
        let mut ws: BrWorkspace<G> = BrWorkspace {
            histories: HashMap::new(),
            br_action: HashMap::new(),
        };
        let init = self.game.initial_state();
        self.br_collect(&init, br_player, 1.0, 0, &mut ws);

        // Process infosets deepest-first so deeper BR decisions are committed
        // before the upper ones consult them.
        let mut keys: Vec<u64> = ws.histories.keys().copied().collect();
        keys.sort_by_key(|k| std::cmp::Reverse(ws.histories[k].max_depth));
        for k in keys {
            let entry = ws.histories.get(&k).expect("present");
            let n = entry.num_actions;
            let mut action_values = vec![0.0; n];
            // Snapshot to avoid borrow issues inside the recursion below.
            let histories = entry.items.clone();
            for (state, opp_reach) in histories {
                for a in 0..n {
                    let child = self.game.next_state(&state, a);
                    action_values[a] += opp_reach * self.br_value_below(&child, br_player, &ws);
                }
            }
            let best = (0..n)
                .max_by(|&a, &b| action_values[a].partial_cmp(&action_values[b]).unwrap())
                .unwrap_or(0);
            ws.br_action.insert(k, best);
        }

        self.br_value_below(&self.game.initial_state(), br_player, &ws)
    }

    fn br_collect(
        &self,
        state: &G::State,
        br_player: usize,
        opp_reach: f64,
        depth: usize,
        ws: &mut BrWorkspace<G>,
    ) {
        if self.game.is_terminal(state) {
            return;
        }
        if self.game.is_chance(state) {
            for (next, _) in self.game.chance_outcomes(state) {
                self.br_collect(&next, br_player, opp_reach, depth + 1, ws);
            }
            return;
        }
        let actor = self.game.current_player(state);
        let n = self.game.num_actions(state);
        if actor == br_player {
            let key = self.game.infoset_key(state, actor);
            let entry = ws.histories.entry(key).or_insert(BrInfoset {
                items: Vec::new(),
                num_actions: n,
                max_depth: depth,
            });
            entry.items.push((state.clone(), opp_reach));
            if depth > entry.max_depth {
                entry.max_depth = depth;
            }
            for a in 0..n {
                let next = self.game.next_state(state, a);
                self.br_collect(&next, br_player, opp_reach, depth + 1, ws);
            }
        } else {
            let key = self.game.infoset_key(state, actor);
            let strat = self.average_strategy(key, n);
            for a in 0..n {
                let next = self.game.next_state(state, a);
                self.br_collect(&next, br_player, opp_reach * strat[a], depth + 1, ws);
            }
        }
    }

    fn br_value_below(&self, state: &G::State, br_player: usize, ws: &BrWorkspace<G>) -> f64 {
        if self.game.is_terminal(state) {
            return self.game.utility(state, br_player);
        }
        if self.game.is_chance(state) {
            let mut v = 0.0;
            for (next, p) in self.game.chance_outcomes(state) {
                v += p * self.br_value_below(&next, br_player, ws);
            }
            return v;
        }
        let actor = self.game.current_player(state);
        let n = self.game.num_actions(state);
        if actor == br_player {
            let key = self.game.infoset_key(state, actor);
            // If we haven't committed a BR action for this infoset yet
            // (during the DP pass), fall back to a single-best-action max —
            // this is only hit while computing values for deeper infosets,
            // which have already been committed.
            let a = ws.br_action.get(&key).copied().unwrap_or_else(|| {
                (0..n)
                    .max_by(|&a, &b| {
                        let va = self.br_value_below(&self.game.next_state(state, a), br_player, ws);
                        let vb = self.br_value_below(&self.game.next_state(state, b), br_player, ws);
                        va.partial_cmp(&vb).unwrap()
                    })
                    .unwrap_or(0)
            });
            self.br_value_below(&self.game.next_state(state, a), br_player, ws)
        } else {
            let key = self.game.infoset_key(state, actor);
            let strat = self.average_strategy(key, n);
            let mut v = 0.0;
            for a in 0..n {
                v += strat[a] * self.br_value_below(&self.game.next_state(state, a), br_player, ws);
            }
            v
        }
    }

    /// Expected value of self-play under the current average strategy, from
    /// player 0's perspective. At Nash this equals the game value.
    pub fn self_play_value(&self) -> f64 {
        self.replay(&self.game.initial_state(), 0)
    }

    fn replay(&self, state: &G::State, eval_player: usize) -> f64 {
        if self.game.is_terminal(state) {
            return self.game.utility(state, eval_player);
        }
        if self.game.is_chance(state) {
            let mut v = 0.0;
            for (next, p) in self.game.chance_outcomes(state) {
                v += p * self.replay(&next, eval_player);
            }
            return v;
        }
        let actor = self.game.current_player(state);
        let n = self.game.num_actions(state);
        let key = self.game.infoset_key(state, actor);
        let strat = self.average_strategy(key, n);
        let mut v = 0.0;
        for a in 0..n {
            v += strat[a] * self.replay(&self.game.next_state(state, a), eval_player);
        }
        v
    }
}

struct BrInfoset<G: Game> {
    items: Vec<(G::State, f64)>,
    num_actions: usize,
    max_depth: usize,
}

struct BrWorkspace<G: Game> {
    histories: HashMap<u64, BrInfoset<G>>,
    br_action: HashMap<u64, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Kuhn poker game ----------------------------------------------------
    //
    // 3-card deck (J=11, Q=12, K=13). Ante 1. Player 0 acts first.
    //   - check / bet on first decision
    //   - after check: opponent check (showdown) or bet
    //   - after bet: opponent fold or call (showdown)
    //   - after check-bet: original actor fold or call (showdown)
    //
    // Bets are 1 chip on top of the 1-chip ante.

    #[derive(Clone)]
    struct KuhnState {
        cards: Option<(u8, u8)>, // (p0, p1) after chance is resolved
        history: Vec<KuhnAct>,
        next_player: usize,
    }

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    enum KuhnAct {
        Check, // also "pass" / "fold" depending on history position
        Bet,
    }

    struct Kuhn;

    impl Kuhn {
        fn terminal(history: &[KuhnAct]) -> bool {
            use KuhnAct::*;
            match history {
                [Check, Check] => true,
                [Check, Bet, Check] => true,         // fold (p0 folds after p1 bet)
                [Check, Bet, Bet] => true,           // call → showdown
                [Bet, Check] => true,                // fold (p1 folds)
                [Bet, Bet] => true,                  // call → showdown
                _ => false,
            }
        }

        fn winner(p0: u8, p1: u8) -> usize {
            if p0 > p1 {
                0
            } else {
                1
            }
        }
    }

    impl Game for Kuhn {
        type State = KuhnState;

        fn initial_state(&self) -> Self::State {
            KuhnState {
                cards: None,
                history: Vec::new(),
                next_player: usize::MAX,
            }
        }

        fn is_terminal(&self, s: &Self::State) -> bool {
            s.cards.is_some() && Kuhn::terminal(&s.history)
        }

        fn is_chance(&self, s: &Self::State) -> bool {
            s.cards.is_none()
        }

        fn chance_outcomes(&self, s: &Self::State) -> Vec<(Self::State, f64)> {
            assert!(s.cards.is_none());
            let mut out = Vec::with_capacity(6);
            for p0 in [11u8, 12, 13] {
                for p1 in [11u8, 12, 13] {
                    if p0 == p1 {
                        continue;
                    }
                    out.push((
                        KuhnState {
                            cards: Some((p0, p1)),
                            history: Vec::new(),
                            next_player: 0,
                        },
                        1.0 / 6.0,
                    ));
                }
            }
            out
        }

        fn current_player(&self, s: &Self::State) -> usize {
            // Player to act is determined by history length parity, with the
            // first decision belonging to player 0.
            s.history.len() % 2
        }

        fn num_actions(&self, _s: &Self::State) -> usize {
            2
        }

        fn next_state(&self, s: &Self::State, action_idx: usize) -> Self::State {
            let mut ns = s.clone();
            ns.history.push(if action_idx == 0 {
                KuhnAct::Check
            } else {
                KuhnAct::Bet
            });
            ns.next_player = ns.history.len() % 2;
            ns
        }

        fn utility(&self, s: &Self::State, player: usize) -> f64 {
            let (p0, p1) = s.cards.expect("chance resolved");
            use KuhnAct::*;
            let (p0_util, _p1_util) = match s.history.as_slice() {
                [Check, Check] => {
                    let w = Kuhn::winner(p0, p1);
                    let amt = 1.0; // each anted 1; winner gains 1
                    if w == 0 {
                        (amt, -amt)
                    } else {
                        (-amt, amt)
                    }
                }
                [Check, Bet, Check] => (-1.0, 1.0), // p0 folds
                [Check, Bet, Bet] => {
                    let w = Kuhn::winner(p0, p1);
                    let amt = 2.0; // ante + bet
                    if w == 0 {
                        (amt, -amt)
                    } else {
                        (-amt, amt)
                    }
                }
                [Bet, Check] => (1.0, -1.0), // p1 folds
                [Bet, Bet] => {
                    let w = Kuhn::winner(p0, p1);
                    let amt = 2.0;
                    if w == 0 {
                        (amt, -amt)
                    } else {
                        (-amt, amt)
                    }
                }
                _ => unreachable!("utility called on non-terminal"),
            };
            if player == 0 {
                p0_util
            } else {
                -p0_util
            }
        }

        fn infoset_key(&self, s: &Self::State, player: usize) -> u64 {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let (p0, p1) = s.cards.expect("chance resolved before infoset query");
            let mut h = DefaultHasher::new();
            (if player == 0 { p0 } else { p1 }).hash(&mut h);
            for a in &s.history {
                a.hash(&mut h);
            }
            h.finish()
        }
    }

    fn kuhn_infoset_key(card: u8, history: &[KuhnAct]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        card.hash(&mut h);
        for a in history {
            a.hash(&mut h);
        }
        h.finish()
    }

    fn prob_of(strategy: &[f64], action: KuhnAct) -> f64 {
        match action {
            KuhnAct::Check => strategy[0],
            KuhnAct::Bet => strategy[1],
        }
    }

    #[test]
    fn cfr_plus_solves_kuhn_to_nash() {
        let mut solver = CFRSolver::new(Kuhn);
        solver.run(2500);

        // Exploitability should be tiny.
        let expl = solver.exploitability();
        assert!(
            expl < 0.01,
            "exploitability did not converge near 0: {}",
            expl
        );

        // Game value from P0's perspective is -1/18 ≈ -0.0556.
        let v = solver.self_play_value();
        assert!(
            (v - (-1.0 / 18.0)).abs() < 0.01,
            "game value {} != expected -1/18",
            v
        );

        // Structural Kuhn-NE properties (invariant across the equilibrium
        // family — all valid Nash profiles satisfy these):
        //
        //   Q never opens a bet
        //   J never calls a bet
        //   K always bets after check
        //   K always calls after a bet
        let p0_q = solver.average_strategy(kuhn_infoset_key(12, &[]), 2);
        let p1_j_after_check = solver.average_strategy(kuhn_infoset_key(11, &[KuhnAct::Check]), 2);
        let p1_k_after_check = solver.average_strategy(kuhn_infoset_key(13, &[KuhnAct::Check]), 2);
        let p1_k_after_bet = solver.average_strategy(kuhn_infoset_key(13, &[KuhnAct::Bet]), 2);
        let p0_j_after_cb = solver.average_strategy(
            kuhn_infoset_key(11, &[KuhnAct::Check, KuhnAct::Bet]),
            2,
        );

        // Across all Kuhn NE, P0's Q-open-bet probability is 0 and the
        // weaker hands fold to a check-bet. P1's strategy is rigid (not
        // parameterized), so we can pin its frequencies precisely:
        //   - bluff J (after P0 check) with probability 1/3
        //   - always bet K (after P0 check) for value
        //   - always call K (after P0 bet)
        assert!(
            prob_of(&p0_q, KuhnAct::Bet) < 0.02,
            "P0 with Q must not open-bet: {:?}",
            p0_q
        );
        let p1_j_bluff = prob_of(&p1_j_after_check, KuhnAct::Bet);
        assert!(
            (p1_j_bluff - 1.0 / 3.0).abs() < 0.05,
            "P1 with J after check should bluff at ~1/3: {:?}",
            p1_j_after_check
        );
        assert!(
            prob_of(&p1_k_after_check, KuhnAct::Bet) > 0.96,
            "P1 with K after check must bet for value: {:?}",
            p1_k_after_check
        );
        assert!(
            prob_of(&p1_k_after_bet, KuhnAct::Bet) > 0.96,
            "P1 with K facing a bet must call: {:?}",
            p1_k_after_bet
        );
        assert!(
            prob_of(&p0_j_after_cb, KuhnAct::Bet) < 0.04,
            "P0 with J facing a check-bet must fold: {:?}",
            p0_j_after_cb
        );

        // Convergence is monotone-ish on a log scale; the last recorded
        // exploitability should be lower than the first.
        let first = solver.history.points.first().expect("recorded points");
        let last = solver.history.points.last().expect("recorded points");
        assert!(last.exploitability < first.exploitability);
    }

    /// Diagnostic: compare CFR$^+$ and vanilla CFR exploitability curves on
    /// Kuhn poker. Runs both for the same iteration count and prints the
    /// trajectory. Used during paper experiment validation to confirm
    /// CFR$^+$ does in fact dominate vanilla CFR on a game small enough to
    /// reach the asymptotic regime within a few hundred iters.
    #[test]
    fn cfr_vs_cfr_plus_kuhn_trajectory() {
        let mut plus = CFRSolver::with_variant(Kuhn, CfrVariant::Plus);
        plus.run(2000);
        let mut vanilla = CFRSolver::with_variant(Kuhn, CfrVariant::Vanilla);
        vanilla.run(2000);

        println!("\niter\tCFR+\tCFR");
        for (a, b) in plus.history.points.iter().zip(vanilla.history.points.iter()) {
            println!(
                "{}\t{:.5}\t{:.5}",
                a.iteration, a.exploitability, b.exploitability
            );
        }
        let plus_last = plus.history.points.last().unwrap().exploitability;
        let vanilla_last = vanilla.history.points.last().unwrap().exploitability;
        println!("final: CFR+ = {plus_last:.5}, CFR = {vanilla_last:.5}");
        assert!(
            plus_last <= vanilla_last * 1.2,
            "expected CFR+ to be at least as fast as vanilla CFR on Kuhn at 2000 iters: \
             plus={plus_last:.5}, vanilla={vanilla_last:.5}"
        );
    }
}
