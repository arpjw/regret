//! 7-card hand evaluator.
//!
//! For 5-card inputs, each hand is mapped to a canonical rank in `0..=7461`
//! where `0` is a royal flush and `7461` is the worst 7-high. For 6/7-card
//! inputs, all `C(n, 5)` subsets are enumerated and the minimum (best) rank
//! is returned.

use crate::game::Card;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HandRank(pub u16);

impl HandRank {
    pub const BEST: HandRank = HandRank(0);
    pub const WORST: HandRank = HandRank(7461);
}

// Category bases (best = lowest):
//   straight flush:   0    (10)
//   four of a kind:   10   (156)
//   full house:       166  (156)
//   flush:            322  (1277)
//   straight:         1599 (10)
//   three of a kind:  1609 (858)
//   two pair:         2467 (858)
//   one pair:         3325 (2860)
//   high card:        6185 (1277)
const SF_BASE: u16 = 0;
const QUADS_BASE: u16 = 10;
const FH_BASE: u16 = 166;
const FLUSH_BASE: u16 = 322;
const STRAIGHT_BASE: u16 = 1599;
const TRIPS_BASE: u16 = 1609;
const TWO_PAIR_BASE: u16 = 2467;
const PAIR_BASE: u16 = 3325;
const HIGH_BASE: u16 = 6185;

struct RankTables {
    flush: HashMap<[u8; 5], u16>,
    high: HashMap<[u8; 5], u16>,
    trips: HashMap<(u8, u8, u8), u16>,
    two_pair: HashMap<(u8, u8, u8), u16>,
    pair: HashMap<(u8, u8, u8, u8), u16>,
}

static TABLES: OnceLock<RankTables> = OnceLock::new();

fn tables() -> &'static RankTables {
    TABLES.get_or_init(build_tables)
}

fn build_tables() -> RankTables {
    // -- flush + high card: 5 distinct ranks, no straights for flushes
    let mut combos: Vec<[u8; 5]> = Vec::new();
    for a in 2..=14u8 {
        for b in 2..a {
            for c in 2..b {
                for d in 2..c {
                    for e in 2..d {
                        combos.push([a, b, c, d, e]);
                    }
                }
            }
        }
    }
    combos.sort_by(|x, y| y.cmp(x));

    let mut flush = HashMap::with_capacity(1277);
    let mut high = HashMap::with_capacity(1277);
    let mut fi: u16 = 0;
    let mut hi: u16 = 0;
    for c in &combos {
        if is_straight_ranks(c) {
            continue;
        }
        flush.insert(*c, fi);
        high.insert(*c, hi);
        fi += 1;
        hi += 1;
    }
    debug_assert_eq!(fi, 1277);
    debug_assert_eq!(hi, 1277);

    // -- three of a kind: (trips, k1, k2) with k1 > k2, neither equal to trips
    let mut trip_combos: Vec<(u8, u8, u8)> = Vec::new();
    for trips in 2..=14u8 {
        for k1 in 2..=14u8 {
            if k1 == trips {
                continue;
            }
            for k2 in 2..k1 {
                if k2 == trips {
                    continue;
                }
                trip_combos.push((trips, k1, k2));
            }
        }
    }
    // sort by: trips desc, k1 desc, k2 desc
    trip_combos.sort_by(|a, b| (b.0, b.1, b.2).cmp(&(a.0, a.1, a.2)));
    let trips: HashMap<(u8, u8, u8), u16> = trip_combos
        .iter()
        .enumerate()
        .map(|(i, &c)| (c, i as u16))
        .collect();
    debug_assert_eq!(trips.len(), 858);

    // -- two pair: (high_pair, low_pair, kicker)
    let mut tp_combos: Vec<(u8, u8, u8)> = Vec::new();
    for hp in 3..=14u8 {
        for lp in 2..hp {
            for kicker in 2..=14u8 {
                if kicker == hp || kicker == lp {
                    continue;
                }
                tp_combos.push((hp, lp, kicker));
            }
        }
    }
    tp_combos.sort_by(|a, b| (b.0, b.1, b.2).cmp(&(a.0, a.1, a.2)));
    let two_pair: HashMap<(u8, u8, u8), u16> = tp_combos
        .iter()
        .enumerate()
        .map(|(i, &c)| (c, i as u16))
        .collect();
    debug_assert_eq!(two_pair.len(), 858);

    // -- one pair: (pair, k1, k2, k3) with k1 > k2 > k3, none equal to pair
    let mut p_combos: Vec<(u8, u8, u8, u8)> = Vec::new();
    for pair in 2..=14u8 {
        for k1 in 2..=14u8 {
            if k1 == pair {
                continue;
            }
            for k2 in 2..k1 {
                if k2 == pair {
                    continue;
                }
                for k3 in 2..k2 {
                    if k3 == pair {
                        continue;
                    }
                    p_combos.push((pair, k1, k2, k3));
                }
            }
        }
    }
    p_combos.sort_by(|a, b| (b.0, b.1, b.2, b.3).cmp(&(a.0, a.1, a.2, a.3)));
    let pair: HashMap<(u8, u8, u8, u8), u16> = p_combos
        .iter()
        .enumerate()
        .map(|(i, &c)| (c, i as u16))
        .collect();
    debug_assert_eq!(pair.len(), 2860);

    RankTables {
        flush,
        high,
        trips,
        two_pair,
        pair,
    }
}

