# @stack/api for AI Agents

Builders-stack reference API. Hono + @hono/zod-openapi. Auth via Better Auth (/api/auth/*). CRUD /posts (reads public, writes session-scoped). GDPR data-rights at /me/export and /me/delete.

## Authentication

**API Key** (cookie: `better-auth.session_token`)
```bash
curl "<url>?better-auth.session_token=<api-key>"
```

## Quick Reference

```bash
# meta
GET   /health                                       # Liveness check
GET   /openapi.json                                 # OpenAPI 3.1 specification
GET   /docs                                         # Swagger UI

# auth
GET   /api/auth/{path}                              # Better Auth — GET handler
POST  /api/auth/{path}                              # Better Auth — POST handler
GET   /me                                           # Current user

# gdpr
GET   /me/export                                    # GDPR data export
POST  /me/delete                                    # GDPR erasure (right to be forgotten)

# posts
GET   /posts                                        # List posts
POST  /posts                                        # Create post
GET   /posts/{id}                                   # Get post
PATCH /posts/{id}                                   # Update post
DELETE/posts/{id}                                   # Delete post

```

## Endpoint Reference

### `GET /health`

Liveness check

**Responses:**

| Status | Description |
|--------|-------------|
| 200 | Service is up |

**Example:**
```bash
curl "https://api.example.com/health"
```

### `GET /openapi.json`

OpenAPI 3.1 specification

**Responses:**

| Status | Description |
|--------|-------------|
| 200 | OpenAPI document |

**Example:**
```bash
curl "https://api.example.com/openapi.json"
```

### `GET /docs`

Swagger UI

**Responses:**

| Status | Description |
|--------|-------------|
| 200 | HTML page |

**Example:**
```bash
curl "https://api.example.com/docs"
```

### `GET /api/auth/{path}`

Better Auth — GET handler

**Parameters:**

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| `path` | path | string | yes |  |

**Responses:**

| Status | Description |
|--------|-------------|
| 200 | Auth response (varies by sub-path) |

**Example:**
```bash
curl "https://api.example.com/api/auth/{path}"
```

### `POST /api/auth/{path}`

Better Auth — POST handler

**Parameters:**

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| `path` | path | string | yes |  |

**Request Body** (`application/json`)

**Responses:**

| Status | Description |
|--------|-------------|
| 200 | Auth response (varies by sub-path) |

**Example:**
```bash
curl -X POST "https://api.example.com/api/auth/{path}" \
  -H "Content-Type: application/json" \
  -d '{}'
```

### `GET /me`

Current user

**Responses:**

| Status | Description |
|--------|-------------|
| 200 | Authenticated user |
| 401 | Not signed in |

**Example:**
```bash
curl "https://api.example.com/me"
```

### `GET /me/export`

GDPR data export

**Responses:**

| Status | Description |
|--------|-------------|
| 200 | User data export |
| 401 | Not signed in |

**Example:**
```bash
curl "https://api.example.com/me/export"
```

### `POST /me/delete`

GDPR erasure (right to be forgotten)

**Responses:**

| Status | Description |
|--------|-------------|
| 200 | Deleted |
| 401 | Not signed in |

**Example:**
```bash
curl -X POST "https://api.example.com/me/delete" \
  -H "Content-Type: application/json" \
  -d '{}'
```

### `GET /posts`

List posts

**Responses:**

| Status | Description |
|--------|-------------|
| 200 | All posts |

**Example:**
```bash
curl "https://api.example.com/posts"
```

### `POST /posts`

Create post

**Request Body** (`application/json`, required)

**Responses:**

| Status | Description |
|--------|-------------|
| 201 | Created post |
| 401 | Unauthorized |

**Example:**
```bash
curl -X POST "https://api.example.com/posts" \
  -H "Content-Type: application/json" \
  -d '{}'
```

### `GET /posts/{id}`

Get post

**Parameters:**

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| `id` | path | string | yes |  |

**Responses:**

| Status | Description |
|--------|-------------|
| 200 | The post |
| 404 | Not found |

**Example:**
```bash
curl "https://api.example.com/posts/{id}"
```

### `PATCH /posts/{id}`

Update post

**Parameters:**

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| `id` | path | string | yes |  |

**Request Body** (`application/json`, required)

**Responses:**

| Status | Description |
|--------|-------------|
| 200 | Updated post |
| 401 | Unauthorized |
| 403 | Forbidden — not the author |
| 404 | Not found |

**Example:**
```bash
curl -X PATCH "https://api.example.com/posts/{id}" \
  -H "Content-Type: application/json" \
  -d '{}'
```

### `DELETE /posts/{id}`

Delete post

**Parameters:**

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| `id` | path | string | yes |  |

**Responses:**

| Status | Description |
|--------|-------------|
| 204 | Deleted |
| 401 | Unauthorized |
| 403 | Forbidden — not the author |
| 404 | Not found |

**Example:**
```bash
curl -X DELETE "https://api.example.com/posts/{id}"
```

## Data Models

### `Post`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes |  |
| `title` | string | yes |  |
| `body` | string | yes |  |
| `authorId` | string | yes |  |
| `createdAt` | string | yes |  |
| `updatedAt` | string | yes |  |

### `NewPost`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `title` | string | yes |  |
| `body` | string | yes |  |

### `PatchPost`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `title` | string | no |  |
| `body` | string | no |  |

### `Error`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `error` | string | yes |  |

### `HealthResponse`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `status` | string | yes |  |
| `service` | string | yes |  |
| `uptime` | number | yes |  |

### `MeResponse`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes |  |
| `email` | string | yes |  |
| `name` | string | no |  |

### `ExportResponse`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `exportedAt` | string | no |  |
| `user` | string | no |  |

### `DeleteResponse`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `deleted` | boolean | no |  |
| `userId` | string | no |  |

---
**Format**: OpenAPI 3.x | **Version**: 1.0.0
