use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime};
use rust_decimal::Decimal;
use sea_orm::{
    Condition, DatabaseBackend,
    sea_query::{ColumnType, Expr, ExprTrait, LikeExpr},
};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

#[cfg(test)]
use super::pagination::{MAX_OFFSET, MAX_PAGE_SIZE};
pub use super::pagination::{parse_pagination, parse_range};
use super::search::{build_fulltext_condition, build_like_condition, escape_like_wildcards};

// Basic safety limits
const MAX_FIELD_VALUE_LENGTH: usize = 10_000;
/// Maximum number of elements accepted in a single array-valued filter.
///
/// An array filter (`filter={"id":[...]}`) becomes one SQL `IN (...)` clause with
/// one bind parameter per element. `MAX_FILTER_CLAUSES` caps the number of keys, not
/// the length of any single array, so without this cap a single key could carry tens
/// of thousands of elements and blow past the backend bind-parameter ceiling (`SQLite`
/// 32766, Postgres/MySQL 65535). A 500 at the top, a query-planning `DoS` below it.
/// This bound is also reachable over GET, whose query string is not covered by the
/// request body-size limit. Exceeding it produces `400 Bad Request`, matching the
/// reject-don't-silently-drop policy of `MAX_FILTER_CLAUSES`.
const MAX_FILTER_ARRAY_LEN: usize = 1000;
/// Maximum number of filter clauses accepted per request.
///
/// A malicious client could otherwise submit a filter object with thousands of
/// keys, each producing a SQL condition and potentially blowing up query planning
/// or evaluation time. Legitimate admin dashboards rarely exceed ~20 filter fields
/// (remember comparison operators split one field into two clauses: `year_gte`
/// and `year_lte`), so 100 gives generous headroom while still preventing abuse.
///
/// Exceeding this limit produces a `400 Bad Request` response; crudcrate
/// deliberately does *not* silently drop filters, because a silently-unfiltered
/// response is worse than a failed request.
const MAX_FILTER_CLAUSES: usize = 100;

/// Basic field name validation
fn is_valid_field_name(field_name: &str) -> bool {
    // Strengthen validation to prevent injection attempts (defense-in-depth)
    // Note: Actual field names are validated against a whitelist, but this adds an extra layer
    !field_name.is_empty()
        && field_name.len() <= 100
        && field_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !field_name.starts_with('_')
        && !field_name.starts_with(|c: char| c.is_ascii_digit())
}

/// Basic value length check
const fn validate_field_value(value: &str) -> bool {
    value.len() <= MAX_FIELD_VALUE_LENGTH
}

/// Parse React Admin comparison operator suffixes
/// Returns (`base_field_name`, `sql_operator`) if a suffix is found
fn parse_comparison_operator(field_name: &str) -> Option<(&str, &str)> {
    field_name.strip_suffix("_gte").map_or_else(
        || {
            field_name.strip_suffix("_lte").map_or_else(
                || {
                    field_name.strip_suffix("_gt").map_or_else(
                        || {
                            field_name.strip_suffix("_lt").map_or_else(
                                || {
                                    field_name
                                        .strip_suffix("_neq")
                                        .map(|base_field| (base_field, "!="))
                                },
                                |base_field| Some((base_field, "<")),
                            )
                        },
                        |base_field| Some((base_field, ">")),
                    )
                },
                |base_field| Some((base_field, "<=")),
            )
        },
        |base_field| Some((base_field, ">=")),
    )
}

/// Apply a comparison operator against a real column with a typed value, binding a
/// native parameter (`"col" >= $1`) rather than wrapping the column in `UPPER(...)`.
fn apply_typed_comparison<V: Into<sea_orm::Value>>(
    column: impl sea_orm::ColumnTrait + Copy,
    operator: &str,
    value: V,
) -> Expr {
    let column = Expr::col(column);
    match operator {
        ">=" => column.gte(value),
        "<=" => column.lte(value),
        ">" => column.gt(value),
        "<" => column.lt(value),
        "!=" => column.ne(value),
        _ => column.eq(value), // fallback to equality
    }
}

