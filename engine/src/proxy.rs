//! Cross-abstraction exploitability evaluation.
//!
//! A strategy solved in an abstracted game $G_K$ is keyed by $K$-buckets and
//! action history. To measure how it performs against a richer evaluation
//! game $G_{K'}$ (the "proxy" — typically $K' = 200$ for card abstraction or
//! a fuller bet-size set for action abstraction), we project the
//! $K$-strategy onto $G_{K'}$ and compute its exploitability there.
//!
//! **Card-abstraction projection.** Each $K'$-bucket $b'$ contains a set of
//! holes; under the uniform single-hole prior over the residual deck, the
//! conditional $\Pr(K\text{-bucket} = b \mid K'\text{-bucket} = b')$ is the
//! share of holes in $b'$ that also map to $b$. The projected average
//! strategy at $(b', H, p)$ is the $b$-weighted mixture of the source
//! strategy at $(b, H, p)$.
//!
//! **Action-abstraction projection.** Decisions in the evaluation game may
//! offer bet sizes the source strategy never trained on. The convention
//! adopted here is to route any unseen size's probability mass to the
//! nearest available size in the source abstraction (snap-to-grid), keeping
//! the projection mass-preserving without inventing new behaviour.
//!
//! The projected policy is materialized into a fresh `CFRSolver` whose only
//! purpose is to host the strategy; we then call the existing
//! `best_response_value` machinery on the evaluation game to obtain
//! exploitability.

use crate::abstraction::{ActionAbstraction, CardAbstraction};
use crate::cfr::{CFRSolver, Game};
use crate::game::AbstractAction;
use crate::hunl::{HunlFlop, HunlFlopConfig, HunlFlopState, LastAct, Terminal};
use crate::infoset::InfoSetData;
use std::collections::HashMap;

/// Evaluate the exploitability of a small-$K$ HUNL flop strategy inside a
/// larger-$K'$ HUNL flop game on the same board, stacks, and action set.
///
/// Returns exploitability of the projected strategy *in the proxy game*,
/// measured in the same units as the proxy game's utility (typically big
/// blinds, centred such that Nash exploitability is 0).
pub fn cross_card_exploitability(
    source: &CFRSolver<HunlFlop>,
    proxy_config: HunlFlopConfig,
) -> f64 {
    let proxy = HunlFlop::new(proxy_config);

    let source_buckets = source.game.bucket_of_hole_map();
    let proxy_buckets = proxy.bucket_of_hole_map();

    let k_source = source.game.config.card_buckets as usize;
    let k_proxy = proxy.config.card_buckets as usize;

    // weight[b_proxy][b_source] = #(holes with bucket_source = b_source AND
    // bucket_proxy = b_proxy) / #(holes with bucket_proxy = b_proxy).
    let mut counts = vec![vec![0u64; k_source]; k_proxy];
    let mut totals = vec![0u64; k_proxy];
    for (hole, &b_proxy) in &proxy_buckets {
        let b_source = *source_buckets
            .get(hole)
            .expect("source and proxy must share the same residual deck");
        counts[b_proxy as usize][b_source as usize] += 1;
        totals[b_proxy as usize] += 1;
    }
    let mut weights: Vec<Vec<f64>> = vec![Vec::new(); k_proxy];
    for b_proxy in 0..k_proxy {
        let t = totals[b_proxy];
        if t == 0 {
            weights[b_proxy] = vec![0.0; k_source];
            continue;
        }
        weights[b_proxy] = counts[b_proxy]
            .iter()
            .map(|&c| c as f64 / t as f64)
            .collect();
    }

    let mut proxy_solver = CFRSolver::new(proxy);
    project_decisions(source, &mut proxy_solver, &weights);
    proxy_solver.exploitability()
}

