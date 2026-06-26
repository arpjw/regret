//! Axum HTTP + SSE server for the Regret CFR+ solver.
//!
//! Job lifecycle:
//!
//!   POST /solve              -> spawns a solver task on Tokio, returns job_id
//!   GET  /solve/:id/stream   -> SSE stream of ConvergencePoint events
//!   GET  /strategy/:infoset  -> action probabilities for a stored infoset
//!   GET  /convergence/:id    -> full ConvergenceHistory for a completed job
//!
//! All job state lives in `AppState.jobs`, a `DashMap<job_id, JobHandle>`.
//! Each running job owns a Tokio broadcast channel that progress events are
//! published on; subscribers receive them as SSE events.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use regret_api::server;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let state = Arc::new(server::AppState::default());

    let app = Router::new()
        .merge(server::router())
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = "0.0.0.0:8080".parse().expect("valid bind addr");
    tracing::info!("regret-api listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind 0.0.0.0:8080");
    axum::serve(listener, app).await.expect("serve");
}