/// Columns whose comparison filters bind a typed `sea_orm::Value` parsed from the
/// request string. Text/enum/json and other types keep case-insensitive string
/// comparison (see `process_string_filter`).
fn binds_typed_value(col_type: &ColumnType) -> bool {
    matches!(
        col_type,
        ColumnType::Date
            | ColumnType::Time
            | ColumnType::DateTime
            | ColumnType::Timestamp
            | ColumnType::TimestampWithTimeZone
            | ColumnType::Decimal(_)
            | ColumnType::Money(_)
            | ColumnType::Uuid
            | ColumnType::Float
            | ColumnType::Double
            | ColumnType::TinyInteger
            | ColumnType::SmallInteger
            | ColumnType::Integer
            | ColumnType::BigInteger
            | ColumnType::TinyUnsigned
            | ColumnType::SmallUnsigned
            | ColumnType::Unsigned
            | ColumnType::BigUnsigned
            | ColumnType::Boolean
    )
}

/// Parse a raw filter string into a `sea_orm::Value` matching the column's SQL type.
/// Returns `None` when the value can't be parsed to that type, so the caller drops
/// the clause rather than emitting a comparison the backend will reject.
fn typed_value_for_column(col_type: &ColumnType, raw: &str) -> Option<sea_orm::Value> {
    match col_type {
        ColumnType::Date => NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .ok()
            .map(sea_orm::Value::from),
        ColumnType::Time => NaiveTime::parse_from_str(raw, "%H:%M:%S")
            .ok()
            .map(sea_orm::Value::from),
        ColumnType::DateTime | ColumnType::Timestamp => {
            parse_naive_datetime(raw).map(sea_orm::Value::from)
        }
        ColumnType::TimestampWithTimeZone => DateTime::<FixedOffset>::parse_from_rfc3339(raw)
            .ok()
            .map(sea_orm::Value::from),
        ColumnType::Decimal(_) | ColumnType::Money(_) => {
            Decimal::from_str(raw).ok().map(sea_orm::Value::from)
        }
        ColumnType::Uuid => Uuid::parse_str(raw).ok().map(sea_orm::Value::from),
        ColumnType::Float => raw.parse::<f32>().ok().map(sea_orm::Value::from),
        ColumnType::Double => raw.parse::<f64>().ok().map(sea_orm::Value::from),
        ColumnType::TinyInteger
        | ColumnType::SmallInteger
        | ColumnType::Integer
        | ColumnType::BigInteger => raw.parse::<i64>().ok().map(sea_orm::Value::from),
        ColumnType::TinyUnsigned
        | ColumnType::SmallUnsigned
        | ColumnType::Unsigned
        | ColumnType::BigUnsigned => raw.parse::<u64>().ok().map(sea_orm::Value::from),
        ColumnType::Boolean => raw.parse::<bool>().ok().map(sea_orm::Value::from),
        _ => None,
    }
}

