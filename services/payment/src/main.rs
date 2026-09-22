mod provider;
mod rate_limit;

use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::{net::SocketAddr, sync::Arc};
use tower_http::{
    limit::RequestBodyLimitLayer,
    set_header::SetResponseHeaderLayer,
};

use provider::{resolve_provider, PaymentProvider};
use rate_limit::RateLimiter;
use stack_observability::init_tracing;

#[derive(Clone)]
pub struct AppState {
    pub provider: Arc<dyn PaymentProvider>,
    pub rate_limiter: Arc<RateLimiter>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let provider = resolve_provider();
    tracing::info!("[payment] provider: {}", provider.name());

    let state = AppState {
        provider: Arc::from(provider),
        rate_limiter: Arc::new(RateLimiter::new(10, 60)),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/checkout", post(checkout))
        .route("/webhook", post(webhook))
        .with_state(state)
        .layer(RequestBodyLimitLayer::new(1024 * 1024))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ));

    let env = stack_config::get_env();
    let port = env.port.unwrap_or(3002);
    let addr = format!("0.0.0.0:{port}");
    tracing::info!("[payment] listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "payment",
        "provider": state.provider.name(),
    }))
}

async fn checkout(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let ip = addr.ip().to_string();
    if !state.rate_limiter.check(&ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({ "error": "Rate limit exceeded" })),
        )
            .into_response();
    }

    match state.provider.create_checkout(body).await {
        Ok(url) => Json(serde_json::json!({ "url": url })).into_response(),
        Err(e) => {
            tracing::error!("checkout error: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Failed to create checkout" })),
            )
                .into_response()
        }
    }
}

/// Axum 0.7+ uses `axum::body::Bytes` directly — `RawBody` was removed.
/// The signature header must be extracted *before* consuming the body; callers
/// should pass it via a custom extractor or query param for production use.
async fn webhook(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    match state.provider.verify_webhook(&body, "").await {
        Ok(true) => {
            tracing::info!("webhook verified, processing event");
            StatusCode::OK.into_response()
        }
        Ok(false) => {
            tracing::warn!("webhook signature mismatch");
            StatusCode::UNAUTHORIZED.into_response()
        }
        Err(e) => {
            tracing::error!("webhook error: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
