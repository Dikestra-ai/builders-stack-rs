use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{AppendHeaders, IntoResponse},
    Json,
};

use crate::AppState;
use stack_api_types::{DeleteResponse, ErrorResponse, ExportResponse, MeResponse};
use stack_auth::CurrentUser;
use stack_db::delete_user;

pub async fn get_me(CurrentUser(user): CurrentUser) -> Json<MeResponse> {
    Json(MeResponse {
        id: user.id.to_string(),
        email: user.email,
        name: user.name,
    })
}

pub async fn export_me(CurrentUser(user): CurrentUser) -> impl IntoResponse {
    let response = ExportResponse {
        exported_at: Some(time::OffsetDateTime::now_utc().to_string()),
        user: Some(MeResponse {
            id: user.id.to_string(),
            email: user.email,
            name: user.name,
        }),
    };
    (
        AppendHeaders([(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"my-data.json\"",
        )]),
        Json(response),
    )
        .into_response()
}

pub async fn delete_me(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> impl IntoResponse {
    match delete_user(&state.pool, user.id).await {
        Ok(true) => Json(DeleteResponse {
            deleted: Some(true),
            user_id: Some(user.id.to_string()),
        })
        .into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "User not found" })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("delete_me error: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Internal Server Error".to_string(),
                }),
            )
                .into_response()
        }
    }
}