/// Parse every element of an array-valued filter into a `sea_orm::Value` matching the
/// column's SQL type.
///
/// # Errors
/// Returns `ApiError::BadRequest` when an element cannot be parsed to the column's
/// type. Dropping the clause instead would silently return unfiltered rows, which is
/// the same reject-don't-silently-drop policy as `MAX_FILTER_ARRAY_LEN`.
fn typed_array_values(
    col_type: &ColumnType,
    field: &str,
    array_values: &[serde_json::Value],
) -> Result<Vec<sea_orm::Value>, crate::errors::ApiError> {
    array_values
        .iter()
        .map(|element| {
            let raw = match element {
                serde_json::Value::String(s) => s.trim().to_string(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                // Null, nested arrays and objects have no scalar form, and the
                // empty string parses to no column type.
                _ => String::new(),
            };
            typed_value_for_column(col_type, &raw).ok_or_else(|| {
                // Echo only what the client sent; naming the column's SQL type here
                // would expose schema that joined filters deliberately keep hidden.
                crate::errors::ApiError::bad_request(format!(
                    "Filter value {element} is not valid for field `{field}`"
                ))
            })
        })
        .collect()
}

/// Accept a bare `YYYY-MM-DDTHH:MM:SS` / space-separated datetime, or a full RFC 3339
/// timestamp (normalised to naive UTC), for naive datetime columns.
fn parse_naive_datetime(raw: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S"))
        .ok()
        .or_else(|| {
            DateTime::parse_from_rfc3339(raw)
                .ok()
                .map(|dt| dt.naive_utc())
        })
}

fn parse_filter_json(
    filter_str: Option<String>,
) -> Result<HashMap<String, serde_json::Value>, crate::errors::ApiError> {
    let Some(filter) = filter_str else {
        return Ok(HashMap::new());
    };

    match serde_json::from_str::<HashMap<String, serde_json::Value>>(&filter) {
        Ok(parsed) => {
            if parsed.len() > MAX_FILTER_CLAUSES {
                tracing::debug!(
                    "Filter has {} clauses, exceeding MAX_FILTER_CLAUSES ({})",
                    parsed.len(),
                    MAX_FILTER_CLAUSES
                );
                return Err(crate::errors::ApiError::bad_request(format!(
                    "Filter contains too many clauses (max {MAX_FILTER_CLAUSES})"
                )));
            }
            // Cap array-valued filters here, at the shared chokepoint, so the bound
            // applies to every element type (UUID/int/string/bool) before it fans out
            // into an `IN (...)` list, and to joined `In` filters too, which also
            // flow through this parser.
            if let Some((key, len)) = parsed.iter().find_map(|(k, v)| match v {
                serde_json::Value::Array(a) if a.len() > MAX_FILTER_ARRAY_LEN => {
                    Some((k.clone(), a.len()))
                }
                _ => None,
            }) {
                tracing::debug!(
                    "Filter key '{key}' has {len} array elements, exceeding MAX_FILTER_ARRAY_LEN ({MAX_FILTER_ARRAY_LEN})"
                );
                return Err(crate::errors::ApiError::bad_request(format!(
                    "Filter array for '{key}' has too many elements (max {MAX_FILTER_ARRAY_LEN})"
                )));
            }
            Ok(parsed)
        }
        Err(_e) => {
            // Invalid JSON is a client error but we preserve historical behavior
            // (ignore and return empty) to avoid breaking callers that pass
            // malformed filters defensively. The MAX_FILTER_CLAUSES path is new
            // and deliberately strict.
            tracing::debug!("Invalid JSON in filter parameter - ignoring filter");
            Ok(HashMap::new())
        }
    }
}

fn handle_fulltext_search<T: crate::traits::CRUDResource>(
    filters: &HashMap<String, serde_json::Value>,
    searchable_columns: &[(&str, impl sea_orm::ColumnTrait)],
    backend: DatabaseBackend,
) -> Option<Condition> {
    if let Some(q_value) = filters.get("q")
        && let Some(q_value_str) = q_value.as_str()
    {
        // Trim and skip empty/whitespace-only queries
        let trimmed_q = q_value_str.trim();
        if trimmed_q.is_empty() {
            return None;
        }

        // Try fulltext search first
        if let Some(fulltext_expr) = build_fulltext_condition::<T>(trimmed_q, backend) {
            return Some(Condition::all().add(fulltext_expr));
        }

        // Fallback to original LIKE search on regular searchable columns
        // Escape LIKE wildcards to prevent wildcard injection
        let escaped_query = escape_like_wildcards(trimmed_q);

        let mut or_conditions = Condition::any();
        for (col_name, col) in searchable_columns {
            if T::is_enum_field(col_name) {
                or_conditions = or_conditions.add(
                    Expr::FunctionCall(sea_orm::sea_query::Func::upper(enum_text_expr(
                        *col, backend,
                    )))
                    .like(LikeExpr::new(format!("%{}%", escaped_query.to_uppercase())).escape('!')),
                );
            } else {
                let cast_type = match backend {
                    DatabaseBackend::MySql => "CHAR",
                    _ => "TEXT",
                };
                or_conditions = or_conditions.add(
                    Expr::FunctionCall(sea_orm::sea_query::Func::upper(Expr::cast_as(
                        Expr::col(*col),
                        cast_type,
                    )))
                    .like(LikeExpr::new(format!("%{}%", escaped_query.to_uppercase())).escape('!')),
                );
            }
        }
        return Some(or_conditions);
    }
    None
}

/// Column expression for comparing an enum column as text. `PostgreSQL` rejects
/// string operations on native ENUM columns, so it needs an explicit
/// `CAST(col AS TEXT)`; the other backends compare enum columns as strings
/// directly.
fn enum_text_expr(column: impl sea_orm::ColumnTrait + Copy, backend: DatabaseBackend) -> Expr {
    match backend {
        DatabaseBackend::Postgres => Expr::cast_as(Expr::col(column), "TEXT"),
        _ => Expr::col(column),
    }
}

/// Apply a string comparison using the given operator.
fn apply_string_comparison(
    column: impl sea_orm::ColumnTrait + Copy,
    operator: &str,
    trimmed_value: &str,
) -> Expr {
    let col_upper = Expr::FunctionCall(sea_orm::sea_query::Func::upper(Expr::col(column)));
    let val_upper = trimmed_value.to_uppercase();
    match operator {
        "!=" => col_upper.ne(val_upper),
        ">=" => col_upper.gte(val_upper),
        "<=" => col_upper.lte(val_upper),
        ">" => col_upper.gt(val_upper),
        "<" => col_upper.lt(val_upper),
        _ => col_upper.eq(val_upper),
    }
}

fn process_string_filter<T: crate::traits::CRUDResource>(
    base_field: &str,
    operator: &str,
    string_value: &str,
    column: impl sea_orm::ColumnTrait + Copy,
    backend: DatabaseBackend,
) -> Option<Expr> {
    if !validate_field_value(string_value) {
        return None;
    }

    let trimmed_value = string_value.trim();
    if trimmed_value.is_empty() {
        return None;
    }

    // Check if this field should use LIKE queries (only for equality, not comparison operators)
    if operator == "=" && T::like_filterable_columns().contains(&base_field) {
        return Some(build_like_condition(base_field, trimmed_value, backend));
    }

    if T::is_enum_field(base_field) {
        // Handle enum fields with case-insensitive matching
        let col_upper = Expr::FunctionCall(sea_orm::sea_query::Func::upper(enum_text_expr(
            column, backend,
        )));
        let val_upper = trimmed_value.to_uppercase();
        return Some(match operator {
            "!=" => col_upper.ne(val_upper),
            ">=" => col_upper.gte(val_upper),
            "<=" => col_upper.lte(val_upper),
            ">" => col_upper.gt(val_upper),
            "<" => col_upper.lt(val_upper),
            _ => col_upper.eq(val_upper),
        });
    }

    // Route by the column's SQL type. Non-text columns bind a typed value so the
    // backend compares natively; wrapping them in UPPER() errors on Postgres (dates,
    // numbers, uuids, ...) and orders lexically where it doesn't. Text/enum/json keep
    // case-insensitive comparison; an unparseable value drops the clause.
    let column_def = column.def();
    let col_type = column_def.get_column_type();
    if binds_typed_value(col_type) {
        return typed_value_for_column(col_type, trimmed_value)
            .map(|value| apply_typed_comparison(column, operator, value));
    }

    // Case-insensitive string comparison with operator
    Some(apply_string_comparison(column, operator, trimmed_value))
}

fn typed_array_in_list<C: sea_orm::ColumnTrait + Copy>(
    column: C,
    array_values: &[serde_json::Value],
) -> Option<Expr> {
    if array_values.is_empty() || array_values.len() > MAX_FILTER_ARRAY_LEN {
        return None;
    }
    if array_values.iter().all(serde_json::Value::is_i64) {
        let ints: Vec<i64> = array_values
            .iter()
            .filter_map(serde_json::Value::as_i64)
            .collect();
        return Some(Expr::col(column).is_in(ints));
    }
    if array_values.iter().all(serde_json::Value::is_number) {
        let nums: Vec<f64> = array_values
            .iter()
            .filter_map(serde_json::Value::as_f64)
            .collect();
        if nums.len() == array_values.len() {
            return Some(Expr::col(column).is_in(nums));
        }
    }
    if array_values.iter().all(serde_json::Value::is_boolean) {
        let bools: Vec<bool> = array_values
            .iter()
            .filter_map(serde_json::Value::as_bool)
            .collect();
        return Some(Expr::col(column).is_in(bools));
    }
    None
}

fn process_array_filter(
    array_values: &[serde_json::Value],
    field: &str,
    column: impl sea_orm::ColumnTrait + Copy,
    is_enum: bool,
    backend: DatabaseBackend,
) -> Result<Option<Expr>, crate::errors::ApiError> {
    if array_values.is_empty() || array_values.len() > MAX_FILTER_ARRAY_LEN {
        return Ok(None);
    }

    // Route by the column's SQL type, as the scalar path does: an IN list over a
    // date, timestamp, numeric or uuid column must bind typed values, not text.
    // Enum columns keep the casted/uppercased string path below.
    let column_def = column.def();
    let col_type = column_def.get_column_type();
    if !is_enum && binds_typed_value(col_type) {
        let values = typed_array_values(col_type, field, array_values)?;
        return Ok(Some(Expr::col(column).is_in(values)));
    }

    // Try to parse all values as UUIDs first
    let mut uuid_values = Vec::new();
    let mut all_uuids = true;
    for v in array_values {
        if let Some(s) = v.as_str()
            && let Ok(uuid_value) = Uuid::parse_str(s.trim())
        {
            uuid_values.push(uuid_value);
            continue;
        }
        all_uuids = false;
        break;
    }

    if all_uuids && !uuid_values.is_empty() {
        return Ok(Some(Expr::col(column).is_in(uuid_values)));
    }

    // Type-matched IN list (integers/floats/bools) so the bound values match the
    // column type on strict backends. Enum columns keep the string path below;
    // their casted/uppercased comparison needs text binds.
    if !is_enum && let Some(expr) = typed_array_in_list(column, array_values) {
        return Ok(Some(expr));
    }

    // Fall back to string-based IN for non-UUID values
    let in_values: Vec<String> = array_values
        .iter()
        .filter_map(|v| match v {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            serde_json::Value::Bool(b) => Some(b.to_string()),
            _ => None,
        })
        .collect();

    if !in_values.is_empty() {
        if is_enum {
            // Uppercase both sides so string bind parameters match native enums
            let col_upper = Expr::FunctionCall(sea_orm::sea_query::Func::upper(enum_text_expr(
                column, backend,
            )));
            let upper_values: Vec<String> = in_values.iter().map(|v| v.to_uppercase()).collect();
            return Ok(Some(col_upper.is_in(upper_values)));
        }
        return Ok(Some(Expr::col(column).is_in(in_values)));
    }
    Ok(None)
}

/// Comparison for value types where ordering operators apply; `Like`, `In` and
/// `IsNull` return `None`.
fn ordered_comparison<C, V>(
    column: C,
    operator: super::joined::FilterOperator,
    value: V,
) -> Option<Expr>
where
    C: sea_orm::ColumnTrait + Copy,
    V: Into<sea_orm::sea_query::Value>,
{
    use super::joined::FilterOperator;

    let col = Expr::col(column);
    let value = value.into();
    match operator {
        FilterOperator::Eq => Some(col.eq(value)),
        FilterOperator::Neq => Some(col.ne(value)),
        FilterOperator::Gt => Some(col.gt(value)),
        FilterOperator::Gte => Some(col.gte(value)),
        FilterOperator::Lt => Some(col.lt(value)),
        FilterOperator::Lte => Some(col.lte(value)),
        FilterOperator::Like | FilterOperator::In | FilterOperator::IsNull => None,
    }
}

/// Build a Sea-ORM `Expr` from a column, operator, and a JSON value without
/// any resource-aware normalisation (no case folding, enum casts or
/// `like_filterable` handling). Hand-written `resolve_joined_filters`
/// implementations can use it for plain columns; the derive uses
/// [`build_filter_expr`], which applies the same rules as main-entity filters.
///
/// Attempts to use range operators (`_gt`, `_gte`, `_lt`, `_lte`) against
/// unsupported value kinds return `Ok(None)` so the caller can skip the filter.
///
/// Returns `Ok(None)` for:
/// - empty strings / overlong strings (> `10_000` chars)
/// - range operators against UUIDs, bools, arrays, or null
/// - `IsNull` / `In` operators against non-matching value kinds
/// - objects as values
/// - a scalar value that cannot be parsed to the column's SQL type
///
/// # Errors
/// Returns `ApiError::BadRequest` when an element of an array-valued filter cannot
/// be parsed to the column's SQL type. The clause is rejected rather than dropped,
/// because a dropped clause returns unfiltered rows.
pub fn build_comparison_expr<C>(
    column: C,
    operator: super::joined::FilterOperator,
    value: &serde_json::Value,
) -> Result<Option<Expr>, crate::errors::ApiError>
where
    C: sea_orm::ColumnTrait + Copy,
{
    use super::joined::FilterOperator;
    use serde_json::Value;

    let col = || Expr::col(column);
    let column_def = column.def();
    let col_type = column_def.get_column_type();

    match value {
        Value::String(s) => {
            if !validate_field_value(s) {
                return Ok(None);
            }
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }

            // Try UUID first: ranges on UUIDs are meaningless, so only allow eq/neq.
            // This stays ahead of the type routing below, which would otherwise make
            // ranges expressible on a `Uuid` column.
            if let Ok(uuid_val) = Uuid::parse_str(trimmed) {
                return Ok(match operator {
                    FilterOperator::Eq => Some(col().eq(uuid_val)),
                    FilterOperator::Neq => Some(col().ne(uuid_val)),
                    _ => None,
                });
            }

            // Route by the column's SQL type, as the main-entity path does, so a date,
            // timestamp or numeric column binds a typed value rather than text.
            // `Like` keeps the string path below.
            if operator != FilterOperator::Like && binds_typed_value(col_type) {
                return Ok(typed_value_for_column(col_type, trimmed)
                    .and_then(|typed| ordered_comparison(column, operator, typed)));
            }

            Ok(match operator {
                FilterOperator::Eq => Some(col().eq(trimmed)),
                FilterOperator::Neq => Some(col().ne(trimmed)),
                FilterOperator::Gt => Some(col().gt(trimmed)),
                FilterOperator::Gte => Some(col().gte(trimmed)),
                FilterOperator::Lt => Some(col().lt(trimmed)),
                FilterOperator::Lte => Some(col().lte(trimmed)),
                FilterOperator::Like => {
                    let escaped = escape_like_wildcards(trimmed);
                    Some(col().like(LikeExpr::new(format!("%{escaped}%")).escape('!')))
                }
                FilterOperator::In | FilterOperator::IsNull => None,
            })
        }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                return Ok(ordered_comparison(column, operator, i));
            }
            // Values above i64::MAX bind as u64 rather than falling through to a
            // lossy f64 (a BIGINT UNSIGNED column would otherwise mis-compare).
            if let Some(u) = n.as_u64() {
                return Ok(ordered_comparison(column, operator, u));
            }
            Ok(n.as_f64()
                .and_then(|f| ordered_comparison(column, operator, f)))
        }
        Value::Bool(b) => Ok(match operator {
            FilterOperator::Eq => Some(col().eq(*b)),
            FilterOperator::Neq => Some(col().ne(*b)),
            _ => None,
        }),
        Value::Array(arr) => {
            if arr.is_empty() || arr.len() > MAX_FILTER_ARRAY_LEN {
                return Ok(None);
            }
            // Type-matched IN list so the bound values match the column type
            // (Postgres rejects `int_col IN ('1','3')` and `ts_col IN ('...')`).
            if binds_typed_value(col_type) {
                let values =
                    typed_array_values(col_type, sea_orm::IdenStatic::as_str(&column), arr)?;
                return Ok(Some(col().is_in(values)));
            }
            if let Some(expr) = typed_array_in_list(column, arr) {
                return Ok(Some(expr));
            }
            let strings: Vec<String> = arr
                .iter()
                .filter_map(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    Value::Number(n) => Some(n.to_string()),
                    Value::Bool(b) => Some(b.to_string()),
                    _ => None,
                })
                .collect();
            if strings.is_empty() {
                return Ok(None);
            }
            Ok(Some(col().is_in(strings)))
        }
        Value::Null => Ok(match operator {
            FilterOperator::Eq | FilterOperator::IsNull => Some(col().is_null()),
            FilterOperator::Neq => Some(col().is_not_null()),
            _ => None,
        }),
        Value::Object(_) => Ok(None),
    }
}

