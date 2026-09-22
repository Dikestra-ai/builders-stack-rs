//! Payment provider abstraction.
//!
//! # Provider resolution order
//!
//! 1. `PAYMENT_PROVIDER` env var (`"creem"` | `"dodo"` | `"mock"`)
//! 2. Key detection — first non-empty key wins
//! 3. Fallback: `MockProvider` (exits with code 1 in production)
//!
//! # Dyn-compatibility
//!
//! Async methods in traits cannot be dispatched through `dyn Trait` directly.
//! We return `BoxFuture<'_, T>` (a type alias for `Pin<Box<dyn Future<Output=T> + Send + '_>>`)
//! so the trait is dyn-compatible and can be stored as `Arc<dyn PaymentProvider>`.

use anyhow::Result;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::{future::Future, pin::Pin};

/// Convenience alias for heap-allocated, `Send` futures returned from trait methods.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// HMAC-SHA256 type alias.
pub type HmacSha256 = Hmac<Sha256>;

/// Common interface for every payment backend.
///
/// Methods return `BoxFuture` so the trait is dyn-compatible and can be stored
/// as `Arc<dyn PaymentProvider>` without the `async-trait` proc-macro.
pub trait PaymentProvider: Send + Sync {
    /// Create a hosted checkout session and return its redirect URL.
    fn create_checkout<'a>(
        &'a self,
        payload: serde_json::Value,
    ) -> BoxFuture<'a, Result<String>>;

    /// Verify a provider webhook signature.
    ///
    /// Returns `Ok(true)` when the signature is valid, `Ok(false)` when it
    /// does not match (caller should respond 401), and `Err(_)` only for
    /// hard failures (HMAC init, etc.).
    fn verify_webhook<'a>(
        &'a self,
        raw_body: &'a [u8],
        signature: &'a str,
    ) -> BoxFuture<'a, Result<bool>>;

    /// Short identifier used in health-check and logging output.
    fn name(&self) -> &'static str;
}

// ── Creem ────────────────────────────────────────────────────────────────────

/// [Creem](https://creem.io) payment provider.
pub struct CreemProvider {
    api_key: String,
    webhook_secret: String,
}

impl CreemProvider {
    pub fn new(api_key: String, webhook_secret: String) -> Self {
        Self {
            api_key,
            webhook_secret,
        }
    }
}

impl PaymentProvider for CreemProvider {
    fn create_checkout<'a>(
        &'a self,
        payload: serde_json::Value,
    ) -> BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            let resp = reqwest::Client::new()
                .post("https://api.creem.io/v1/checkouts")
                .header("x-api-key", &self.api_key)
                .json(&payload)
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!("Creem API error {}: {}", status, body));
            }

            let json: serde_json::Value = resp.json().await?;
            let url = json["url"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("No url in Creem response"))?
                .to_string();
            Ok(url)
        })
    }

    fn verify_webhook<'a>(
        &'a self,
        raw_body: &'a [u8],
        signature: &'a str,
    ) -> BoxFuture<'a, Result<bool>> {
        Box::pin(async move {
            if self.webhook_secret.is_empty() {
                // Dev mode: skip verification.
                return Ok(true);
            }
            let mut mac = HmacSha256::new_from_slice(self.webhook_secret.as_bytes())
                .map_err(|e| anyhow::anyhow!("HMAC init error: {}", e))?;
            mac.update(raw_body);
            let expected = hex::encode(mac.finalize().into_bytes());
            Ok(constant_time_eq(&expected, signature))
        })
    }

    fn name(&self) -> &'static str {
        "creem"
    }
}

// ── Dodo ─────────────────────────────────────────────────────────────────────

/// [Dodo Payments](https://dodopayments.com) provider stub.
pub struct DodoProvider {
    api_key: String,
}

impl DodoProvider {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

impl PaymentProvider for DodoProvider {
    fn create_checkout<'a>(
        &'a self,
        _payload: serde_json::Value,
    ) -> BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            // TODO: map to Dodo Payments API (https://docs.dodopayments.com)
            // Expected: POST https://api.dodopayments.com/... with self.api_key
            let _ = &self.api_key; // prevent dead_code lint until implemented
            Err(anyhow::anyhow!(
                "DodoProvider.create_checkout: not implemented"
            ))
        })
    }

    fn verify_webhook<'a>(
        &'a self,
        _raw_body: &'a [u8],
        _signature: &'a str,
    ) -> BoxFuture<'a, Result<bool>> {
        Box::pin(async move {
            // TODO: Dodo webhook signature verification
            Ok(false)
        })
    }

    fn name(&self) -> &'static str {
        "dodo"
    }
}

