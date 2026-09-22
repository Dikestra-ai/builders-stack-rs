// stack-config — the single typed door to environment variables.
//
// The rule this enforces: never read std::env::var("X") inline with a string
// default scattered across the codebase. Declare it ONCE here (name, type,
// default), then `get_env().field` everywhere. One schema = one source of
// truth for what the app needs.
//
// Fields are optional or defaulted, so the app boots with an empty env in
// local dev (env-gated, non-breaking) and only real misconfig fails fast at
// first read.

use once_cell::sync::Lazy;
use serde::Deserialize;

fn default_node_env() -> String {
    "development".to_string()
}

fn default_web_origin() -> String {
    "http://localhost:3000".to_string()
}

fn default_site_name() -> String {
    "Builder's Stack".to_string()
}

fn default_next_public_site_url() -> String {
    "http://localhost:3000".to_string()
}

fn default_posthog_host() -> String {
    "https://us.i.posthog.com".to_string()
}

/// All environment variables the app uses, parsed once and cached.
///
/// Fields map 1-to-1 to env var names (envy uses the field name uppercased
/// by default, which matches since the fields are already upper-snake here).
#[derive(Debug, Clone, Deserialize)]
pub struct Env {
    /// "development" | "test" | "production", default "development"
    #[serde(default = "default_node_env")]
    pub node_env: String,

    /// Origin the browser app runs on — used by the API for CORS.
    #[serde(default = "default_web_origin")]
    pub web_origin: String,

    /// Public site name — single source for SEO/OG metadata.
    #[serde(default = "default_site_name")]
    pub site_name: String,

    /// Canonical public URL inlined for the browser too.
    #[serde(default = "default_next_public_site_url")]
    pub next_public_site_url: String,

    /// Server-side PostHog key (optional — absent = analytics no-op).
    pub posthog_api_key: Option<String>,

    /// PostHog ingest host.
    #[serde(default = "default_posthog_host")]
    pub posthog_host: String,

    /// Database connection URL (optional — services that don't use a DB omit this).
    pub database_url: Option<String>,

    /// better-auth secret key.
    pub better_auth_secret: Option<String>,

    /// better-auth base URL.
    pub better_auth_url: Option<String>,

    /// Comma-separated list of trusted origins for better-auth CORS.
    pub better_auth_trusted_origins: Option<String>,

    /// HTTP port override (optional — services pick a sensible default if absent).
    pub port: Option<u16>,

    /// Better Stack Logtail source token for remote log shipping.
    pub betterstack_source_token: Option<String>,

    /// Resend API key for transactional email.
    pub resend_api_key: Option<String>,

    /// Resend sender address (e.g. "noreply@example.com").
    pub resend_from: Option<String>,

    /// AI provider API key (OpenAI-compatible).
    pub ai_api_key: Option<String>,

    /// Payment provider identifier (e.g. "creem" or "dodo").
    pub payment_provider: Option<String>,

    /// Creem payment API key.
    pub creem_api_key: Option<String>,

    /// Creem webhook signing secret.
    pub creem_webhook_secret: Option<String>,

    /// Dodo Payments API key.
    pub dodo_api_key: Option<String>,

    /// GitHub OAuth app client ID.
    pub github_client_id: Option<String>,

    /// GitHub OAuth app client secret.
    pub github_client_secret: Option<String>,

    /// AI worker demo flag / endpoint (optional feature gate).
    pub ai_worker_demo: Option<String>,
}

/// Parse once, cache for the process lifetime.
///
/// Loads `.env.local` first (dotenvy, silent if absent), overriding any
/// existing process environment vars, then reads the process environment.
/// Panics on first call if required vars are missing or values are malformed
/// — fail-fast is intentional.
static ENV: Lazy<Env> = Lazy::new(|| {
    // Load .env.local (or plain .env as fallback) and override existing env.
    let _ = dotenvy::dotenv_override();

    envy::from_env::<Env>()
        .expect("Failed to parse environment variables into Env struct.")
});

/// Return a reference to the parsed, cached `Env`.
///
/// The first call triggers `.env.local` loading and validation; subsequent
/// calls are free (static reference).
pub fn get_env() -> &'static Env {
    &ENV
}

/// Convenience accessor for site identity used by SEO helpers.
///
/// Returns `(site_name, canonical_url)` from the cached env.
pub fn site_config() -> (&'static str, &'static str) {
    let e = get_env();
    (e.site_name.as_str(), e.next_public_site_url.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_node_env_is_development() {
        assert_eq!(default_node_env(), "development");
    }

    #[test]
    fn default_web_origin_is_localhost() {
        assert_eq!(default_web_origin(), "http://localhost:3000");
    }

    #[test]
    fn default_site_name_value() {
        assert_eq!(default_site_name(), "Builder's Stack");
    }

    #[test]
    fn default_posthog_host_value() {
        assert_eq!(default_posthog_host(), "https://us.i.posthog.com");
    }
}
