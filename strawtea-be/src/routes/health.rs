use axum::{Json, Router, routing::get};
use serde::Serialize;

use crate::state::AppState;

pub fn health_routes() -> Router<AppState> {
    Router::new().route("/healthz", get(healthz))
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn healthz() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}