/// Evaluate the exploitability of a strategy trained with a small bet-size
/// set inside a richer-action proxy game.
///
/// The card abstraction must match between source and proxy (same `K`,
/// same `abstraction_seed`, same `equity_rollouts`) — only the bet-size set
/// differs. At any decision node the proxy may offer sizes the source
/// strategy doesn't know; those sizes' probability mass is snapped to the
/// nearest available source size.
pub fn cross_action_exploitability(
    source: &CFRSolver<HunlFlop>,
    proxy_config: HunlFlopConfig,
) -> f64 {
    assert_eq!(
        source.game.config.card_buckets, proxy_config.card_buckets,
        "cross_action_exploitability requires matching card abstractions"
    );
    assert_eq!(source.game.config.abstraction_seed, proxy_config.abstraction_seed);

    let proxy = HunlFlop::new(proxy_config);
    let source_sizes = source.game.config.bet_sizes.clone();
    let proxy_sizes = proxy.config.bet_sizes.clone();

    // Map each proxy size-bucket index to the closest source size-bucket
    // index by fractional value. `u8::MAX` is reserved for all-in and maps
    // through unchanged.
    let proxy_to_source_size: Vec<u8> = proxy_sizes
        .iter()
        .map(|&p| {
            let mut best = 0usize;
            let mut best_d = f64::INFINITY;
            for (i, &s) in source_sizes.iter().enumerate() {
                let d = (s - p).abs();
                if d < best_d {
                    best_d = d;
                    best = i;
                }
            }
            best as u8
        })
        .collect();

    let mut proxy_solver = CFRSolver::new(proxy);
    project_action_decisions(source, &mut proxy_solver, &proxy_to_source_size);
    proxy_solver.exploitability()
}

/// Walk the proxy game's post-chance action tree once, and for every
/// `(bucket, history, player)` decision infoset compute the projected
/// strategy by mixing the source solver's per-source-bucket strategy under
/// the projection weights. The result is stored in `proxy_solver.data` so
/// that `proxy_solver.exploitability()` (which reads `average_strategy`)
/// returns the cross-abstraction exploitability.
fn project_decisions(
    source: &CFRSolver<HunlFlop>,
    proxy_solver: &mut CFRSolver<HunlFlop>,
    weights: &[Vec<f64>], // [b_proxy][b_source] -> weight
) {
    let k_proxy = proxy_solver.game.config.card_buckets as usize;

    // Enumerate decision nodes (history, actor, num_actions) once by
    // walking the action sub-tree from a representative post-chance state.
    // For each, replicate the projected policy across every proxy bucket.
    let decisions = enumerate_decisions(&proxy_solver.game);

    for dec in &decisions {
        let n = dec.num_actions;
        for b_proxy in 0..k_proxy {
            // Build projected strategy: weighted sum over source buckets.
            let mut projected = vec![0.0_f64; n];
            for (b_source, &w) in weights[b_proxy].iter().enumerate() {
                if w == 0.0 {
                    continue;
                }
                let source_key = HunlFlop::infoset_key_from_components(
                    b_source as u32,
                    &dec.history,
                    dec.actor,
                );
                let src_strat = match source.data.get(&source_key) {
                    Some(d) => d.average_strategy(),
                    // Untouched infoset in source solver -> uniform.
                    None => vec![1.0 / n as f64; n],
                };
                let len = src_strat.len().min(n);
                for a in 0..len {
                    projected[a] += w * src_strat[a];
                }
            }
            // Normalize defensively (should already sum to ~1 if weights do).
            let total: f64 = projected.iter().sum();
            if total > 0.0 {
                for v in projected.iter_mut() {
                    *v /= total;
                }
            } else {
                for v in projected.iter_mut() {
                    *v = 1.0 / n as f64;
                }
            }
            let key = HunlFlop::infoset_key_from_components(
                b_proxy as u32,
                &dec.history,
                dec.actor,
            );
            install_policy(&proxy_solver.data, key, &projected);
        }
    }
}

