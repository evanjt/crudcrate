# Security Best Practices

This page covers the security knobs CRUDCrate provides, and points at
upstream Axum / tower-http / axum-server for the layers it
intentionally does not provide (authentication, CORS, TLS, rate
limiting, response headers).

## Built-in Protections

### SQL injection prevention

All queries use Sea-ORM's parameterized expression builders, so user
input never reaches the SQL string:

```rust
let condition = Column::Email.eq(user_input);
// Renders as `WHERE email = $1` with `user_input` bound as a parameter.
```

The fulltext-search builders route the query value through
`Expr::cust_with_values`, so the `LIKE` pattern is bound rather than
interpolated.

### Pagination limits

```rust
const MAX_PAGE_SIZE: u64 = 1000;
const MAX_OFFSET: u64 = 1_000_000;
```

Override per resource with `#[crudcrate(max_page_size = 500)]`, or by
implementing `CRUDResource::max_page_size()` for runtime sources (env
vars, config).

### Overflow protection

Pagination math uses saturating arithmetic. `page=u64::MAX` and
`per_page=u64::MAX` resolve to safe values instead of panicking.

### Field value and search query length limits

```rust
const MAX_FIELD_VALUE_LENGTH: usize = 10_000;   // 10 KB per filter value
const MAX_SEARCH_QUERY_LENGTH: usize = 10_000;  // 10 KB for {"q": ...}
```

Oversized values are truncated before they reach the query builder.

### LIKE wildcard escaping

`%` and `_` in filter values are escaped, so `{"name": "%admin%"}`
matches the literal string rather than every row containing `admin`.

### Filter clause count limit

```rust
const MAX_FILTER_CLAUSES: usize = 100;
```

Requests with more than 100 filter keys return `400 Bad Request`.
Comparison-operator suffixes count separately:
`{"year_gte": 2020, "year_lte": 2024}` is two clauses on one field.

CRUDCrate deliberately does not silently drop over-limit filters: an
unfiltered response is a worse failure mode than a rejected request,
because a caller relying on the filter would see data it did not ask
for.

### Batch operation limits

```rust
#[crudcrate(batch_limit = 500)]   // default 100
pub struct Model { /* ... */ }
```

Override at runtime by implementing `CRUDResource::batch_limit()` (for
example, reading from an environment variable).

### Partial-success mode

`POST /resource/batch?partial=true` (and the equivalents for `PATCH`
and `DELETE`) returns `207 Multi-Status` with a `succeeded` / `failed`
split instead of failing the whole batch.

Each item is processed through the single-item hooks
(`create::one::*`, etc.) and commits independently. There is no shared
transaction. Error strings in the `failed` array use the sanitized
`ApiError` display output, not the raw DB error.

HTTP status codes:

- `200` if every item succeeded.
- `207 Multi-Status` if some succeeded and some failed.
- `400 Bad Request` if every item failed.

### Header injection prevention

Resource names embedded in `Content-Range` response headers are
filtered to ASCII non-control characters, so a malicious table-name
substring cannot inject extra headers.

The `ApiError` to `Response` translation also sanitizes the
user-derived prefix in `DbErr::RecordNotFound` messages: only an
alphanumeric/underscore identifier in the first word is kept, capped
at 64 characters. Anything else falls back to a generic `Resource`
prefix.

### Error sanitization

Internal DB errors are logged via `tracing` but the client only sees a
generic message:

```text
Internal log: SQLSTATE[42P01]: relation "users" does not exist
Client response: "A database error occurred"
```

## SecurityProfile

`SecurityProfile` bundles the runtime defaults that vary between
deployments: filter strictness, scope propagation, deleted-ID
exposure, and request body size limit. Three presets cover the common
cases:

| Preset                       | Strict filter parsing | Strict scope propagation | Expose deleted IDs | Body limit |
|------------------------------|-----------------------|--------------------------|--------------------|------------|
| `secure()` (0.9.0 default)   | yes                   | yes                      | no                 | 2 MiB      |
| `react_admin()`              | no                    | yes                      | yes                | 2 MiB      |
| `legacy()` (pre-0.9.0)       | no                    | no                       | yes                | 2 MiB      |

### Per-resource override

```rust
#[derive(EntityToModels, /* ... */)]
#[crudcrate(api_struct = "Customer", security_profile = "react_admin")]
pub struct Model { /* ... */ }
```

### Global override

```rust
use axum::Extension;
use crudcrate::SecurityProfile;

let app = Router::new()
    .merge(Customer::router(&db))
    .merge(Article::router(&db))
    .layer(Extension(SecurityProfile::secure()));
```

The `Extension` wins over the per-resource attribute, so a global layer
can tighten or loosen individual resources without touching each
`impl CRUDResource`.

### Custom profile

Use struct-update syntax to mix fields:

