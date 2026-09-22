use axum::{
    extract::State,
    http::{header::SET_COOKIE, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use axum_extra::extract::CookieJar;

use crate::AppState;
use stack_api_types::{SignInRequest, SignUpRequest};
use stack_auth::{
    clear_session_cookie, session_cookie, sign_in as auth_sign_in, sign_out as auth_sign_out,
    sign_up as auth_sign_up,
};

fn set_cookie_header(token: &str) -> HeaderValue {
    let cookie = session_cookie(token, false);
    HeaderValue::from_str(&cookie.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static(""))
}

fn clear_cookie_header() -> HeaderValue {
    let cookie = clear_session_cookie();
    HeaderValue::from_str(&cookie.to_string())
        .unwrap_or_else(|_| HeaderValue::from_static(""))
}

pub async fn sign_up(
    State(state): State<AppState>,
    Json(body): Json<SignUpRequest>,
) -> impl IntoResponse {
    match auth_sign_up(&state.pool, &body.email, &body.password, body.name.as_deref()).await {
        Ok(auth_session) => {
            let user_json = serde_json::json!({
                "id": auth_session.user.id.to_string(),
                "email": auth_session.user.email,
                "name": auth_session.user.name,
            });
            let mut response = (StatusCode::CREATED, Json(user_json)).into_response();
            response
                .headers_mut()
                .insert(SET_COOKIE, set_cookie_header(&auth_session.session.token));
            response
        }
        Err(e) => e.into_response(),
    }
}

pub async fn sign_in(
    State(state): State<AppState>,
    Json(body): Json<SignInRequest>,
) -> impl IntoResponse {
    match auth_sign_in(&state.pool, &body.email, &body.password).await {
        Ok(auth_session) => {
            let user_json = serde_json::json!({
                "id": auth_session.user.id.to_string(),
                "email": auth_session.user.email,
                "name": auth_session.user.name,
            });
            let mut response = Json(user_json).into_response();
            response
                .headers_mut()
                .insert(SET_COOKIE, set_cookie_header(&auth_session.session.token));
            response
        }
        Err(e) => e.into_response(),
    }
}

pub async fn sign_out(
    State(state): State<AppState>,
    jar: CookieJar,
) -> impl IntoResponse {
    if let Some(token_cookie) = jar.get(stack_auth::SESSION_COOKIE_NAME) {
        let _ = auth_sign_out(&state.pool, token_cookie.value()).await;
    }
    let mut response = Json(serde_json::json!({ "ok": true })).into_response();
    response.headers_mut().insert(SET_COOKIE, clear_cookie_header());
    response
}
