//! HTTP routes for the regret-api.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures::stream::{Stream, StreamExt};
use regret_engine::HunlFlop;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;

use crate::jobs::{
    self, active_player_after, parse_action_history, parse_hand_label, JobMap, SolveRequest,
};

#[derive(Default)]
pub struct AppState {
    pub jobs: Arc<JobMap>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/solve", post(solve))
        .route("/solve/:id/stream", get(stream))
        .route("/strategy/:job_id/:infoset", get(strategy_for_job))
        .route("/strategy/:infoset", get(strategy_global))
        .route("/strategy_for_hand/:job_id/:hand", get(strategy_for_hand))
        .route("/convergence/:id", get(convergence))
        .route("/health", get(health))
}

#[derive(Serialize)]
struct SolveResponse {
    job_id: String,
}

async fn health() -> &'static str {
    "ok"
}

async fn solve(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SolveRequest>,
) -> Result<Json<SolveResponse>, (StatusCode, String)> {
    let config = req
        .to_config()
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let id = jobs::spawn_solver(state.jobs.clone(), config, req.max_iterations);
    Ok(Json(SolveResponse { job_id: id }))
}

async fn stream(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let handle = state
        .jobs
        .get(&id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("no such job: {id}")))?
        .clone();

    let rx = handle.progress.subscribe();
    let s = BroadcastStream::new(rx).filter_map(|res| async move {
        match res {
            Ok(evt) => Some(Ok::<_, Infallible>(
                Event::default()
                    .json_data(&evt)
                    .unwrap_or_else(|_| Event::default().data("{}")),
            )),
            Err(_) => None, // lagged subscriber — skip
        }
    });

    Ok(Sse::new(s).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}

async fn convergence(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let handle = state
        .jobs
        .get(&id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("no such job: {id}")))?
        .clone();
    let history = handle.history.read().await.clone();
    Ok(Json(serde_json::json!({
        "job_id": id,
        "points": history.points,
    })))
}

async fn strategy_for_job(
    State(state): State<Arc<AppState>>,
    Path((job_id, infoset)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let handle = state
        .jobs
        .get(&job_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("no such job: {job_id}")))?
        .clone();
    let key: u64 = parse_infoset_key(&infoset)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("bad infoset key: {infoset}")))?;
    let strategies = handle.strategies.read().await;
    let probs = strategies
        .get(&key)
        .cloned()
        .ok_or_else(|| (StatusCode::NOT_FOUND, "no strategy stored for that infoset".into()))?;
    Ok(Json(serde_json::json!({
        "infoset": infoset,
        "probabilities": probs,
    })))
}

/// Convenience endpoint that searches across all jobs. Returns the first
/// match — useful when the caller only has an infoset key.
async fn strategy_global(
    State(state): State<Arc<AppState>>,
    Path(infoset): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let key: u64 = parse_infoset_key(&infoset)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, format!("bad infoset key: {infoset}")))?;
    for entry in state.jobs.iter() {
        let handle = entry.value().clone();
        let strategies = handle.strategies.read().await;
        if let Some(probs) = strategies.get(&key) {
            return Ok(Json(serde_json::json!({
                "infoset": infoset,
                "probabilities": probs,
                "job_id": entry.key().clone(),
            })));
        }
    }
    Err((StatusCode::NOT_FOUND, "infoset not found in any job".into()))
}

fn parse_infoset_key(s: &str) -> Option<u64> {
    if let Some(rest) = s.strip_prefix("0x") {
        u64::from_str_radix(rest, 16).ok()
    } else {
        s.parse::<u64>().ok()
    }
}

#[derive(Deserialize, Default)]
struct HandQuery {
    /// Dot-separated abstract action history. Empty string (or absent) means
    /// the start of the flop, OOP to act.
    #[serde(default)]
    history: String,
}

/// Resolves a hand label (`AA`, `AKs`, `72o`) to the engine's infoset key
/// using the job's shared `HunlFlop` instance and returns the action
/// distribution stored for that key.
async fn strategy_for_hand(
    State(state): State<Arc<AppState>>,
    Path((job_id, hand)): Path<(String, String)>,
    Query(q): Query<HandQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let handle = state
        .jobs
        .get(&job_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("no such job: {job_id}")))?
        .clone();

    let game = handle
        .game
        .read()
        .await
        .clone()
        .ok_or_else(|| (StatusCode::CONFLICT, "job has not initialized its game yet".into()))?;

    let history = parse_action_history(&q.history).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let player = active_player_after(history.len());
    let hole = parse_hand_label(&hand, &game.config.board)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let bucket = game.bucket_for(&hole);
    let key = HunlFlop::infoset_key_from_components(bucket, &history, player);

    let strategies = handle.strategies.read().await;
    let probs = strategies.get(&key).cloned().ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("no strategy stored for hand `{hand}` (infoset {key}) — solver may not have visited this branch yet"),
        )
    })?;

    Ok(Json(serde_json::json!({
        "job_id": job_id,
        "hand": hand,
        "hole": [hole[0].to_string(), hole[1].to_string()],
        "bucket": bucket,
        "history": q.history,
        "player": player,
        "infoset_key": key,
        "probabilities": probs,
    })))
}

