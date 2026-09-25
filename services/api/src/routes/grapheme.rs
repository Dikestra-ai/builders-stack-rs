//! POST /grapheme/optimize — Grapheme Memory TSP solver.
//!
//! Accepts a distance matrix or relay-node shorthand and returns the
//! minimum-cost Hamiltonian tour using nearest-neighbour + 2-opt.
//!
//! This is the HTTP surface for api-001. The ArkAChat web layer
//! calls this endpoint when the local TypeScript solver is bypassed
//! (e.g. server-side pre-computation, or from the Adel assistant).

use axum::{http::StatusCode, response::IntoResponse, Json};
use stack_api_types::{
    ErrorResponse, GraphemeOptimizeRequest, GraphemeOptimizeResponse, GraphemeStats, RelayInput,
};

// ---------------------------------------------------------------------------
// Solver (nearest-neighbour + 2-opt, O(n²) — adequate for n ≤ 50 relays)
// ---------------------------------------------------------------------------

fn tour_cost(dist: &[Vec<f64>], tour: &[usize]) -> f64 {
    let n = tour.len();
    let mut cost = 0.0;
    for i in 0..n {
        cost += dist[tour[i]][tour[(i + 1) % n]];
    }
    cost
}

fn nearest_neighbour(dist: &[Vec<f64>], start: usize) -> Vec<usize> {
    let n = dist.len();
    let mut visited = vec![false; n];
    let mut tour = Vec::with_capacity(n);
    let mut current = start;
    visited[current] = true;
    tour.push(current);

    for _ in 1..n {
        let next = (0..n)
            .filter(|&j| !visited[j])
            .min_by(|&a, &b| dist[current][a].partial_cmp(&dist[current][b]).unwrap())
            .unwrap();
        visited[next] = true;
        tour.push(next);
        current = next;
    }
    tour
}

fn two_opt(dist: &[Vec<f64>], tour: &mut Vec<usize>) -> usize {
    let n = tour.len();
    let mut improved = true;
    let mut iters = 0usize;
    while improved {
        improved = false;
        iters += 1;
        for i in 0..n - 1 {
            for j in i + 1..n {
                let a = tour[i];
                let b = tour[(i + 1) % n];
                let c = tour[j];
                let d = tour[(j + 1) % n];
                let before = dist[a][b] + dist[c][d];
                let after = dist[a][c] + dist[b][d];
                if after < before - 1e-10 {
                    tour[i + 1..=j].reverse();
                    improved = true;
                }
            }
        }
    }
    iters
}

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
        // Validate: square matrix
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

    // Solve: pick best starting city (lowest average outbound distance) then 2-opt
    let start = (0..n)
        .min_by(|&a, &b| {
            let avg_a: f64 = dist[a].iter().sum::<f64>() / n as f64;
            let avg_b: f64 = dist[b].iter().sum::<f64>() / n as f64;
            avg_a.partial_cmp(&avg_b).unwrap()
        })
        .unwrap_or(0);

    let mut tour = nearest_neighbour(&dist, start);
    let iters = two_opt(&dist, &mut tour);
    let cost = tour_cost(&dist, &tour);

    // Close the tour
    let mut closed = tour.clone();
    closed.push(closed[0]);

    let hops: Vec<String> = if relay_hosts.is_empty() {
        vec![]
    } else {
        tour.iter().map(|&i| relay_hosts[i].clone()).collect()
    };

    Json(GraphemeOptimizeResponse {
        tour: closed,
        cost,
        relay_hops: hops,
        stats: GraphemeStats {
            n,
            iterations: iters,
            solver: "nearest-neighbour+2-opt".to_string(),
        },
    })
    .into_response()
}
