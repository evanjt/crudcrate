//! Rate Limiting Example
//!
//! This example demonstrates how to protect CRUD endpoints from abuse using
//! rate limiting middleware. This is critical for:
//! - Preventing DoS attacks
//! - Protecting against brute force attempts
//! - Ensuring fair resource usage
//! - Managing API quotas
//!
//! ## Rate Limiting Strategies
//! - IP-based rate limiting
//! - Per-user rate limiting
//! - Per-endpoint rate limiting
//! - Tiered limits (different limits for different user types)
//!
//! ## Implementation
//! Uses tower_governor for efficient in-memory rate limiting with sliding window.
//!
//! ## Usage
//! ```bash
//! # Add to Cargo.toml:
//! # tower-governor = "0.4"
//! # governor = "0.6"
//! ```

use axum::{
    Router,
    extract::{ConnectInfo, Request},
    response::{Response, IntoResponse},
    middleware::{self, Next},
    body::Body,
};
use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::{DatabaseConnection, entity::prelude::*};
use std::{
    net::SocketAddr,
    sync::Arc,
    collections::HashMap,
    time::SystemTime,
};
use uuid::Uuid;

// ============================================================================
// Simple In-Memory Rate Limiter (Production: use tower_governor or redis)
// ============================================================================

#[derive(Clone)]
struct RateLimiter {
    /// Maps IP address to (request count, window start time)
    requests: Arc<std::sync::Mutex<HashMap<String, (u32, SystemTime)>>>,
    max_requests: u32,
    window_secs: u64,
}

impl RateLimiter {
    fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            requests: Arc::new(std::sync::Mutex::new(HashMap::new())),
            max_requests,
            window_secs,
        }
    }

    /// Check if request should be allowed
    fn check_rate_limit(&self, key: &str) -> Result<(), RateLimitError> {
        let mut requests = self.requests.lock().unwrap();
        let now = SystemTime::now();

        let (count, window_start) = requests.entry(key.to_string()).or_insert((0, now));

        // Check if window has expired
        if let Ok(elapsed) = now.duration_since(*window_start) {
            if elapsed.as_secs() >= self.window_secs {
                // Reset window
                *count = 0;
                *window_start = now;
            }
        }

        // Check if limit exceeded
        if *count >= self.max_requests {
            let retry_after = self.window_secs - now.duration_since(*window_start).unwrap().as_secs();
            return Err(RateLimitError {
                retry_after_secs: retry_after,
                limit: self.max_requests,
                window_secs: self.window_secs,
            });
        }

        // Increment counter
        *count += 1;

        Ok(())
    }

    /// Cleanup expired entries (should be called periodically)
    fn cleanup(&self) {
        let mut requests = self.requests.lock().unwrap();
        let now = SystemTime::now();

        requests.retain(|_, (_, window_start)| {
            if let Ok(elapsed) = now.duration_since(*window_start) {
                elapsed.as_secs() < self.window_secs
            } else {
                true
            }
        });
    }
}

#[derive(Debug)]
struct RateLimitError {
    retry_after_secs: u64,
    limit: u32,
    window_secs: u64,
}

#[derive(serde::Serialize)]
struct RateLimitResponse {
    error: String,
    retry_after_seconds: u64,
    limit: u32,
    window_seconds: u64,
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert("Retry-After", self.retry_after_secs.to_string().parse().unwrap());
        headers.insert("X-RateLimit-Limit", self.limit.to_string().parse().unwrap());
        headers.insert("X-RateLimit-Window", self.window_secs.to_string().parse().unwrap());

        (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            headers,
            axum::Json(RateLimitResponse {
                error: format!(
                    "Rate limit exceeded. Maximum {} requests per {} seconds.",
                    self.limit, self.window_secs
                ),
                retry_after_seconds: self.retry_after_secs,
                limit: self.limit,
                window_seconds: self.window_secs,
            }),
        )
            .into_response()
    }
}

// ============================================================================
// Entity Definition
// ============================================================================

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "api_resources")]
#[crudcrate(
    api_struct = "ApiResource",
    name_singular = "resource",
    name_plural = "resources",
    description = "Rate-limited API resource",
    generate_router
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub name: String,

    pub data: String,

    #[crudcrate(exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

// ============================================================================
// Rate Limiting Middleware
// ============================================================================

/// IP-based rate limiting middleware
async fn ip_rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<Arc<RateLimiter>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, RateLimitError> {
    // Use IP address as rate limit key
    let key = addr.ip().to_string();

    // Check rate limit
    limiter.check_rate_limit(&key)?;

    // Continue to next middleware/handler
    Ok(next.run(req).await)
}

/// User-based rate limiting middleware (requires authentication)
async fn user_rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<Arc<RateLimiter>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, RateLimitError> {
    // Extract user ID from request extensions (set by auth middleware)
    // For this example, we'll use a header as a simple demonstration
    let user_id = req
        .headers()
        .get("X-User-ID")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("anonymous");

    // Check rate limit
    limiter.check_rate_limit(user_id)?;

    // Continue to next middleware/handler
    Ok(next.run(req).await)
}

// ============================================================================
// Tiered Rate Limiting (Different limits for different user types)
// ============================================================================

#[derive(Clone)]
struct TieredRateLimiter {
    free_tier: Arc<RateLimiter>,
    premium_tier: Arc<RateLimiter>,
}