/// Returns `(top_card, is_straight)`. For the wheel (A-2-3-4-5), the top is `5`.
fn detect_straight(ranks_desc: &[u8; 5]) -> (u8, bool) {
    // Check for distinct ranks first.
    for i in 1..5 {
        if ranks_desc[i] == ranks_desc[i - 1] {
            return (0, false);
        }
    }
    if ranks_desc[0] - ranks_desc[4] == 4 {
        return (ranks_desc[0], true);
    }
    // Wheel: A-5-4-3-2
    if ranks_desc[0] == 14
        && ranks_desc[1] == 5
        && ranks_desc[2] == 4
        && ranks_desc[3] == 3
        && ranks_desc[4] == 2
    {
        return (5, true);
    }
    (0, false)
}

fn is_straight_ranks(ranks_desc: &[u8; 5]) -> bool {
    detect_straight(ranks_desc).1
}

fn straight_flush_rank(top: u8) -> u16 {
    // top in {14, 13, 12, ..., 5} → {0, 1, ..., 9}
    SF_BASE + (14 - top) as u16
}

fn straight_rank(top: u8) -> u16 {
    STRAIGHT_BASE + (14 - top) as u16
}

fn quads_rank(quad: u8, kicker: u8) -> u16 {
    // (14 - quad) * 12 + position-of-kicker-among-other-ranks-descending
    let mut pos: u16 = 0;
    for r in (2..=14u8).rev() {
        if r == quad {
            continue;
        }
        if r == kicker {
            return QUADS_BASE + (14 - quad) as u16 * 12 + pos;
        }
        pos += 1;
    }
    unreachable!("quads_rank: kicker {} not found", kicker);
}

fn fullhouse_rank(trips: u8, pair: u8) -> u16 {
    let mut pos: u16 = 0;
    for r in (2..=14u8).rev() {
        if r == trips {
            continue;
        }
        if r == pair {
            return FH_BASE + (14 - trips) as u16 * 12 + pos;
        }
        pos += 1;
    }
    unreachable!("fullhouse_rank: pair {} not found", pair);
}