fn operator_symbol(operator: super::joined::FilterOperator) -> &'static str {
    use super::joined::FilterOperator;
    match operator {
        FilterOperator::Neq => "!=",
        FilterOperator::Gt => ">",
        FilterOperator::Gte => ">=",
        FilterOperator::Lt => "<",
        FilterOperator::Lte => "<=",
        FilterOperator::Eq | FilterOperator::Like | FilterOperator::In | FilterOperator::IsNull => {
            "="
        }
    }
}

fn operator_from_symbol(symbol: &str) -> super::joined::FilterOperator {
    use super::joined::FilterOperator;
    match symbol {
        "!=" => FilterOperator::Neq,
        ">" => FilterOperator::Gt,
        ">=" => FilterOperator::Gte,
        "<" => FilterOperator::Lt,
        "<=" => FilterOperator::Lte,
        _ => FilterOperator::Eq,
    }
}

/// Condition for one filter entry on a column of resource `T`.
///
/// This is the value dispatch behind both main-entity filters and the
/// derive-generated `resolve_joined_filters`: string comparisons fold case,
/// enum columns are cast to text on Postgres, `like_filterable` columns and
/// the `_like` operator use a case-insensitive `LIKE`, and date, decimal,
/// numeric, boolean and UUID columns bind typed values. Returns `Ok(None)` when
/// the value cannot be applied to the column (empty or overlong strings,
/// ranges on booleans, objects, or a scalar that does not parse to the column's
/// SQL type), in which case the filter is skipped.
///
/// # Errors
/// Returns `ApiError::BadRequest` when an element of an array-valued filter cannot
/// be parsed to the column's SQL type. Scalar values that do not parse are still
/// dropped rather than rejected.
pub fn build_filter_expr<T: crate::traits::CRUDResource, C: sea_orm::ColumnTrait + Copy>(
    column: C,
    column_name: &str,
    operator: super::joined::FilterOperator,
    value: &serde_json::Value,
    backend: DatabaseBackend,
) -> Result<Option<Expr>, crate::errors::ApiError> {
    use super::joined::FilterOperator;
    match value {
        serde_json::Value::String(s) => {
            if operator == FilterOperator::Like {
                if !validate_field_value(s) {
                    return Ok(None);
                }
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(build_like_condition(column_name, trimmed, backend)));
            }
            Ok(process_string_filter::<T>(
                column_name,
                operator_symbol(operator),
                s,
                column,
                backend,
            ))
        }
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                return Ok(ordered_comparison(column, operator, i));
            }
            if let Some(u) = n.as_u64() {
                return Ok(ordered_comparison(column, operator, u));
            }
            Ok(n.as_f64()
                .and_then(|f| ordered_comparison(column, operator, f)))
        }
        serde_json::Value::Bool(b) => Ok(match operator {
            FilterOperator::Eq => Some(Expr::col(column).eq(*b)),
            FilterOperator::Neq => Some(Expr::col(column).ne(*b)),
            _ => None,
        }),
        serde_json::Value::Array(values) => process_array_filter(
            values,
            column_name,
            column,
            T::is_enum_field(column_name),
            backend,
        ),
        serde_json::Value::Null => Ok(match operator {
            FilterOperator::Eq | FilterOperator::IsNull => Some(Expr::col(column).is_null()),
            FilterOperator::Neq => Some(Expr::col(column).is_not_null()),
            _ => None,
        }),
        serde_json::Value::Object(_) => Ok(None),
    }
}