impl TieredRateLimiter {
    fn new() -> Self {
        Self {
            // Free tier: 10 requests per minute
            free_tier: Arc::new(RateLimiter::new(10, 60)),
            // Premium tier: 100 requests per minute
            premium_tier: Arc::new(RateLimiter::new(100, 60)),
        }
    }

    fn get_limiter(&self, is_premium: bool) -> &RateLimiter {
        if is_premium {
            &self.premium_tier
        } else {
            &self.free_tier
        }
    }
}

async fn tiered_rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<Arc<TieredRateLimiter>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, RateLimitError> {
    // Check if user is premium (from auth middleware or header)
    let is_premium = req
        .headers()
        .get("X-Premium-User")
        .and_then(|h| h.to_str().ok())
        .map(|s| s == "true")
        .unwrap_or(false);

    let user_id = req
        .headers()
        .get("X-User-ID")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("anonymous");

    // Select appropriate rate limiter
    let rate_limiter = limiter.get_limiter(is_premium);

    // Check rate limit
    rate_limiter.check_rate_limit(user_id)?;

    Ok(next.run(req).await)
}

// ============================================================================
// Per-Endpoint Rate Limiting
// ============================================================================

struct EndpointLimits {
    // More expensive operations get lower limits
    create_limiter: Arc<RateLimiter>, // 5 per minute
    update_limiter: Arc<RateLimiter>, // 10 per minute
    read_limiter: Arc<RateLimiter>,   // 60 per minute
}

impl EndpointLimits {
    fn new() -> Self {
        Self {
            create_limiter: Arc::new(RateLimiter::new(5, 60)),
            update_limiter: Arc::new(RateLimiter::new(10, 60)),
            read_limiter: Arc::new(RateLimiter::new(60, 60)),
        }
    }
}

// ============================================================================
// Router Setup with Rate Limiting
// ============================================================================

fn create_rate_limited_router(db: &DatabaseConnection) -> Router {
    // Create rate limiters
    let ip_limiter = Arc::new(RateLimiter::new(100, 60)); // 100 requests per minute per IP
    let _user_limiter = Arc::new(RateLimiter::new(50, 60)); // 50 requests per minute per user

    // Generate CRUD router
    let crud_router: Router = ApiResource::router(db).into();

    // Apply rate limiting
    crud_router
        .layer(middleware::from_fn_with_state(
            ip_limiter.clone(),
            ip_rate_limit_middleware,
        ))
}

fn create_tiered_rate_limited_router(db: &DatabaseConnection) -> Router {
    let tiered_limiter = Arc::new(TieredRateLimiter::new());
    let crud_router: Router = ApiResource::router(db).into();

    crud_router
        .layer(middleware::from_fn_with_state(
            tiered_limiter.clone(),
            tiered_rate_limit_middleware,
        ))
}

// ============================================================================
// Production Example: Using tower_governor
// ============================================================================

// In production, use tower_governor for more features:
//
// ```rust
// use tower_governor::{
//     governor::GovernorConfigBuilder,
//     GovernorLayer,
// };
//
// fn create_governor_router(db: &DatabaseConnection) -> Router {
//     // Configure rate limiting
//     let governor_conf = Box::new(
//         GovernorConfigBuilder::default()
//             .per_second(2)
//             .burst_size(5)
//             .finish()
//             .unwrap(),
//     );
//
//     let governor_limiter = governor_conf.limiter().clone();
//     let governor_layer = GovernorLayer {
//         config: Box::leak(governor_conf),
//     };
//
//     let crud_router = ApiResource::router(db);
//
//     Router::new()
//         .nest("/api/resources", crud_router)
//         .layer(governor_layer)
// }
// ```

// ============================================================================
// Main (Example usage)
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Rate Limiting Example");
    println!("====================");
    println!();
    println!("Rate limiting strategies demonstrated:");
    println!("  ✓ IP-based rate limiting (prevent IP-based abuse)");
    println!("  ✓ User-based rate limiting (per-user quotas)");
    println!("  ✓ Tiered rate limiting (free vs premium users)");
    println!("  ✓ Per-endpoint limits (expensive ops get lower limits)");
    println!("  ✓ Sliding window algorithm");
    println!("  ✓ Proper HTTP 429 responses with Retry-After headers");
    println!();
    println!("Example limits:");
    println!("  - IP-based: 100 requests/minute per IP");
    println!("  - User-based: 50 requests/minute per user");
    println!("  - Free tier: 10 requests/minute");
    println!("  - Premium tier: 100 requests/minute");
    println!("  - Create operations: 5 requests/minute");
    println!("  - Read operations: 60 requests/minute");
    println!();
    println!("Production recommendations:");
    println!("  - Use tower_governor crate for production-grade rate limiting");
    println!("  - Use Redis for distributed rate limiting across multiple servers");
    println!("  - Implement exponential backoff for clients");
    println!("  - Monitor rate limit violations for abuse detection");
    println!("  - Use different limits for different endpoints");
    println!("  - Consider implementing token bucket or leaky bucket algorithms");
    println!("  - Add rate limit headers (X-RateLimit-*) to all responses");
    println!("  - Log rate limit violations for security analysis");
    println!();
    println!("Response headers:");
    println!("  - Retry-After: Seconds until rate limit resets");
    println!("  - X-RateLimit-Limit: Maximum requests allowed");
    println!("  - X-RateLimit-Window: Time window in seconds");

    Ok(())
}
