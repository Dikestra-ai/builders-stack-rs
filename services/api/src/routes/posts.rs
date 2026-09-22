use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::AppState;
use stack_api_types::{ErrorResponse, NewPost, PatchPost, Post};
use stack_auth::CurrentUser;
use stack_db::{self, Post as DbPost};

fn db_post_to_api(p: DbPost) -> Post {
    Post {
        id: p.id.to_string(),
        title: p.title,
        body: p.body,
        author_id: p.author_id.to_string(),
        created_at: p.created_at.to_string(),
        updated_at: p.updated_at.to_string(),
    }
}

pub async fn list_posts(State(state): State<AppState>) -> impl IntoResponse {
    match stack_db::list_posts(&state.pool).await {
        Ok(posts) => Json(posts.into_iter().map(db_post_to_api).collect::<Vec<_>>()).into_response(),
        Err(e) => {
            tracing::error!("list_posts: {:?}", e);
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

pub async fn create_post(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<NewPost>,
) -> impl IntoResponse {
    if let Err(e) = body.validate() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse { error: e }),
        )
            .into_response();
    }
    let new_post = stack_db::NewPost {
        title: body.title,
        body: body.body,
        author_id: user.id,
    };
    match stack_db::create_post(&state.pool, &new_post).await {
        Ok(post) => (StatusCode::CREATED, Json(db_post_to_api(post))).into_response(),
        Err(e) => {
            tracing::error!("create_post: {:?}", e);
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

pub async fn get_post(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match stack_db::get_post(&state.pool, id).await {
        Ok(Some(post)) => Json(db_post_to_api(post)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Not found".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("get_post: {:?}", e);
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

pub async fn update_post(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<PatchPost>,
) -> impl IntoResponse {
    let existing = match stack_db::get_post(&state.pool, id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Not found".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("update_post fetch: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Internal Server Error".to_string(),
                }),
            )
                .into_response();
        }
    };
    if existing.author_id != user.id {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Forbidden".to_string(),
            }),
        )
            .into_response();
    }
    let patch = stack_db::UpdatePost {
        title: body.title,
        body: body.body,
    };
    match stack_db::update_post(&state.pool, id, &patch).await {
        Ok(Some(post)) => Json(db_post_to_api(post)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Not found".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("update_post: {:?}", e);
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

pub async fn delete_post(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    CurrentUser(user): CurrentUser,
) -> impl IntoResponse {
    let existing = match stack_db::get_post(&state.pool, id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Not found".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("delete_post fetch: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Internal Server Error".to_string(),
                }),
            )
                .into_response();
        }
    };
    if existing.author_id != user.id {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Forbidden".to_string(),
            }),
        )
            .into_response();
    }
    match stack_db::delete_post(&state.pool, id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Not found".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("delete_post: {:?}", e);
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
