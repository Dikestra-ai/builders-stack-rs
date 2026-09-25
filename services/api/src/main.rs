mod error;
mod openapi;
mod routes;

use axum::{
    http::{header, HeaderValue, Method},
    routing::{delete, get, patch, post},
    Router,
};
use std::{sync::Arc, time::Instant};
use tower_http::{
    cors::{AllowHeaders, CorsLayer},
    limit::RequestBodyLimitLayer,
    set_header::SetResponseHeaderLayer,
};

use stack_db::DbPool;
use stack_observability::init_tracing;

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub start_time: Arc<Instant>,
}

impl axum::extract::FromRef<AppState> for DbPool {
    fn from_ref(state: &AppState) -> DbPool {
        state.pool.clone()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let env = stack_config::get_env();

    // Guard: refuse to start with missing auth secret outside dev
    if env.node_env != "development" && env.better_auth_secret.is_none() {
        tracing::error!("FATAL: BETTER_AUTH_SECRET is not set. Refusing to start.");
        std::process::exit(1);
    }

    let database_url = env
        .database_url
        .as_ref()
        .expect("DATABASE_URL is required");

    let pool = stack_db::create_pool(database_url).await?;
    tracing::info!("Database pool created");

    let state = AppState {
        pool,
        start_time: Arc::new(Instant::now()),
    };

    let web_origin = env.web_origin.clone();
    let cors = CorsLayer::new()
        .allow_origin(
            web_origin
                .parse::<HeaderValue>()
                .unwrap_or(HeaderValue::from_static("http://localhost:3000")),
        )
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(AllowHeaders::any())
        .allow_credentials(true);

    let app = Router::new()
        // Health
        .route("/health", get(routes::health))
        // Auth
        .route("/auth/sign-up", post(routes::auth::sign_up))
        .route("/auth/sign-in", post(routes::auth::sign_in))
        .route("/auth/sign-out", post(routes::auth::sign_out))
        // Me (GDPR)
        .route("/me", get(routes::me::get_me))
        .route("/me/export", get(routes::me::export_me))
        .route("/me/delete", post(routes::me::delete_me))
        // Posts
        .route("/posts", get(routes::posts::list_posts))
        .route("/posts", post(routes::posts::create_post))
        .route("/posts/:id", get(routes::posts::get_post))
        .route("/posts/:id", patch(routes::posts::update_post))
        .route("/posts/:id", delete(routes::posts::delete_post))
        // Grapheme TSP relay optimizer (api-001)
        .route("/grapheme/optimize", post(routes::grapheme::optimize))
        // OpenAPI + Swagger UI
        .route("/openapi.json", get(openapi::openapi_json))
        .route("/docs", get(openapi::swagger_ui))
        .with_state(state)
        .layer(RequestBodyLimitLayer::new(1024 * 1024)) // 1 MB
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(cors);

    let port = env.port.unwrap_or(3001);
    let addr = format!("0.0.0.0:{port}");
    tracing::info!("[api] listening on http://{addr}  (docs: /docs)");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