// ── Mock ──────────────────────────────────────────────────────────────────────

/// In-process mock; trusts all webhooks and returns a hard-coded URL.
///
/// Never use in production — `resolve_provider` exits with code 1 if this
/// would be selected when `NODE_ENV=production`.
pub struct MockProvider;

impl PaymentProvider for MockProvider {
    fn create_checkout<'a>(
        &'a self,
        _payload: serde_json::Value,
    ) -> BoxFuture<'a, Result<String>> {
        Box::pin(async move {
            Ok("https://mock-checkout.example.com/pay/mock_session_123".to_string())
        })
    }

    fn verify_webhook<'a>(
        &'a self,
        _raw_body: &'a [u8],
        _signature: &'a str,
    ) -> BoxFuture<'a, Result<bool>> {
        Box::pin(async move { Ok(true) })
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}

// ── Resolution ────────────────────────────────────────────────────────────────

/// Build the correct [`PaymentProvider`] from the environment.
///
/// Resolution order:
/// 1. `PAYMENT_PROVIDER` env var
/// 2. Key detection (`CREEM_API_KEY` → creem, `DODO_API_KEY` → dodo)
/// 3. Fallback to `MockProvider`; hard-exits in production
pub fn resolve_provider() -> Box<dyn PaymentProvider> {
    let env = stack_config::get_env();

    // 1. Explicit selector.
    if let Some(ref selector) = env.payment_provider {
        match selector.as_str() {
            "creem" => {
                return Box::new(CreemProvider::new(
                    env.creem_api_key.clone().unwrap_or_default(),
                    env.creem_webhook_secret.clone().unwrap_or_default(),
                ));
            }
            "dodo" => {
                return Box::new(DodoProvider::new(
                    env.dodo_api_key.clone().unwrap_or_default(),
                ));
            }
            "mock" => return Box::new(MockProvider),
            other => tracing::warn!(
                "Unknown PAYMENT_PROVIDER={}, falling back to key detection",
                other
            ),
        }
    }

    // 2. Key detection.
    if let Some(ref key) = env.creem_api_key {
        if !key.is_empty() {
            return Box::new(CreemProvider::new(
                key.clone(),
                env.creem_webhook_secret.clone().unwrap_or_default(),
            ));
        }
    }
    if let Some(ref key) = env.dodo_api_key {
        if !key.is_empty() {
            return Box::new(DodoProvider::new(key.clone()));
        }
    }

    // 3. Fallback.
    if env.node_env == "production" {
        tracing::error!("FATAL: No payment provider configured in production");
        std::process::exit(1);
    }
    tracing::warn!("No payment provider configured — using MockProvider");
    Box::new(MockProvider)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Constant-time string comparison to prevent timing attacks on HMAC signatures.
///
/// Returns `false` immediately if lengths differ (length is not secret for
/// hex-encoded HMAC outputs, which are always fixed-width).
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches() {
        assert!(constant_time_eq("abcdef", "abcdef"));
    }

    #[test]
    fn constant_time_eq_mismatch() {
        assert!(!constant_time_eq("abcdef", "abcdeg"));
    }

    #[test]
    fn constant_time_eq_different_lengths() {
        assert!(!constant_time_eq("abc", "abcd"));
    }

    #[tokio::test]
    async fn mock_provider_checkout() {
        let p = MockProvider;
        let url = p
            .create_checkout(serde_json::json!({}))
            .await
            .expect("mock checkout should not fail");
        assert!(url.starts_with("https://"));
    }

    #[tokio::test]
    async fn mock_provider_webhook_always_ok() {
        let p = MockProvider;
        let ok = p.verify_webhook(b"body", "sig").await.unwrap();
        assert!(ok);
    }

    #[tokio::test]
    async fn creem_webhook_dev_mode_skips_verification() {
        // Empty secret = dev mode → always Ok(true)
        let p = CreemProvider::new("key".to_string(), "".to_string());
        let ok = p.verify_webhook(b"body", "wrong_sig").await.unwrap();
        assert!(ok);
    }

    #[tokio::test]
    async fn creem_webhook_signature_mismatch() {
        let p = CreemProvider::new("key".to_string(), "secret".to_string());
        let ok = p.verify_webhook(b"body", "badsig").await.unwrap();
        assert!(!ok);
    }

    #[tokio::test]
    async fn creem_webhook_valid_signature() {
        use hmac::Mac;
        let secret = "test_secret";
        let body = b"hello webhook";
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let sig = hex::encode(mac.finalize().into_bytes());

        let p = CreemProvider::new("key".to_string(), secret.to_string());
        let ok = p.verify_webhook(body, &sig).await.unwrap();
        assert!(ok);
    }
}
