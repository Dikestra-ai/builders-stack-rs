//! stack-observability — structured logging to stdout (tracing) and Better Stack (Logtail).
//!
//! Design rules:
//! - Fire-and-forget: never block the caller.
//! - Never panics: logging failures must not crash the application.
//! - Env-gated: no `BETTERSTACK_SOURCE_TOKEN` → stdout only (silent no-op for remote drain).

use std::collections::BTreeMap;

/// Initialize tracing with an `EnvFilter` (reads `RUST_LOG`) and a fmt layer to stdout.
/// Call this once at service startup before any logging occurs.
pub fn init_tracing() {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let _ = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(fmt::layer())
        .try_init(); // try_init so a double-call in tests does not panic
}

/// Log a structured event to stdout via tracing and — when `BETTERSTACK_SOURCE_TOKEN` is set —
/// fire-and-forget POST the event to Better Stack.
///
/// `level` should be `"info"`, `"warn"`, or `"error"`.
/// `meta` is an optional JSON object whose fields are merged into the log payload.
pub fn log(level: &str, message: &str, meta: Option<serde_json::Value>) {
    // 1. Always emit to stdout via tracing.
    match level {
        "warn" => tracing::warn!(message = %message, meta = ?meta),
        "error" => tracing::error!(message = %message, meta = ?meta),
        _ => tracing::info!(message = %message, meta = ?meta),
    }

    // 2. Optionally ship to Better Stack (fire-and-forget).
    let token = match std::env::var("BETTERSTACK_SOURCE_TOKEN") {
        Ok(t) if !t.is_empty() => t,
        _ => return, // env-gated: no token → stdout only
    };

    let host = std::env::var("BETTERSTACK_INGEST_HOST")
        .unwrap_or_else(|_| "https://in.logs.betterstack.com".to_string());

    // Build the JSON payload.
    let mut payload: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    payload.insert(
        "dt".to_string(),
        serde_json::Value::String(chrono_or_rfc3339()),
    );
    payload.insert(
        "level".to_string(),
        serde_json::Value::String(level.to_string()),
    );
    payload.insert(
        "message".to_string(),
        serde_json::Value::String(message.to_string()),
    );

    // Merge meta fields at the top level (matches TypeScript `{ ...meta }` spread).
    if let Some(serde_json::Value::Object(map)) = meta {
        for (k, v) in map {
            payload.entry(k).or_insert(v);
        }
    }

    let body = match serde_json::to_string(&payload) {
        Ok(b) => b,
        Err(_) => return, // serialisation failure must not crash
    };

    // Spawn a fire-and-forget task; errors are silently discarded.
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let _ = client
            .post(&host)
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {}", token))
            .body(body)
            .send()
            .await;
        // Any error (network, auth, server) is intentionally ignored.
    });
}

/// Report a Rust error as a structured `"error"` log event.
///
/// The error's `Display` representation becomes the message; `context` is merged
/// into the meta object alongside it.
pub fn report_error(
    err: &(dyn std::error::Error + Send + Sync),
    context: Option<serde_json::Value>,
) {
    let message = err.to_string();

    // Merge context with an `error` key carrying the debug representation.
    let meta = match context {
        Some(serde_json::Value::Object(mut map)) => {
            map.entry("error".to_string())
                .or_insert_with(|| serde_json::Value::String(format!("{:?}", err)));
            Some(serde_json::Value::Object(map))
        }
        Some(other) => {
            // Context was not an object — wrap it under a "context" key.
            let mut map = serde_json::Map::new();
            map.insert("context".to_string(), other);
            map.insert(
                "error".to_string(),
                serde_json::Value::String(format!("{:?}", err)),
            );
            Some(serde_json::Value::Object(map))
        }
        None => {
            let mut map = serde_json::Map::new();
            map.insert(
                "error".to_string(),
                serde_json::Value::String(format!("{:?}", err)),
            );
            Some(serde_json::Value::Object(map))
        }
    };

    log("error", &message, meta);
}

/// Return a best-effort RFC-3339 / ISO-8601 UTC timestamp without pulling in `chrono`.
/// Uses `std::time::SystemTime`; produces `"YYYY-MM-DDTHH:MM:SS.mmmZ"`.
fn chrono_or_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Decompose Unix timestamp into calendar components (no external dep).
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;

    // Rata Die → Gregorian (algorithm by Howard Hinnant, public domain).
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, mo, d, h, m, s
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_format() {
        let ts = chrono_or_rfc3339();
        // Should look like 2026-09-22T...
        assert!(ts.contains('T'), "timestamp should contain T: {}", ts);
        assert!(ts.ends_with('Z'), "timestamp should end with Z: {}", ts);
        assert_eq!(ts.len(), 20, "timestamp length unexpected: {}", ts);
    }

    #[test]
    fn test_log_no_token_does_not_panic() {
        // Remove token so we exercise the stdout-only path.
        std::env::remove_var("BETTERSTACK_SOURCE_TOKEN");
        log("info", "hello from test", None);
        log("warn", "warn test", Some(serde_json::json!({"key": "value"})));
        log("error", "error test", None);
    }

    #[test]
    fn test_report_error_no_token_does_not_panic() {
        std::env::remove_var("BETTERSTACK_SOURCE_TOKEN");
        let err: Box<dyn std::error::Error + Send + Sync> =
            "something went wrong".into();
        report_error(err.as_ref(), None);
        report_error(
            err.as_ref(),
            Some(serde_json::json!({"request_id": "abc123"})),
        );
    }
}
