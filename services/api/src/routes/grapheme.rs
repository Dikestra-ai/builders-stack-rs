//! POST /grapheme/optimize — Grapheme Memory TSP solver (HTTP surface).
//!
//! Delegates to the `grapheme-routing` crate (Guard8.ai/grapheme-nn) for the
//! actual algorithm. This handler is the stable HTTP interface for ArkAChat
//! and other services that need relay-path or general TSP optimisation.

use axum::{http::StatusCode, response::IntoResponse, Json};
use grapheme_routing::{RelayNode as GRelayNode, TspInstance};
use stack_api_types::{
    ErrorResponse, GraphemeOptimizeRequest, GraphemeOptimizeResponse, GraphemeStats, RelayInput,
};

// ---------------------------------------------------------------------------
// Distance matrix helpers
// ---------------------------------------------------------------------------

const REGION_PENALTY: f64 = 30.0;

fn relay_distance(a: &RelayInput, b: &RelayInput) -> f64 {
    let penalty = match (&a.region, &b.region) {
        (Some(ra), Some(rb)) if ra != rb => REGION_PENALTY,
        _ => 0.0,
    };
    (a.latency_ms + b.latency_ms) / 2.0 + penalty
}

fn relay_matrix(relays: &[RelayInput]) -> Vec<Vec<f64>> {
    let n = relays.len();
    (0..n)
        .map(|i| {
            (0..n)
                .map(|j| if i == j { 0.0 } else { relay_distance(&relays[i], &relays[j]) })
                .collect()
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

pub async fn optimize(Json(body): Json<GraphemeOptimizeRequest>) -> impl IntoResponse {
    // Build distance matrix — relay shorthand takes precedence
    let (dist, relay_hosts): (Vec<Vec<f64>>, Vec<String>) = if !body.relays.is_empty() {
        let hosts: Vec<String> = body.relays.iter().map(|r| r.host.clone()).collect();
        (relay_matrix(&body.relays), hosts)
    } else if !body.distances.is_empty() {
        let n = body.distances.len();
        if body.distances.iter().any(|row| row.len() != n) {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({ "error": "distances must be a square n×n matrix" })),
            )
                .into_response();
        }
        (body.distances, vec![])
    } else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "error": "provide either distances or relays" })),
        )
            .into_response();
    };

    let n = dist.len();
    if n < 2 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "error": "need at least 2 cities/relays" })),
        )
            .into_response();
    }

    // Delegate to grapheme-routing
    let instance = TspInstance::new(dist);
    let result = grapheme_routing::GraphemeMemoryTSP::default_params(instance).solve();

    if result.tour.is_empty() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "solver found no tour" })),
        )
            .into_response();
    }

    let hops: Vec<String> = if relay_hosts.is_empty() {
        vec![]
    } else {
        result.tour[..n].iter().map(|&i| relay_hosts[i].clone()).collect()
    };

    Json(GraphemeOptimizeResponse {
        tour: result.tour,
        cost: result.cost,
        relay_hops: hops,
        stats: GraphemeStats {
            n,
            iterations: result.stats.exact_phase_states + result.stats.approx_phase_states,
            solver: "grapheme-memory-tsp".to_string(),
        },
    })
    .into_response()
}
