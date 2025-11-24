# Security Best Practices

CRUDCrate includes built-in security features, but here's how to maximize protection.

## Built-in Protections

### SQL Injection Prevention

All queries use parameterization:

```rust
// User input is NEVER interpolated into SQL
let condition = Column::Email.eq(user_input);

// Generated SQL:
// SELECT * FROM users WHERE email = $1
// With parameter: user_input (escaped)
```

### Pagination Limits

Prevents denial-of-service via large queries:

```rust
// Built-in limits
const MAX_PAGE_SIZE: u64 = 1000;
const MAX_OFFSET: u64 = 1_000_000;

// Even if user requests 10000 items, only 1000 returned
```

### Overflow Protection

Pagination calculations use saturating arithmetic to prevent integer overflow panics:

```rust
// Malicious request: page=18446744073709551615&per_page=18446744073709551615
// Would normally cause: panic on integer overflow

// CRUDCrate uses saturating arithmetic:
let offset = (page.saturating_sub(1)).saturating_mul(safe_per_page);
// Result: Returns max safe values instead of panicking
```

### Field Value Length Limits

Protects against memory exhaustion from oversized filter values:

```rust
// Built-in limit
const MAX_FIELD_VALUE_LENGTH: usize = 10_000;  // 10KB max per field

// User provides: {"name": "A".repeat(1_000_000)}
// Result: Value truncated or rejected, not processed
```

### Fulltext Search Query Limits

Prevents oversized search queries from consuming resources:

```rust
// Built-in limit
const MAX_SEARCH_QUERY_LENGTH: usize = 10_000;  // 10KB max

// User provides: {"q": "A".repeat(1_000_000)}
// Result: Query truncated to 10KB before processing
```

### Header Injection Prevention

Sanitizes resource names to prevent HTTP header injection attacks:

```rust
// Malicious resource name: "items\r\nInjected-Header: evil"
// Could inject headers into Content-Range response

// CRUDCrate sanitizes:
fn sanitize_resource_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii() && !c.is_ascii_control())
        .collect()
}
// Result: Control characters stripped, header injection prevented
```

### LIKE Wildcard Escaping

Prevents wildcard injection in search queries:

```rust
// User provides: {"name": "%admin%"}
// Without escaping: matches ALL records containing "admin"

// CRUDCrate escapes wildcards:
// % → \%
// _ → \_
// Result: Literal search for "%admin%" string
```

### Batch Operation Limits

```rust
// Bulk delete limited to 100 items
async fn delete_many(ids: Vec<Uuid>) -> Result<u64, ApiError> {
    if ids.len() > 100 {
        return Err(ApiError::BadRequest("Cannot delete more than 100 items".into()));
    }
    // ...
}
```

### Error Sanitization

Internal errors are logged but not exposed:

```rust
// Internal: "SQLSTATE[42P01]: Undefined table: 7 ERROR: relation \"users\" does not exist"
// Response: "Database error"
```

## Authentication

CRUDCrate doesn't include authentication - use Axum middleware:

### JWT Authentication

```rust
use axum::{
    middleware,
    http::{Request, StatusCode},
    response::Response,
    extract::State,
};
use jsonwebtoken::{decode, Validation, DecodingKey};

#[derive(Clone)]
pub struct AuthState {
    pub jwt_secret: String,
}

async fn auth_middleware<B>(
    State(state): State<AuthState>,
    mut request: Request<B>,
    next: middleware::Next<B>,
) -> Result<Response, StatusCode> {
    let auth_header = request.headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &Validation::default()
    ).map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Add user to request extensions
    request.extensions_mut().insert(token_data.claims.user_id);

    Ok(next.run(request).await)
}

// Apply to routes
let app = Router::new()
    .merge(protected_router())
    .layer(middleware::from_fn_with_state(auth_state, auth_middleware));
```

### API Key Authentication

```rust
async fn api_key_middleware<B>(
    State(valid_keys): State<HashSet<String>>,
    request: Request<B>,
    next: middleware::Next<B>,
) -> Result<Response, StatusCode> {
    let api_key = request.headers()
        .get("X-API-Key")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !valid_keys.contains(api_key) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}
```

## Authorization

### Row-Level Security

Use `before_get_all` to filter results:

```rust
impl CRUDOperations for ArticleOperations {
    async fn before_get_all(
        &self,
        _db: &DatabaseConnection,
        condition: &mut Condition,
    ) -> Result<(), ApiError> {
        let user = get_current_user();

        if !user.is_admin {
            // Users only see their own articles
            *condition = condition.clone().add(Column::AuthorId.eq(user.id));
        }

        Ok(())
    }
}
```

### Operation-Level Authorization

```rust
impl CRUDOperations for ArticleOperations {
    async fn before_update(
        &self,
        db: &DatabaseConnection,
        id: Uuid,
        _data: &mut ArticleUpdate,
    ) -> Result<(), ApiError> {
        let user = get_current_user();
        let article = Entity::find_by_id(id).one(db).await?.ok_or(ApiError::NotFound)?;

        // Only author or admin can edit
        if article.author_id != user.id && !user.is_admin {
            return Err(ApiError::Forbidden);
        }

        Ok(())
    }

    async fn before_delete(
        &self,
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<(), ApiError> {
        let user = get_current_user();

        // Only admins can delete
        if !user.is_admin {
            return Err(ApiError::Forbidden);
        }

        Ok(())
    }
}
```

### Role-Based Access Control

