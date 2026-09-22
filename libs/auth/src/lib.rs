use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use rand::RngCore;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use stack_db::{DbPool, Session, User};

pub mod github;

pub const SESSION_COOKIE_NAME: &str = "stack.session_token";
pub const SESSION_DURATION_DAYS: i64 = 7;

// ── Error ────────────────────────────────────────────────
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Email already registered")]
    EmailExists,
    #[error("Session expired or not found")]
    SessionExpired,
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Hash error: {0}")]
    HashError(String),
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            AuthError::InvalidCredentials => (StatusCode::UNAUTHORIZED, self.to_string()),
            AuthError::EmailExists => (StatusCode::CONFLICT, self.to_string()),
            AuthError::SessionExpired | AuthError::Unauthorized => {
                (StatusCode::UNAUTHORIZED, self.to_string())
            }
            AuthError::Database(_) | AuthError::HashError(_) | AuthError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
        };
        (status, axum::Json(serde_json::json!({ "error": msg }))).into_response()
    }
}

// ── AuthSession ──────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct AuthSession {
    pub session: Session,
    pub user: User,
}

// ── Password hashing ─────────────────────────────────────
pub fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AuthError::HashError(e.to_string()))
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, AuthError> {
    let parsed = PasswordHash::new(hash).map_err(|e| AuthError::HashError(e.to_string()))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

// ── Token generation ─────────────────────────────────────
pub fn generate_session_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn session_cookie_name() -> &'static str {
    SESSION_COOKIE_NAME
}

// ── Auth operations ──────────────────────────────────────
pub async fn sign_up(
    pool: &DbPool,
    email: &str,
    password: &str,
    name: Option<&str>,
) -> Result<AuthSession, AuthError> {
    // Check email uniqueness
    if stack_db::get_user_by_email(pool, email).await?.is_some() {
        return Err(AuthError::EmailExists);
    }

    let password_hash = hash_password(password)?;

    // Create user with password hash
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (email, name, password_hash) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(email)
    .bind(name)
    .bind(&password_hash)
    .fetch_one(pool)
    .await?;

    let session = create_user_session(pool, user.id).await?;

    // Send welcome email (fire-and-forget)
    let email_to = user.email.clone();
    let user_name = user.name.clone().unwrap_or_else(|| "there".to_string());
    tokio::spawn(async move {
        let args = stack_email::EmailArgs {
            to: email_to,
            template: stack_email::EmailTemplate::Welcome,
            name: user_name,
            extra: Default::default(),
        };
        let _ = stack_email::send_email(args).await;
    });

    tracing::info!(user_id = %user.id, "user.create.after: new user registered");

    Ok(AuthSession { session, user })
}

pub async fn sign_in(
    pool: &DbPool,
    email: &str,
    password: &str,
) -> Result<AuthSession, AuthError> {
    let user = stack_db::get_user_by_email(pool, email)
        .await?
        .ok_or(AuthError::InvalidCredentials)?;

    // Get password hash from DB
    let password_hash: Option<String> =
        sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1")
            .bind(user.id)
            .fetch_optional(pool)
            .await?
            .flatten();

    let hash = password_hash.ok_or(AuthError::InvalidCredentials)?;

    if !verify_password(password, &hash)? {
        return Err(AuthError::InvalidCredentials);
    }

    let session = create_user_session(pool, user.id).await?;

    tracing::info!(
        user_id = %user.id,
        session_id = %session.id,
        "session.create.after: audit log"
    );

    Ok(AuthSession { session, user })
}

pub async fn sign_out(pool: &DbPool, session_token: &str) -> Result<(), AuthError> {
    stack_db::delete_session_by_token(pool, session_token).await?;
    Ok(())
}

pub async fn get_session(
    pool: &DbPool,
    session_token: &str,
) -> Result<Option<AuthSession>, AuthError> {
    let session = match stack_db::get_session_by_token(pool, session_token).await? {
        Some(s) => s,
        None => return Ok(None),
    };

    if session.expires_at < OffsetDateTime::now_utc() {
        // Clean up expired session
        let _ = stack_db::delete_session_by_token(pool, session_token).await;
        return Ok(None);
    }

    let user = match stack_db::get_user_by_id(pool, session.user_id).await? {
        Some(u) => u,
        None => return Ok(None),
    };

    Ok(Some(AuthSession { session, user }))
}

async fn create_user_session(pool: &DbPool, user_id: Uuid) -> Result<Session, AuthError> {
    let session_id = Uuid::new_v4().to_string();
    let token = generate_session_token();
    let expires_at = OffsetDateTime::now_utc() + Duration::days(SESSION_DURATION_DAYS);
    let session =
        stack_db::create_session(pool, &session_id, user_id, &token, expires_at).await?;
    Ok(session)
}

// ── Cookie helpers ───────────────────────────────────────
pub fn session_cookie(token: &str, secure: bool) -> Cookie<'static> {
    let mut cookie = Cookie::new(SESSION_COOKIE_NAME, token.to_owned());
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::days(SESSION_DURATION_DAYS));
    if secure {
        cookie.set_secure(true);
    }
    cookie
}

pub fn clear_session_cookie() -> Cookie<'static> {
    let mut cookie = Cookie::new(SESSION_COOKIE_NAME, "");
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::seconds(0));
    cookie
}

// ── Axum extractor ───────────────────────────────────────
/// Axum extractor that reads the session cookie and returns the current user.
/// Requires axum::extract::State<DbPool> to be in the router state.
#[derive(Debug, Clone)]
pub struct CurrentUser(pub User);

impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
    DbPool: FromRef<S>,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(SESSION_COOKIE_NAME)
            .map(|c| c.value().to_string())
            .ok_or(AuthError::Unauthorized)?;

        let pool = DbPool::from_ref(state);
        let auth_session = get_session(&pool, &token)
            .await?
            .ok_or(AuthError::Unauthorized)?;

        Ok(CurrentUser(auth_session.user))
    }
}
