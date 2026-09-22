/// GitHub OAuth2 scaffold.
///
/// Flow:
///   1. Redirect user to `authorization_url()` with a CSRF state param.
///   2. GitHub redirects back to your callback URL with `?code=...&state=...`.
///   3. Exchange the code for an access token via `exchange_code()`.
///   4. Fetch the user's GitHub profile via `get_github_user()`.
///   5. Upsert a `users` row + `accounts` row (provider="github"), then
///      create a session with `stack_auth::create_user_session` (private).
///
/// Environment variables consumed (via stack-config):
///   GITHUB_CLIENT_ID     — GitHub OAuth App client ID
///   GITHUB_CLIENT_SECRET — GitHub OAuth App client secret
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

const GITHUB_AUTH_URL: &str = "https://github.com/login/oauth/authorize";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_USER_URL: &str = "https://api.github.com/user";

// ── Public types ──────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubUser {
    pub id: u64,
    pub login: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenRequest {
    client_id: String,
    client_secret: String,
    code: String,
    redirect_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

// ── Helpers ───────────────────────────────────────────────

/// Build the URL to redirect the user to for GitHub OAuth consent.
///
/// `state` should be a CSRF-safe random value stored in the user's session
/// and verified when the callback arrives.
pub fn authorization_url(state: &str, redirect_uri: Option<&str>) -> String {
    let client_id = stack_config::get_env()
        .github_client_id
        .as_deref()
        .unwrap_or("");

    let mut url = format!(
        "{}?client_id={}&scope=user:email&state={}",
        GITHUB_AUTH_URL, client_id, state
    );
    if let Some(uri) = redirect_uri {
        url.push_str(&format!("&redirect_uri={}", uri));
    }
    url
}

/// Exchange a GitHub OAuth code for an access token.
pub async fn exchange_code(code: &str, redirect_uri: Option<&str>) -> Result<String> {
    let env = stack_config::get_env();
    let client_id = env
        .github_client_id
        .as_deref()
        .ok_or_else(|| anyhow!("GITHUB_CLIENT_ID not set"))?;
    let client_secret = env
        .github_client_secret
        .as_deref()
        .ok_or_else(|| anyhow!("GITHUB_CLIENT_SECRET not set"))?;

    let body = TokenRequest {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
        code: code.to_string(),
        redirect_uri: redirect_uri.map(|s| s.to_string()),
    };

    let resp: TokenResponse = reqwest::Client::new()
        .post(GITHUB_TOKEN_URL)
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await?
        .json()
        .await?;

    if let Some(err) = resp.error {
        let desc = resp.error_description.unwrap_or_default();
        return Err(anyhow!("GitHub OAuth error {}: {}", err, desc));
    }

    resp.access_token
        .ok_or_else(|| anyhow!("GitHub returned no access_token"))
}

/// Fetch the authenticated user's GitHub profile.
pub async fn get_github_user(access_token: &str) -> Result<GitHubUser> {
    let user: GitHubUser = reqwest::Client::new()
        .get(GITHUB_USER_URL)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "stack-auth/0.1")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .json()
        .await?;
    Ok(user)
}