fn evaluate_5(cards: &[Card; 5]) -> HandRank {
    let mut sorted = *cards;
    sorted.sort_unstable_by(|a, b| b.rank.cmp(&a.rank));
    let ranks: [u8; 5] = [
        sorted[0].rank,
        sorted[1].rank,
        sorted[2].rank,
        sorted[3].rank,
        sorted[4].rank,
    ];
    let is_flush = sorted.iter().all(|c| c.suit == sorted[0].suit);
    let (straight_top, is_straight) = detect_straight(&ranks);

    if is_straight && is_flush {
        return HandRank(straight_flush_rank(straight_top));
    }

    // Bucket ranks by frequency, sorted by (count desc, rank desc).
    let mut counts: [u8; 15] = [0; 15];
    for r in &ranks {
        counts[*r as usize] += 1;
    }
    let mut grouped: Vec<(u8, u8)> = (2..=14u8)
        .filter(|r| counts[*r as usize] > 0)
        .map(|r| (counts[r as usize], r))
        .collect();
    grouped.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));

    let pattern: Vec<u8> = grouped.iter().map(|g| g.0).collect();
    let t = tables();

    match pattern.as_slice() {
        [4, 1] => HandRank(quads_rank(grouped[0].1, grouped[1].1)),
        [3, 2] => HandRank(fullhouse_rank(grouped[0].1, grouped[1].1)),
        _ if is_flush => HandRank(FLUSH_BASE + t.flush[&ranks]),
        _ if is_straight => HandRank(straight_rank(straight_top)),
        [3, 1, 1] => HandRank(TRIPS_BASE + t.trips[&(grouped[0].1, grouped[1].1, grouped[2].1)]),
        [2, 2, 1] => {
            HandRank(TWO_PAIR_BASE + t.two_pair[&(grouped[0].1, grouped[1].1, grouped[2].1)])
        }
        [2, 1, 1, 1] => HandRank(
            PAIR_BASE
                + t.pair[&(grouped[0].1, grouped[1].1, grouped[2].1, grouped[3].1)],
        ),
        [1, 1, 1, 1, 1] => HandRank(HIGH_BASE + t.high[&ranks]),
        _ => unreachable!("unexpected rank pattern: {:?}", pattern),
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct HandEvaluator;

impl HandEvaluator {
    pub fn new() -> Self {
        // Force table init so the first call's latency is bounded.
        let _ = tables();
        Self
    }

    pub fn evaluate(&self, cards: &[Card]) -> HandRank {
        assert!(
            (5..=7).contains(&cards.len()),
            "evaluator expects 5..=7 cards, got {}",
            cards.len()
        );
        if cards.len() == 5 {
            return evaluate_5(cards.try_into().expect("checked length"));
        }
        let n = cards.len();
        let mut best = HandRank(u16::MAX);
        // C(n, 5) subsets
        for a in 0..n - 4 {
            for b in a + 1..n - 3 {
                for c in b + 1..n - 2 {
                    for d in c + 1..n - 1 {
                        for e in d + 1..n {
                            let sub = [cards[a], cards[b], cards[c], cards[d], cards[e]];
                            let r = evaluate_5(&sub);
                            if r < best {
                                best = r;
                            }
                        }
                    }
                }
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Card;

    fn c(s: &str) -> Card {
        Card::parse(s).unwrap_or_else(|| panic!("bad card: {}", s))
    }

    #[test]
    fn royal_flush_is_best() {
        let h = [c("As"), c("Ks"), c("Qs"), c("Js"), c("Ts")];
        assert_eq!(HandEvaluator::new().evaluate(&h), HandRank(0));
    }

    #[test]
    fn straight_flush_below_royal() {
        let royal = [c("As"), c("Ks"), c("Qs"), c("Js"), c("Ts")];
        let kh = [c("Ks"), c("Qs"), c("Js"), c("Ts"), c("9s")];
        let wheel_sf = [c("5s"), c("4s"), c("3s"), c("2s"), c("As")];
        let ev = HandEvaluator::new();
        assert!(ev.evaluate(&royal) < ev.evaluate(&kh));
        assert!(ev.evaluate(&kh) < ev.evaluate(&wheel_sf));
        assert_eq!(ev.evaluate(&wheel_sf), HandRank(9));
    }

    #[test]
    fn category_ordering() {
        let ev = HandEvaluator::new();
        let quads = [c("As"), c("Ad"), c("Ah"), c("Ac"), c("Kd")];
        let fh = [c("As"), c("Ad"), c("Ah"), c("Kc"), c("Kd")];
        let flush = [c("As"), c("Ts"), c("8s"), c("5s"), c("2s")];
        let straight = [c("Ah"), c("Kd"), c("Qc"), c("Js"), c("Th")];
        let trips = [c("As"), c("Ad"), c("Ah"), c("Kc"), c("Qd")];
        let two_pair = [c("As"), c("Ad"), c("Kh"), c("Kc"), c("Qd")];
        let pair = [c("As"), c("Ad"), c("Kh"), c("Qc"), c("Jd")];
        let high = [c("As"), c("Kd"), c("Qh"), c("Jc"), c("9d")];

        assert!(ev.evaluate(&quads) < ev.evaluate(&fh));
        assert!(ev.evaluate(&fh) < ev.evaluate(&flush));
        assert!(ev.evaluate(&flush) < ev.evaluate(&straight));
        assert!(ev.evaluate(&straight) < ev.evaluate(&trips));
        assert!(ev.evaluate(&trips) < ev.evaluate(&two_pair));
        assert!(ev.evaluate(&two_pair) < ev.evaluate(&pair));
        assert!(ev.evaluate(&pair) < ev.evaluate(&high));
        assert!(ev.evaluate(&high).0 <= 7461);
    }

    #[test]
    fn wheel_straight_below_six_high() {
        let ev = HandEvaluator::new();
        let six_high = [c("6h"), c("5d"), c("4c"), c("3s"), c("2h")];
        let wheel = [c("5h"), c("4d"), c("3c"), c("2s"), c("Ah")];
        assert!(ev.evaluate(&six_high) < ev.evaluate(&wheel));
    }

    #[test]
    fn seven_card_picks_best_subset() {
        let ev = HandEvaluator::new();
        // A♠ K♠ Q♠ J♠ T♠ in a 7-card hand should rank as royal flush.
        let hand = [c("As"), c("Ks"), c("Qs"), c("Js"), c("Ts"), c("2c"), c("3d")];
        assert_eq!(ev.evaluate(&hand), HandRank(0));
    }

    #[test]
    fn worst_hand_is_seven_high() {
        let ev = HandEvaluator::new();
        // 7-5-4-3-2 of mixed suits — the worst possible 5-card hand
        let h = [c("7h"), c("5d"), c("4c"), c("3s"), c("2h")];
        assert_eq!(ev.evaluate(&h), HandRank(7461));
    }

    #[test]
    fn distinct_ranks_count_to_7462() {
        // The table sizes must cover all 7462 equivalence classes.
        let t = tables();
        assert_eq!(t.flush.len(), 1277);
        assert_eq!(t.high.len(), 1277);
        assert_eq!(t.trips.len(), 858);
        assert_eq!(t.two_pair.len(), 858);
        assert_eq!(t.pair.len(), 2860);
        // categories with closed-form ranks: straight flush 10, quads 156, fh 156, straight 10
        let closed = 10 + 156 + 156 + 10;
        let table_total = 1277 + 1277 + 858 + 858 + 2860;
        assert_eq!(closed + table_total, 7462);
    }
}