/// Condition for one main-entity filter entry, dispatched on the JSON value type.
fn main_filter_expr<T: crate::traits::CRUDResource, C: sea_orm::ColumnTrait + Copy>(
    base_field: &str,
    operator: &str,
    value: &serde_json::Value,
    column: C,
    backend: DatabaseBackend,
) -> Result<Option<Expr>, crate::errors::ApiError> {
    build_filter_expr::<T, C>(
        column,
        base_field,
        operator_from_symbol(operator),
        value,
        backend,
    )
}

/// Build a table-qualified column reference (`"table"."column"`).
///
/// Used by the derive-macro-generated join code to name a child-table column
/// inside a sub-query, and available to hand-written
/// [`CRUDResource::resolve_joined_filters`](crate::traits::CRUDResource::resolve_joined_filters)
/// implementations. Both arguments accept anything convertible to a Sea-Query
/// identifier, including the `(table, column)` pair returned by
/// [`sea_orm::ColumnTrait::as_column_ref`].
#[must_use]
pub fn table_column_ref<T, C>(table: T, column: C) -> sea_orm::sea_query::ColumnRef
where
    T: sea_orm::sea_query::IntoIden,
    C: sea_orm::sea_query::IntoIden,
{
    use sea_orm::sea_query::{ColumnName, ColumnRef, TableName};

    ColumnRef::Column(ColumnName(
        Some(TableName(None, table.into_iden())),
        column.into_iden(),
    ))
}