fn project_action_decisions(
    source: &CFRSolver<HunlFlop>,
    proxy_solver: &mut CFRSolver<HunlFlop>,
    proxy_to_source_size: &[u8],
) {
    let k = proxy_solver.game.config.card_buckets as usize;
    let proxy_decisions = enumerate_decisions(&proxy_solver.game);
    let source_sizes = source.game.config.bet_sizes.clone();
    let proxy_sizes = proxy_solver.game.config.bet_sizes.clone();

    for dec in &proxy_decisions {
        // Translate the proxy-side history into a source-side history by
        // snapping each Bet(idx)/Raise(idx) to the nearest source-size idx.
        let translated: Vec<AbstractAction> = dec
            .history
            .iter()
            .map(|a| translate_action(a, proxy_to_source_size))
            .collect();

        let n = dec.num_actions;
        // Reconstruct what action vectors look like in each game at this
        // decision so we can map each source action to its corresponding
        // proxy action by name (Fold/Call/Check/AllIn) or by closest pot
        // fraction (Bet/Raise).
        let source_actions = action_vector(&source.game, &translated, dec.actor);
        let proxy_actions = action_vector(&proxy_solver.game, &dec.history, dec.actor);
        let source_to_proxy_action = align_source_to_proxy(
            &source_actions,
            &proxy_actions,
            &source_sizes,
            &proxy_sizes,
        );

        for b in 0..k {
            let source_key = HunlFlop::infoset_key_from_components(
                b as u32,
                &translated,
                dec.actor,
            );
            let src_strat = match source.data.get(&source_key) {
                Some(d) => d.average_strategy(),
                None => vec![1.0 / source_actions.len().max(1) as f64; source_actions.len().max(1)],
            };

            // Place each source action's probability mass onto its matching
            // proxy action. Sizes the proxy offers but the source never
            // trained on receive zero mass — this matches the "snap to
            // nearest trained size" play convention.
            let mut projected = vec![0.0_f64; n];
            for (source_idx, &proxy_idx) in source_to_proxy_action.iter().enumerate() {
                if let Some(pa) = proxy_idx {
                    if source_idx < src_strat.len() && pa < n {
                        projected[pa] += src_strat[source_idx];
                    }
                }
            }
            let total: f64 = projected.iter().sum();
            if total > 0.0 {
                for v in projected.iter_mut() {
                    *v /= total;
                }
            } else {
                for v in projected.iter_mut() {
                    *v = 1.0 / n as f64;
                }
            }
            let key = HunlFlop::infoset_key_from_components(
                b as u32,
                &dec.history,
                dec.actor,
            );
            install_policy(&proxy_solver.data, key, &projected);
        }
    }
    // ActionAbstraction lookups for the source aren't used during BR — only
    // proxy_solver.game is queried. Keep references alive for the borrow
    // checker without warnings.
    let _ = (CardAbstraction::new(1, 1, 0), ActionAbstraction::new(vec![]));
}

/// For each source action, find the index of the proxy action that should
/// receive its probability mass. Returns `None` if no proxy action matches
/// (this only happens for degenerate degenerate stack/facing edge cases).
fn align_source_to_proxy(
    source_actions: &[AbstractAction],
    proxy_actions: &[AbstractAction],
    source_sizes: &[f64],
    proxy_sizes: &[f64],
) -> Vec<Option<usize>> {
    source_actions
        .iter()
        .map(|sa| match *sa {
            AbstractAction::Fold => proxy_actions
                .iter()
                .position(|a| matches!(a, AbstractAction::Fold)),
            AbstractAction::Call => proxy_actions
                .iter()
                .position(|a| matches!(a, AbstractAction::Call)),
            AbstractAction::Check => proxy_actions
                .iter()
                .position(|a| matches!(a, AbstractAction::Check)),
            AbstractAction::Bet(idx) if idx == u8::MAX => proxy_actions
                .iter()
                .position(|a| matches!(a, AbstractAction::Bet(i) if *i == u8::MAX)),
            AbstractAction::Raise(idx) if idx == u8::MAX => proxy_actions
                .iter()
                .position(|a| matches!(a, AbstractAction::Raise(i) if *i == u8::MAX)),
            AbstractAction::Bet(idx) => {
                let frac = source_sizes[idx as usize];
                closest_proxy(proxy_actions, proxy_sizes, frac, /*raise=*/ false)
            }
            AbstractAction::Raise(idx) => {
                let frac = source_sizes[idx as usize];
                closest_proxy(proxy_actions, proxy_sizes, frac, /*raise=*/ true)
            }
        })
        .collect()
}

fn closest_proxy(
    proxy_actions: &[AbstractAction],
    proxy_sizes: &[f64],
    frac: f64,
    raise: bool,
) -> Option<usize> {
    proxy_actions
        .iter()
        .enumerate()
        .filter_map(|(i, a)| match (raise, a) {
            (false, AbstractAction::Bet(pidx)) if *pidx != u8::MAX => {
                Some((i, (proxy_sizes[*pidx as usize] - frac).abs()))
            }
            (true, AbstractAction::Raise(pidx)) if *pidx != u8::MAX => {
                Some((i, (proxy_sizes[*pidx as usize] - frac).abs()))
            }
            _ => None,
        })
        .min_by(|(_, da), (_, db)| da.partial_cmp(db).unwrap())
        .map(|(i, _)| i)
}

