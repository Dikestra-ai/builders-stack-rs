pub mod auth;
pub mod me;
pub mod posts;

use axum::{extract::State, Json};

use crate::AppState;
use stack_api_types::HealthResponse;

pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        service: "api".to_string(),
        uptime: state.start_time.elapsed().as_secs_f64(),
    })
}
