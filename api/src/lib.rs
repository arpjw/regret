//! Library face of the regret-api crate.
//!
//! Exposed so integration tests (and downstream tooling) can call into the
//! job/server logic without going through the HTTP transport.

pub mod jobs;
pub mod server;
