//! Aggregation support for time-series data.
//!
//! Provides the runtime types and helpers for the aggregate endpoint generated
//! by `#[crudcrate(aggregate(...))]`.
//!
//! Requires the `aggregation` feature flag.

use std::collections::{BTreeMap, HashMap};

/// Query parameters for the aggregate endpoint.
#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub struct AggregateParams {
    /// The time bucket interval in short form (e.g., "1h", "1d", "30s", "1w")
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

/// Check that a string matches the strict short interval format: digits followed by a known unit suffix.
///
/// Valid: `1h`, `30s`, `7d`, `1w`, `3M`, `500ms`, `100us`
/// Invalid: `1 hour`, `foo`, `1h; DROP TABLE`, empty string
fn is_valid_short_interval(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    let boundary = match s.find(|c: char| !c.is_ascii_digit()) {
        Some(b) if b > 0 => b,
        _ => return false,
    };
    let unit = &s[boundary..];
    matches!(unit, "us" | "ms" | "s" | "m" | "h" | "d" | "w" | "M")
}

/// Validates that the requested interval is in the allowed list.
///
/// Only accepts the short format (`1h`, `1d`, `30s`, etc.) to prevent injection.
/// Returns the matched interval string from the allowed list, or an `ApiError`.
///
/// # Errors
///
/// Returns `ApiError::bad_request` if the format is invalid or the interval is not allowed.
pub fn validate_interval<'a>(
    requested: &str,
    allowed: &'a [&str],
) -> Result<&'a str, crate::ApiError> {
    if !is_valid_short_interval(requested) {
        return Err(crate::ApiError::bad_request(format!(
            "Invalid interval format '{}'. Use short form: 1h, 1d, 30s, 1w, etc.",
            requested
        )));
    }

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

/// Configuration for pivoting flat rows into columnar format.
/// Generated by the proc macro, consumed by pivot_to_columnar().
#[derive(Debug, Clone)]
pub struct PivotConfig {
    pub metrics: Vec<String>,
    pub aggregates: Vec<String>,
    pub group_by: Vec<String>,
    pub resolution: String,
}

/// Pivoted aggregate response — shared time axis with per-group metric arrays.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct AggregateResponse {
    pub resolution: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    pub times: Vec<String>,
    pub groups: Vec<AggregateGroup>,
}

/// One group in a pivoted aggregate response.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct AggregateGroup {
    /// Group-by column values (e.g., {"parameter_id": "uuid-1"})
    #[serde(flatten)]
    pub key: HashMap<String, serde_json::Value>,
    /// Per-metric aggregate arrays, each aligned with times
    pub metrics: HashMap<String, HashMap<String, Vec<Option<f64>>>>,
    /// Row count per bucket, aligned with times
    pub count: Vec<Option<i64>>,
}

/// Pivot flat aggregate rows into columnar format with a shared time axis.
///
/// Each input row is expected to have:
/// - `"bucket"`: the time bucket string
/// - Group-by column values (from `config.group_by`)
/// - `"{agg}_{metric}"` columns for each aggregate×metric combination
/// - `"count"`: the row count
///
/// Returns an `AggregateResponse` with aligned time axis and per-group arrays.
pub fn pivot_to_columnar(
    rows: &[serde_json::Value],
    config: &PivotConfig,
    start: Option<&str>,
    end: Option<&str>,
) -> AggregateResponse {
    use std::collections::BTreeSet;

    // 1. Build shared time axis from all unique bucket values
    let mut time_set = BTreeSet::new();
    for row in rows {
        if let Some(bucket) = row.get("bucket").and_then(|v| v.as_str()) {
            time_set.insert(bucket.to_string());
        }
    }
    let times: Vec<String> = time_set.into_iter().collect();
    let time_index: HashMap<&str, usize> = times
        .iter()
        .enumerate()
        .map(|(i, t)| (t.as_str(), i))
        .collect();

    // 2. Group rows by group_by column values
    let mut grouped: BTreeMap<String, Vec<&serde_json::Value>> = BTreeMap::new();
    for row in rows {
        let group_key = if config.group_by.is_empty() {
            String::new()
        } else {
            // Build a deterministic key from group_by column values
            config
                .group_by
                .iter()
                .map(|col| {
                    row.get(col)
                        .map(|v| v.to_string())
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join("|")
        };
        grouped.entry(group_key).or_default().push(row);
    }

    // 3. Build groups with aligned arrays
    let n = times.len();
    let groups: Vec<AggregateGroup> = grouped
        .into_iter()
        .map(|(_key_str, group_rows)| {
            // Extract group key from first row
            let mut key = HashMap::new();
            if let Some(first_row) = group_rows.first() {
                for col in &config.group_by {
                    if let Some(val) = first_row.get(col) {
                        key.insert(col.clone(), val.clone());
                    }
                }
            }

            // Pre-allocate metric arrays
            let mut metrics: HashMap<String, HashMap<String, Vec<Option<f64>>>> = HashMap::new();
            for metric in &config.metrics {
                let mut agg_map = HashMap::new();
                for agg in &config.aggregates {
                    agg_map.insert(agg.clone(), vec![None; n]);
                }
                metrics.insert(metric.clone(), agg_map);
            }
            let mut count = vec![None; n];

            // Fill arrays from rows
            for row in &group_rows {
                let Some(bucket) = row.get("bucket").and_then(|v| v.as_str()) else {
                    continue;
                };
                let Some(&idx) = time_index.get(bucket) else {
                    continue;
                };

                // Fill metric values
                for metric in &config.metrics {
                    for agg in &config.aggregates {
                        let col_name = format!("{agg}_{metric}");
                        if let Some(val) = row.get(&col_name) {
                            let f = val.as_f64();
                            if let Some(agg_map) = metrics.get_mut(metric) {
                                if let Some(arr) = agg_map.get_mut(agg) {
                                    arr[idx] = f;
                                }
                            }
                        }
                    }
                }

                // Fill count
                if let Some(c) = row.get("count") {
                    count[idx] = c.as_i64().or_else(|| c.as_f64().map(|f| f as i64));
                }
            }

            AggregateGroup {
                key,
                metrics,
                count,
            }
        })
        .collect();

    AggregateResponse {
        resolution: config.resolution.clone(),
        start: start.map(String::from),
        end: end.map(String::from),
        times,
        groups,
    }
}
