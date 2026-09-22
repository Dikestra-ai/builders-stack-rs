//! stack-api-types — Rust mirror of @stack/api-types.
//!
//! All structs use `#[serde(rename_all = "camelCase")]` so that wire JSON
//! matches the TypeScript contract exactly (camelCase keys).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ---------------------------------------------------------------------------
// Posts
// ---------------------------------------------------------------------------

/// A fully-hydrated post returned from the API.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Post {
    pub id: String,
    pub title: String,
    pub body: String,
    pub author_id: String,
    /// ISO-8601 datetime string (e.g. "2026-07-01T12:00:00.000Z")
    pub created_at: String,
    /// ISO-8601 datetime string
    pub updated_at: String,
}

/// Request body for creating a new post.
///
/// `authorId` is intentionally absent — it is derived server-side from the
/// authenticated session to prevent BOLA/broken object-level authorisation.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewPost {
    /// Post title (1–255 characters).
    pub title: String,
    /// Post body (1–100 000 characters).
    pub body: String,
}

impl NewPost {
    /// Validate field lengths, mirroring the Zod `.min()` / `.max()` rules.
    pub fn validate(&self) -> Result<(), String> {
        let title_len = self.title.len();
        if title_len < 1 {
            return Err("title must be at least 1 character".to_string());
        }
        if title_len > 255 {
            return Err(format!(
                "title must be at most 255 characters, got {}",
                title_len
            ));
        }

        let body_len = self.body.len();
        if body_len < 1 {
            return Err("body must be at least 1 character".to_string());
        }
        if body_len > 100_000 {
            return Err(format!(
                "body must be at most 100 000 characters, got {}",
                body_len
            ));
        }

        Ok(())
    }
}

/// Request body for partially updating a post (PATCH semantics).
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct PatchPost {
    pub title: Option<String>,
    pub body: Option<String>,
}

// ---------------------------------------------------------------------------
// Common responses
// ---------------------------------------------------------------------------

/// Generic error envelope.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
}

/// Response returned by the health-check endpoint.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    /// Process uptime in seconds.
    pub uptime: f64,
}

// ---------------------------------------------------------------------------
// User / auth
// ---------------------------------------------------------------------------

/// The currently authenticated user, as returned by `GET /me`.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MeResponse {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
}

/// Response for a data-export request.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportResponse {
    /// ISO-8601 datetime at which the export was generated.
    pub exported_at: Option<String>,
    pub user: Option<MeResponse>,
}

/// Response for a delete-account (or similar) request.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteResponse {
    pub deleted: Option<bool>,
    pub user_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

/// Request body for creating a new account.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct SignUpRequest {
    pub email: String,
    pub password: String,
    pub name: Option<String>,
}

/// Request body for signing in to an existing account.
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct SignInRequest {
    pub email: String,
    pub password: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_post_validate_ok() {
        let p = NewPost {
            title: "Hello".to_string(),
            body: "World".to_string(),
        };
        assert!(p.validate().is_ok());
    }

    #[test]
    fn new_post_validate_empty_title() {
        let p = NewPost {
            title: String::new(),
            body: "World".to_string(),
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn new_post_validate_title_too_long() {
        let p = NewPost {
            title: "x".repeat(256),
            body: "World".to_string(),
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn new_post_validate_body_too_long() {
        let p = NewPost {
            title: "Hi".to_string(),
            body: "x".repeat(100_001),
        };
        assert!(p.validate().is_err());
    }

    #[test]
    fn serde_roundtrip_post() {
        let post = Post {
            id: "p_abc123".to_string(),
            title: "Hello world".to_string(),
            body: "My first post.".to_string(),
            author_id: "u_abc123".to_string(),
            created_at: "2026-07-01T12:00:00.000Z".to_string(),
            updated_at: "2026-07-01T12:00:00.000Z".to_string(),
        };
        let json = serde_json::to_string(&post).unwrap();
        // Verify camelCase keys on the wire
        assert!(json.contains("\"authorId\""));
        assert!(json.contains("\"createdAt\""));
        assert!(json.contains("\"updatedAt\""));
        let back: Post = serde_json::from_str(&json).unwrap();
        assert_eq!(back.author_id, post.author_id);
    }

    #[test]
    fn serde_roundtrip_patch_post_partial() {
        let patch = PatchPost {
            title: Some("New title".to_string()),
            body: None,
        };
        let json = serde_json::to_string(&patch).unwrap();
        let back: PatchPost = serde_json::from_str(&json).unwrap();
        assert_eq!(back.title, Some("New title".to_string()));
        assert!(back.body.is_none());
    }
}
