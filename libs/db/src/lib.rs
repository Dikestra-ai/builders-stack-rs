use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;
use time::OffsetDateTime;
use serde::{Serialize, Deserialize};

pub type DbPool = sqlx::PgPool;

// ── Row structs ────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewUser {
    pub email: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Post {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub author_id: Uuid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPost {
    pub title: String,
    pub body: String,
    pub author_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdatePost {
    pub title: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Session {
    pub id: String,
    pub user_id: Uuid,
    pub token: String,
    pub expires_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Account {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider: String,
    pub provider_account_id: String,
    pub created_at: OffsetDateTime,
}

// ── Pool ────────────────────────────────────────────────
pub async fn create_pool(database_url: &str) -> Result<DbPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}

// ── Users ───────────────────────────────────────────────
pub async fn create_user(pool: &DbPool, new_user: &NewUser) -> Result<User, sqlx::Error> {
    sqlx::query_as::<_, User>(
        "INSERT INTO users (email, name) VALUES ($1, $2) RETURNING *"
    )
    .bind(&new_user.email)
    .bind(&new_user.name)
    .fetch_one(pool)
    .await
}

pub async fn get_user_by_id(pool: &DbPool, id: Uuid) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn get_user_by_email(pool: &DbPool, email: &str) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await
}

pub async fn delete_user(pool: &DbPool, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// ── Posts ───────────────────────────────────────────────
pub async fn list_posts(pool: &DbPool) -> Result<Vec<Post>, sqlx::Error> {
    sqlx::query_as::<_, Post>("SELECT * FROM posts ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
}

pub async fn create_post(pool: &DbPool, new_post: &NewPost) -> Result<Post, sqlx::Error> {
    sqlx::query_as::<_, Post>(
        "INSERT INTO posts (title, body, author_id) VALUES ($1, $2, $3) RETURNING *"
    )
    .bind(&new_post.title)
    .bind(&new_post.body)
    .bind(new_post.author_id)
    .fetch_one(pool)
    .await
}

pub async fn get_post(pool: &DbPool, id: Uuid) -> Result<Option<Post>, sqlx::Error> {
    sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn update_post(pool: &DbPool, id: Uuid, patch: &UpdatePost) -> Result<Option<Post>, sqlx::Error> {
    // Dynamic update: only set fields that are Some
    let updated = sqlx::query_as::<_, Post>(
        "UPDATE posts SET
           title = COALESCE($1, title),
           body  = COALESCE($2, body),
           updated_at = now()
         WHERE id = $3
         RETURNING *"
    )
    .bind(&patch.title)
    .bind(&patch.body)
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(updated)
}

pub async fn delete_post(pool: &DbPool, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM posts WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

// ── Sessions ─────────────────────────────────────────────
pub async fn create_session(
    pool: &DbPool,
    session_id: &str,
    user_id: Uuid,
    token: &str,
    expires_at: OffsetDateTime,
) -> Result<Session, sqlx::Error> {
    sqlx::query_as::<_, Session>(
        "INSERT INTO sessions (id, user_id, token, expires_at) VALUES ($1, $2, $3, $4) RETURNING *"
    )
    .bind(session_id)
    .bind(user_id)
    .bind(token)
    .bind(expires_at)
    .fetch_one(pool)
    .await
}

pub async fn get_session_by_token(pool: &DbPool, token: &str) -> Result<Option<Session>, sqlx::Error> {
    sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE token = $1")
        .bind(token)
        .fetch_optional(pool)
        .await
}

pub async fn delete_session_by_token(pool: &DbPool, token: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM sessions WHERE token = $1")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn delete_sessions_for_user(pool: &DbPool, user_id: Uuid) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
