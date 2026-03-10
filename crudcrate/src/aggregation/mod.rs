//! Aggregation support for time-series data.
//!
//! Provides the runtime types and helpers for the aggregate endpoint generated
//! by `#[crudcrate(aggregate(...))]`.
//!
//! Requires the `aggregation` feature flag.

/// Query parameters for the aggregate endpoint.
#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub struct AggregateParams {
    /// The time bucket interval (e.g., "1 hour", "1h", "1 day", "1d")
    pub interval: String,
    /// Start time filter (ISO 8601 datetime, inclusive)
    pub start: Option<String>,
    /// End time filter (ISO 8601 datetime, exclusive)
    pub end: Option<String>,
    /// Filter expression for additional columns (same format as standard CRUD filter)
    pub filter: Option<String>,
    /// IANA timezone for timezone-aware bucketing (e.g., "US/Eastern", "Europe/Berlin")
    pub timezone: Option<String>,
}

/// Validates that the requested interval is in the allowed list.
///
/// Returns the matched interval string from the allowed list, or an `ApiError`.
///
/// # Errors
///
/// Returns `ApiError::bad_request` if the requested interval is not in the allowed list.
pub fn validate_interval<'a>(
    requested: &str,
    allowed: &'a [&str],
) -> Result<&'a str, crate::ApiError> {
    // Try exact match first
    if let Some(matched) = allowed.iter().find(|&&a| a == requested) {
        return Ok(matched);
    }

    // Try parsing both and comparing
    if let Ok(requested_interval) = sea_orm_timescale::types::Interval::parse(requested) {
        for &allowed_str in allowed {
            if let Ok(allowed_interval) = sea_orm_timescale::types::Interval::parse(allowed_str) {
                if requested_interval == allowed_interval {
                    return Ok(allowed_str);
                }
            }
        }
    }

    Err(crate::ApiError::bad_request(format!(
        "Invalid interval '{}'. Allowed intervals: {}",
        requested,
        allowed.join(", ")
    )))
}

/// Parse an ISO 8601 datetime string into a `chrono::DateTime<chrono::Utc>`.
///
/// Supports RFC 3339, naive datetime (`2024-01-01T00:00:00`), and date-only (`2024-01-01`).
///
/// # Errors
///
/// Returns `ApiError::bad_request` if the string cannot be parsed.
pub fn parse_datetime(s: &str) -> Result<chrono::DateTime<chrono::Utc>, crate::ApiError> {
    // Try RFC 3339 first (most common for API params)
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&chrono::Utc));
    }

    // Try parsing as NaiveDateTime (no timezone) and assume UTC
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(naive.and_utc());
    }

    // Try parsing as date only
    if let Ok(naive) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        && let Some(dt) = naive.and_hms_opt(0, 0, 0)
    {
        return Ok(dt.and_utc());
    }

    Err(crate::ApiError::bad_request(format!(
        "Invalid datetime '{s}'. Expected ISO 8601 format (e.g., 2024-01-01T00:00:00Z or 2024-01-01)"
    )))
}

/// Apply basic filters for aggregate queries without requiring `CRUDResource`.
///
/// Handles equality, UUID, numeric comparisons, booleans, arrays (IN), and nulls.
/// Does not support fulltext search, enum detection, or LIKE filtering.
pub fn apply_aggregate_filters(
    filter_str: Option<String>,
    columns: &[(&str, impl sea_orm::ColumnTrait + Copy)],
    _backend: sea_orm::DatabaseBackend,
) -> sea_orm::Condition {
    use sea_orm::{Condition, sea_query::Expr};

    let filters: std::collections::HashMap<String, serde_json::Value> =
        match filter_str.and_then(|s| serde_json::from_str(&s).ok()) {
            Some(parsed) => parsed,
            None => return Condition::all(),
        };

    let mut condition = Condition::all();

    for (key, value) in &filters {
        let Some((_, col)) = columns.iter().find(|(name, _)| *name == key.as_str()) else {
            continue;
        };

        let filter_expr = match value {
            serde_json::Value::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else if let Ok(uuid_val) = uuid::Uuid::parse_str(trimmed) {
                    Some(Expr::col(*col).eq(uuid_val))
                } else {
                    Some(
                        sea_orm::sea_query::SimpleExpr::FunctionCall(
                            sea_orm::sea_query::Func::upper(Expr::col(*col)),
                        )
                        .eq(trimmed.to_uppercase()),
                    )
                }
            }
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Some(Expr::col(*col).eq(i))
                } else {
                    n.as_f64().map(|f| Expr::col(*col).eq(f))
                }
            }
            serde_json::Value::Bool(b) => Some(Expr::col(*col).eq(*b)),
            serde_json::Value::Array(arr) => {
                let values: Vec<String> = arr
                    .iter()
                    .filter_map(|v| match v {
                        serde_json::Value::String(s) => Some(s.clone()),
                        serde_json::Value::Number(n) => Some(n.to_string()),
                        serde_json::Value::Bool(b) => Some(b.to_string()),
                        _ => None,
                    })
                    .collect();
                if values.is_empty() {
                    None
                } else {
                    Some(Expr::col(*col).is_in(values))
                }
            }
            serde_json::Value::Null => Some(Expr::col(*col).is_null()),
            serde_json::Value::Object(_) => None,
        };

        if let Some(expr) = filter_expr {
            condition = condition.add(expr);
        }
    }

    condition
}
