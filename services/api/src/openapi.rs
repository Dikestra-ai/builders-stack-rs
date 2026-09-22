use axum::{
    http::header,
    response::{Html, IntoResponse},
    Json,
};
use utoipa::OpenApi;

use stack_api_types::{
    DeleteResponse, ErrorResponse, ExportResponse, HealthResponse, MeResponse, NewPost, PatchPost,
    Post, SignInRequest, SignUpRequest,
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "@stack/api",
        version = "1.0.0",
        description = "Builders-stack Rust API. Axum + SQLx. Auth via session cookie. CRUD /posts. GDPR at /me/export and /me/delete."
    ),
    components(
        schemas(Post, NewPost, PatchPost, ErrorResponse, HealthResponse, MeResponse, ExportResponse, DeleteResponse, SignUpRequest, SignInRequest)
    ),
    tags(
        (name = "meta", description = "Liveness, OpenAPI spec, Swagger UI"),
        (name = "auth", description = "Session management"),
        (name = "gdpr", description = "GDPR data rights"),
        (name = "posts", description = "Example CRUD resource"),
    )
)]
pub struct ApiDoc;

pub async fn openapi_json() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json")],
        Json(ApiDoc::openapi()),
    )
}

/// Serves the Swagger UI at GET /docs.
/// Loads Swagger UI from unpkg CDN — no bundled binary needed.
pub async fn swagger_ui() -> impl IntoResponse {
    Html(
        r#"<!DOCTYPE html>
<html>
<head>
  <title>@stack/api — Swagger UI</title>
  <meta charset="utf-8"/>
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" >
</head>
<body>
<div id="swagger-ui"></div>
<script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"> </script>
<script>
window.onload = function() {
  SwaggerUIBundle({
    url: "/openapi.json",
    dom_id: '#swagger-ui',
    presets: [SwaggerUIBundle.presets.apis, SwaggerUIBundle.SwaggerUIStandalonePreset],
    layout: "BaseLayout"
  });
};
</script>
</body>
</html>"#,
    )
}