fn translate_action(a: &AbstractAction, proxy_to_source_size: &[u8]) -> AbstractAction {
    match *a {
        AbstractAction::Bet(idx) if idx != u8::MAX => {
            AbstractAction::Bet(proxy_to_source_size[idx as usize])
        }
        AbstractAction::Raise(idx) if idx != u8::MAX => {
            AbstractAction::Raise(proxy_to_source_size[idx as usize])
        }
        other => other,
    }
}

fn install_policy(
    data: &dashmap::DashMap<u64, InfoSetData>,
    key: u64,
    probs: &[f64],
) {
    let mut entry = data
        .entry(key)
        .or_insert_with(|| InfoSetData::new(probs.len()));
    entry.regret_sum = vec![0.0; probs.len()];
    entry.strategy_sum = probs.to_vec();
}

struct Decision {
    history: Vec<AbstractAction>,
    actor: usize,
    num_actions: usize,
}

/// All decision infosets (modulo bucket) reachable in the game's action tree.
/// The chance node is collapsed: we walk from a representative post-chance
/// state (any concrete bucket pair works since dynamics don't depend on the
/// bucket value, only on the actions taken).
fn enumerate_decisions(game: &HunlFlop) -> Vec<Decision> {
    let init = HunlFlopState {
        buckets: Some((0, 0)),
        stacks: [game.config.starting_stack, game.config.starting_stack],
        committed: [0.0, 0.0],
        facing: 0.0,
        to_act: 1,
        history: Vec::new(),
        raises_used: 0,
        last: LastAct::None,
        terminal: None,
    };

    let mut out = Vec::new();
    let mut seen = HashMap::new();
    walk(game, &init, &mut out, &mut seen);
    out
}

fn walk(
    game: &HunlFlop,
    s: &HunlFlopState,
    out: &mut Vec<Decision>,
    seen: &mut HashMap<u64, ()>,
) {
    if game.is_terminal(s) {
        return;
    }
    let actor = s.to_act;
    let n = game.num_actions(s);
    // Dedup by (history, actor) — the bucket field doesn't affect the
    // action tree.
    let key = HunlFlop::infoset_key_from_components(0, &s.history, actor);
    if !seen.contains_key(&key) {
        seen.insert(key, ());
        out.push(Decision {
            history: s.history.clone(),
            actor,
            num_actions: n,
        });
    }
    for a in 0..n {
        let ns = game.next_state(s, a);
        if matches!(ns.terminal, Some(Terminal::Fold { .. }) | Some(Terminal::Showdown)) {
            continue;
        }
        walk(game, &ns, out, seen);
    }
}

/// The exact ordered action vector this game offers at `(history, actor)`,
/// reconstructed by replaying the history from the initial state. Used so
/// the projection knows which abstract action sits at which index in the
/// proxy vs the source.
fn action_vector(
    game: &HunlFlop,
    history: &[AbstractAction],
    _actor: usize,
) -> Vec<AbstractAction> {
    let mut s = HunlFlopState {
        buckets: Some((0, 0)),
        stacks: [game.config.starting_stack, game.config.starting_stack],
        committed: [0.0, 0.0],
        facing: 0.0,
        to_act: 1,
        history: Vec::new(),
        raises_used: 0,
        last: LastAct::None,
        terminal: None,
    };
    for ha in history {
        let acts = enumerate_action_set(game, &s);
        let idx = acts
            .iter()
            .position(|a| a == ha)
            .expect("history must replay against the game tree");
        s = game.next_state(&s, idx);
    }
    enumerate_action_set(game, &s)
}

fn enumerate_action_set(game: &HunlFlop, s: &HunlFlopState) -> Vec<AbstractAction> {
    let n = game.num_actions(s);
    (0..n)
        .map(|a| {
            let ns = game.next_state(s, a);
            *ns.history.last().expect("action appended")
        })
        .collect()
}
