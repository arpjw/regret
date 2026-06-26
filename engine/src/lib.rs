//! Regret — CFR+ Nash equilibrium solver for Heads-Up No-Limit Hold'em.
//!
//! The crate is organized around a `Game` trait that CFR+ operates on. The
//! production target is `hunl::HunlFlop` (flop subgame solving). A minimal
//! `kuhn::Kuhn` implementation lives behind `#[cfg(test)]` and is used to
//! verify the CFR+ implementation reaches the known Nash equilibrium.

pub mod abstraction;
pub mod cfr;
pub mod eval;
pub mod game;
pub mod hunl;
pub mod infoset;

pub use abstraction::{ActionAbstraction, CardAbstraction};
pub use cfr::{CFRSolver, ConvergenceHistory, ConvergencePoint, Game};
pub use eval::{HandEvaluator, HandRank};
pub use game::{Action, AbstractAction, ActionHistory, Card, GameState, Player, Street, Suit};
pub use hunl::{HunlFlop, HunlFlopConfig, HunlFlopState, LastAct, Terminal};
pub use infoset::{InfoSet, InfoSetData};