/// Build a Sea-ORM `Condition` from a JSON filter string.
///
/// # Errors
/// Returns `ApiError::BadRequest` if the filter contains more than
/// `MAX_FILTER_CLAUSES` (100) keys, or if an element of an array-valued filter
/// cannot be parsed to its column's SQL type.
pub fn apply_filters<T: crate::traits::CRUDResource>(
    filter_str: Option<String>,
    searchable_columns: &[(&str, impl sea_orm::ColumnTrait)],
    backend: DatabaseBackend,
) -> Result<Condition, crate::errors::ApiError> {
    let filters = parse_filter_json(filter_str)?;
    let mut condition = Condition::all();

    // Handle fulltext search
    if let Some(fulltext_condition) =
        handle_fulltext_search::<T>(&filters, searchable_columns, backend)
    {
        condition = condition.add(fulltext_condition);
    }

    // Process other filters (excluding 'q')
    for (key, value) in &filters {
        if key == "q" {
            continue; // Skip fulltext search, already handled
        }

        // Validate field name
        if !is_valid_field_name(key) {
            continue;
        }

        // Parse comparison operator to get base field name
        // For "year_neq", this extracts "year" and stores the operator
        let (base_field, operator) = parse_comparison_operator(key).unwrap_or((key, "="));

        // Find the column in searchable columns using the BASE field name
        let column_opt = searchable_columns
            .iter()
            .find(|(col_name, _)| *col_name == base_field)
            .map(|(_, col)| col);

        if let Some(column) = column_opt {
            let filter_condition =
                main_filter_expr::<T, _>(base_field, operator, value, *column, backend)?;

            if let Some(filter_expr) = filter_condition {
                condition = condition.add(filter_expr);
            }
        }
    }

    Ok(condition)
}