```rust
#[derive(Clone, Copy, PartialEq)]
pub enum Role {
    User,
    Moderator,
    Admin,
}

fn require_role(user: &User, required: Role) -> Result<(), ApiError> {
    let user_level = match user.role {
        Role::Admin => 3,
        Role::Moderator => 2,
        Role::User => 1,
    };

    let required_level = match required {
        Role::Admin => 3,
        Role::Moderator => 2,
        Role::User => 1,
    };

    if user_level >= required_level {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}
```

## Input Validation

### Sanitize User Input

```rust
async fn before_create(
    &self,
    _db: &DatabaseConnection,
    data: &mut ArticleCreate,
) -> Result<(), ApiError> {
    // Trim whitespace
    data.title = data.title.trim().to_string();

    // Sanitize HTML (if allowing rich text)
    data.content = ammonia::clean(&data.content);

    // Validate length
    if data.title.len() > 200 {
        return Err(ApiError::ValidationFailed(vec![
            ValidationError::new("title", "Title too long (max 200 characters)")
        ]));
    }

    Ok(())
}
```

### Prevent Mass Assignment

Only allow specific fields to be updated:

```rust
async fn before_update(
    &self,
    _db: &DatabaseConnection,
    _id: Uuid,
    data: &mut UserUpdate,
) -> Result<(), ApiError> {
    // Prevent updating sensitive fields via API
    data.is_admin = None;
    data.email_verified = None;
    data.password_reset_token = None;

    Ok(())
}
```

## Rate Limiting

Use tower middleware:

```rust
use tower_governor::{GovernorLayer, GovernorConfig};

let governor_conf = GovernorConfig::default();

let app = Router::new()
    .merge(api_router())
    .layer(GovernorLayer {
        config: &governor_conf,
    });
```

Or custom rate limiting:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

struct RateLimiter {
    requests: HashMap<String, (u32, Instant)>,
    max_requests: u32,
    window: Duration,
}

async fn rate_limit_middleware<B>(
    State(limiter): State<Arc<Mutex<RateLimiter>>>,
    request: Request<B>,
    next: middleware::Next<B>,
) -> Result<Response, StatusCode> {
    let ip = request.headers()
        .get("X-Forwarded-For")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let mut limiter = limiter.lock().await;

    let (count, start) = limiter.requests
        .entry(ip.clone())
        .or_insert((0, Instant::now()));

    if start.elapsed() > limiter.window {
        *count = 0;
        *start = Instant::now();
    }

    *count += 1;

    if *count > limiter.max_requests {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    drop(limiter);
    Ok(next.run(request).await)
}
```

## CORS Configuration

```rust
use tower_http::cors::{CorsLayer, Any};

// Development (permissive)
let cors = CorsLayer::permissive();

// Production (restrictive)
let cors = CorsLayer::new()
    .allow_origin("https://yourdomain.com".parse::<HeaderValue>().unwrap())
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
    .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
    .max_age(Duration::from_secs(3600));

let app = Router::new()
    .merge(api_router())
    .layer(cors);
```

## HTTPS

Always use HTTPS in production:

```rust
// In production, terminate TLS at load balancer/reverse proxy
// Or use rustls for direct TLS:

use axum_server::tls_rustls::RustlsConfig;

let config = RustlsConfig::from_pem_file("cert.pem", "key.pem").await?;

axum_server::bind_rustls(addr, config)
    .serve(app.into_make_service())
    .await?;
```

## Security Headers

```rust
use tower_http::set_header::SetResponseHeaderLayer;
use axum::http::header;

let app = Router::new()
    .merge(api_router())
    .layer(SetResponseHeaderLayer::if_not_present(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff")
    ))
    .layer(SetResponseHeaderLayer::if_not_present(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("DENY")
    ))
    .layer(SetResponseHeaderLayer::if_not_present(
        header::STRICT_TRANSPORT_SECURITY,
        HeaderValue::from_static("max-age=31536000; includeSubDomains")
    ));
```

## Logging and Monitoring

```rust
use tracing::{info, warn, error, instrument};

impl CRUDOperations for ArticleOperations {
    #[instrument(skip(self, db))]
    async fn before_delete(
        &self,
        db: &DatabaseConnection,
        id: Uuid,
    ) -> Result<(), ApiError> {
        let user = get_current_user();

        info!(
            user_id = %user.id,
            article_id = %id,
            "User attempting to delete article"
        );

        // Authorization check
        let article = Entity::find_by_id(id).one(db).await?;

        if article.is_none() {
            warn!(article_id = %id, "Attempted to delete non-existent article");
            return Err(ApiError::NotFound);
        }

        if !user.is_admin {
            warn!(
                user_id = %user.id,
                article_id = %id,
                "Unauthorized delete attempt"
            );
            return Err(ApiError::Forbidden);
        }

        Ok(())
    }
}
```

## Security Checklist

- [ ] All routes require authentication (where needed)
- [ ] Authorization checks on all mutations
- [ ] Input validation on all user data
- [ ] Rate limiting configured
- [ ] CORS restricted to allowed origins
- [ ] HTTPS enabled in production
- [ ] Security headers set
- [ ] Sensitive fields excluded from responses
- [ ] Logging enabled for security events
- [ ] Database credentials not in code
- [ ] Dependencies updated regularly

## Next Steps

- Learn about [Performance Optimization](./performance.md)
- Configure [Multi-Database Support](./multi-database.md)
- Set up [Custom Operations](./custom-operations.md)
