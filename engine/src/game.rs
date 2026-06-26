//! Core poker types: cards, streets, actions, and game state.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Suit {
    Clubs = 0,
    Diamonds = 1,
    Hearts = 2,
    Spades = 3,
}

impl Suit {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Suit::Clubs),
            1 => Some(Suit::Diamonds),
            2 => Some(Suit::Hearts),
            3 => Some(Suit::Spades),
            _ => None,
        }
    }

    pub fn as_char(self) -> char {
        match self {
            Suit::Clubs => 'c',
            Suit::Diamonds => 'd',
            Suit::Hearts => 'h',
            Suit::Spades => 's',
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Card {
    pub rank: u8, // 2..=14 (Ace = 14)
    pub suit: Suit,
}

impl Card {
    pub fn new(rank: u8, suit: Suit) -> Self {
        debug_assert!((2..=14).contains(&rank), "rank out of range: {}", rank);
        Self { rank, suit }
    }

    /// Unique index in 0..52 used by the evaluator and abstraction layers.
    pub fn index(self) -> u8 {
        (self.rank - 2) * 4 + self.suit as u8
    }

    pub fn from_index(idx: u8) -> Self {
        assert!(idx < 52, "card index out of range: {}", idx);
        let rank = idx / 4 + 2;
        let suit = Suit::from_u8(idx % 4).expect("suit index in 0..4");
        Self { rank, suit }
    }

    pub fn parse(s: &str) -> Option<Card> {
        let bytes = s.as_bytes();
        if bytes.len() != 2 {
            return None;
        }
        let rank = match bytes[0] {
            b'2'..=b'9' => bytes[0] - b'0',
            b'T' | b't' => 10,
            b'J' | b'j' => 11,
            b'Q' | b'q' => 12,
            b'K' | b'k' => 13,
            b'A' | b'a' => 14,
            _ => return None,
        };
        let suit = match bytes[1] {
            b'c' => Suit::Clubs,
            b'd' => Suit::Diamonds,
            b'h' => Suit::Hearts,
            b's' => Suit::Spades,
            _ => return None,
        };
        Some(Card::new(rank, suit))
    }

    fn rank_char(self) -> char {
        match self.rank {
            10 => 'T',
            11 => 'J',
            12 => 'Q',
            13 => 'K',
            14 => 'A',
            r => char::from_digit(r as u32, 10).unwrap_or('?'),
        }
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank_char(), self.suit.as_char())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Street {
    Preflop,
    Flop,
    Turn,
    River,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Player {
    IP = 0,
    OOP = 1,
}

impl Player {
    pub fn idx(self) -> usize {
        self as usize
    }

    pub fn other(self) -> Player {
        match self {
            Player::IP => Player::OOP,
            Player::OOP => Player::IP,
        }
    }
}

/// Concrete poker action with absolute bet size in big blinds.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Action {
    Fold,
    Check,
    Call,
    Bet(f64),
    Raise(f64),
}

/// Discretized action used as part of an information-set key. Bet/raise sizes
/// are stored as bucket indices into the configured `ActionAbstraction` so
/// that information sets remain countable.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AbstractAction {
    Fold,
    Check,
    Call,
    Bet(u8),   // size bucket index
    Raise(u8), // size bucket index
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ActionHistory {
    pub actions: Vec<AbstractAction>,
}

impl ActionHistory {
    pub fn new() -> Self {
        Self { actions: Vec::new() }
    }

    pub fn push(&mut self, action: AbstractAction) {
        self.actions.push(action);
    }

    pub fn len(&self) -> usize {
        self.actions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn last(&self) -> Option<AbstractAction> {
        self.actions.last().copied()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameState {
    pub hole_cards: [Card; 2],
    pub board: Vec<Card>,
    pub pot: f64,
    pub stacks: [f64; 2],
    pub street: Street,
    pub history: ActionHistory,
    pub active_player: Player,
}

impl GameState {
    pub fn new(
        hole_cards: [Card; 2],
        board: Vec<Card>,
        pot: f64,
        stacks: [f64; 2],
        street: Street,
        active_player: Player,
    ) -> Self {
        Self {
            hole_cards,
            board,
            pot,
            stacks,
            street,
            history: ActionHistory::new(),
            active_player,
        }
    }
}