/// Parse filters with support for dot-notation filtering on joined entities.
///
/// This function separates filters into:
/// - Main entity filters (applied to the primary table)
/// - Joined entity filters (applied after joining related tables)
///
/// # Example
///
/// ```text
/// Input:  filter={"name":"John","vehicles.make":"BMW","vehicles.year_gte":2020}
/// Output: main_condition: name = 'John'
///         joined_filters: [vehicles.make = 'BMW', vehicles.year >= 2020]
/// ```
/// Parse filters with support for dot-notation joined-entity filters.
///
/// # Errors
/// Returns `ApiError::BadRequest` if the filter contains more than
/// `MAX_FILTER_CLAUSES` (100) keys, or if an element of an array-valued filter
/// cannot be parsed to its column's SQL type.
pub fn apply_filters_with_joins<T: crate::traits::CRUDResource>(
    filter_str: Option<String>,
    searchable_columns: &[(&str, impl sea_orm::ColumnTrait)],
    backend: DatabaseBackend,
) -> Result<super::joined::ParsedFilters, crate::errors::ApiError> {
    use super::joined::{JoinedFilter, ParsedFilters, parse_dot_notation};

    let filters = parse_filter_json(filter_str)?;
    let mut result = ParsedFilters::default();

    // Get allowed joined columns for validation
    let joined_filterable = T::joined_filterable_columns();

    // Handle fulltext search (always goes to main condition)
    if let Some(fulltext_condition) =
        handle_fulltext_search::<T>(&filters, searchable_columns, backend)
    {
        result.main_condition = result.main_condition.add(fulltext_condition);
    }

    // Process other filters
    for (key, value) in &filters {
        if key == "q" {
            continue; // Skip fulltext search, already handled
        }

        // Check if this is a dot-notation filter (e.g., "vehicles.make")
        if let Some((join_field, column, operator)) = parse_dot_notation(key) {
            // Validate against allowed joined columns
            let full_path_for_check = format!("{join_field}.{column}");
            let is_allowed = joined_filterable
                .iter()
                .any(|c| c.full_path == full_path_for_check);

            if is_allowed {
                result.joined_filters.push(JoinedFilter {
                    join_field,
                    column,
                    operator,
                    value: value.clone(),
                });
                result.has_joined_filters = true;
            }
            // Skip invalid joined filters silently (security: don't expose schema)
            continue;
        }

        // Regular filter - validate field name and apply to main condition
        if !is_valid_field_name(key) {
            continue;
        }

        // Parse comparison operator to get base field name
        let (base_field, operator) = parse_comparison_operator(key).unwrap_or((key, "="));

        // Find the column in searchable columns using the BASE field name
        let column_opt = searchable_columns
            .iter()
            .find(|(col_name, _)| *col_name == base_field)
            .map(|(_, col)| col);

        if let Some(column) = column_opt {
            let filter_condition =
                main_filter_expr::<T, _>(base_field, operator, value, *column, backend)?;

            if let Some(filter_expr) = filter_condition {
                result.main_condition = result.main_condition.add(filter_expr);
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
#[path = "conditions_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "conditions_prop_tests.rs"]
mod prop_tests;