```rust
let p = SecurityProfile {
    expose_deleted_ids: true,
    ..SecurityProfile::secure()
};
```

### Resolution order

1. `axum::Extension<SecurityProfile>` on the request, if present.
2. `CRUDResource::security_profile()` for the resource being served
   (the derive attribute generates this).
3. The trait default, which is `SecurityProfile::secure()` in 0.9.0.

### Build-time caveat for `max_request_body_bytes`

`max_request_body_bytes` is applied via Axum's `DefaultBodyLimit`
layer when the router is built. Changing it via an `Extension` at
request time has no effect on the limit. The other three fields work
fully at request time.

To raise or lower the body limit, set it via the per-resource derive
attribute, or wrap the router yourself:

```rust
use axum::extract::DefaultBodyLimit;

let app = Router::new()
    .nest("/uploads", Upload::router(&db))
    .layer(DefaultBodyLimit::max(10 * 1024 * 1024));
```

See [MIGRATION_0.9.md](../../MIGRATION_0.9.md) for the upgrade path
from `legacy()` and the per-flag rationale.

## Authentication

**CRUDCrate does not authenticate callers.** The generated routers are
open: any request that reaches them is processed. Wrap them with an
Axum middleware layer (or an upstream reverse proxy) that
authenticates the caller before the handler runs.

The [`scoped_access`](https://github.com/evanjt/crudcrate/tree/main/examples/scoped_access)
example combines auth middleware with row-level scoping end to end.

For middleware patterns (JWT, API key, session cookie), see the
[Axum middleware docs](https://docs.rs/axum/latest/axum/middleware/index.html).

## Authorization

Authorization lives in the hook system. Use `before_get_all` for
row-level filtering and `before_*` hooks for per-operation checks:

```rust
async fn before_get_all(
    &self,
    _db: &DatabaseConnection,
    condition: &mut Condition,
) -> Result<(), ApiError> {
    let user = current_user();
    if !user.is_admin {
        *condition = condition.clone().add(Column::AuthorId.eq(user.id));
    }
    Ok(())
}

async fn before_update(
    &self,
    db: &DatabaseConnection,
    id: Uuid,
    _data: &mut ArticleUpdate,
) -> Result<(), ApiError> {
    let user = current_user();
    let article = Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(ApiError::NotFound)?;
    if article.author_id != user.id && !user.is_admin {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}
```

For declarative scope filtering tied to an `Extension`, see the
[scoping tutorial](../tutorial/scoping.md).

### Prevent mass assignment

Strip protected fields in `before_update`:

```rust
async fn before_update(
    &self,
    _db: &DatabaseConnection,
    _id: Uuid,
    data: &mut UserUpdate,
) -> Result<(), ApiError> {
    data.is_admin = None;
    data.password_reset_token = None;
    Ok(())
}
```

## Layers CRUDCrate does not provide

Use the appropriate Axum / tower-http / axum-server layer for each.
CRUDCrate intentionally does not wrap these.

- **Rate limiting**: [`tower-governor`](https://docs.rs/tower_governor),
  or a custom `tower::Layer`.
- **CORS**: [`tower_http::cors::CorsLayer`](https://docs.rs/tower-http/latest/tower_http/cors/index.html).
- **TLS termination**: terminate at a reverse proxy in production, or
  use [`axum-server`](https://docs.rs/axum-server) with `rustls` for
  in-process TLS.
- **Response security headers**:
  [`tower_http::set_header::SetResponseHeaderLayer`](https://docs.rs/tower-http/latest/tower_http/set_header/index.html)
  for `X-Content-Type-Options`, `X-Frame-Options`,
  `Strict-Transport-Security`, and friends.

## Logging

Log security-relevant events through `tracing`. The hook system is the
natural attachment point for `#[instrument]`:

```rust
#[instrument(skip(self, db))]
async fn before_delete(
    &self,
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<(), ApiError> {
    info!(article_id = %id, "delete attempted");
    Ok(())
}
```

A route-mount info log is emitted at startup with the resource name,
table, `batch_limit`, `max_page_size`, and the active security
defaults. It renders only when a `tracing_subscriber` is installed.

## Security checklist

- [ ] Auth middleware wraps every route that needs to be authenticated.
- [ ] `before_*` hooks enforce authorization on mutations.
- [ ] `SecurityProfile::secure()` (or stricter) is the active profile.
- [ ] Rate limiting layer is in place.
- [ ] CORS restricted to allowed origins.
- [ ] HTTPS terminated at the load balancer or in-process.
- [ ] Security response headers set.
- [ ] Sensitive fields excluded via `exclude(list)` or `exclude(one)`.
- [ ] DB credentials sourced from environment, not source.
- [ ] `cargo audit` runs in CI.
- [ ] Dependencies updated regularly.

## Next Steps

- [Performance Optimization](./performance.md)
- [Custom Operations](./custom-operations.md)
