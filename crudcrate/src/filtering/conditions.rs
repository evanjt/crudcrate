use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime};
use rust_decimal::Decimal;
use sea_orm::{
    Condition, DatabaseBackend,
    sea_query::{Alias, ColumnType, Expr, ExprTrait, LikeExpr, SimpleExpr},
};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

use super::search::{build_fulltext_condition, build_like_condition, escape_like_wildcards};

// Basic safety limits
const MAX_FIELD_VALUE_LENGTH: usize = 10_000;
const MAX_PAGE_SIZE: u64 = 1000;
const MAX_OFFSET: u64 = 1_000_000;
/// Maximum number of elements accepted in a single array-valued filter.
///
/// An array filter (`filter={"id":[...]}`) becomes one SQL `IN (...)` clause with
/// one bind parameter per element. `MAX_FILTER_CLAUSES` caps the number of keys, not
/// the length of any single array, so without this cap a single key could carry tens
/// of thousands of elements and blow past the backend bind-parameter ceiling (SQLite
/// 32766, Postgres/MySQL 65535). A 500 at the top, a query-planning DoS below it.
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
/// Exceeding this limit produces a `400 Bad Request` response — crudcrate
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
) -> SimpleExpr {
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
                // Cast enum fields to TEXT for LIKE operations
                match backend {
                    DatabaseBackend::Postgres => {
                        or_conditions = or_conditions.add(
                            SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(
                                Expr::cast_as(Expr::col(*col), Alias::new("TEXT")),
                            ))
                            .like(
                                LikeExpr::new(format!("%{}%", escaped_query.to_uppercase()))
                                    .escape('!'),
                            ),
                        );
                    }
                    _ => {
                        // For SQLite/MySQL, treat enum as string
                        or_conditions = or_conditions.add(
                            SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(Expr::col(
                                *col,
                            )))
                            .like(
                                LikeExpr::new(format!("%{}%", escaped_query.to_uppercase()))
                                    .escape('!'),
                            ),
                        );
                    }
                }
            } else {
                let cast_type = match backend {
                    DatabaseBackend::MySql => "CHAR",
                    _ => "TEXT",
                };
                or_conditions = or_conditions.add(
                    SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(Expr::cast_as(
                        Expr::col(*col),
                        Alias::new(cast_type),
                    )))
                    .like(LikeExpr::new(format!("%{}%", escaped_query.to_uppercase())).escape('!')),
                );
            }
        }
        return Some(or_conditions);
    }
    None
}

/// Apply a string comparison using the given operator.
fn apply_string_comparison(
    column: impl sea_orm::ColumnTrait + Copy,
    operator: &str,
    trimmed_value: &str,
) -> SimpleExpr {
    let col_upper = SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(Expr::col(column)));
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
) -> Option<SimpleExpr> {
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
        let col_expr = match backend {
            DatabaseBackend::Postgres => Expr::cast_as(Expr::col(column), Alias::new("TEXT")),
            _ => Expr::col(column),
        };
        let col_upper = SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(col_expr));
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

fn process_number_filter(
    key: &str,
    number: &serde_json::Number,
    column: impl sea_orm::ColumnTrait + Copy,
    searchable_columns: &[(&str, impl sea_orm::ColumnTrait)],
) -> Option<SimpleExpr> {
    if let Some((base_field, operator)) = parse_comparison_operator(key) {
        // Check if the base field exists in searchable columns
        if searchable_columns
            .iter()
            .any(|(col_name, _)| *col_name == base_field)
        {
            if let Some(int_value) = number.as_i64() {
                return Some(apply_typed_comparison(column, operator, int_value));
            } else if let Some(uint_value) = number.as_u64() {
                // Values above i64::MAX (e.g. a BIGINT UNSIGNED column) must bind as
                // u64, not fall through to a lossy f64 that mis-matches rows.
                return Some(apply_typed_comparison(column, operator, uint_value));
            } else if let Some(float_value) = number.as_f64() {
                return Some(apply_typed_comparison(column, operator, float_value));
            }
        }
    } else {
        // Regular number equality
        if let Some(int_value) = number.as_i64() {
            return Some(Expr::col(column).eq(int_value));
        } else if let Some(uint_value) = number.as_u64() {
            return Some(Expr::col(column).eq(uint_value));
        } else if let Some(float_value) = number.as_f64() {
            return Some(Expr::col(column).eq(float_value));
        }
    }
    None
}

/// Build a type-matched `IN (...)` expression for a homogeneous JSON array so the
/// bound values match the column type on strict backends. Postgres rejects
/// `int_col IN ('1','3')` and `bool_col IN ('true')` (`operator does not exist:
/// integer = text`); SQLite's loose typing silently accepted the stringified form.
/// Returns `None` unless every element is an integer, every element is a number,
/// or every element is a boolean — callers fall back to a string IN list for
/// string/enum/mixed arrays.
fn typed_array_in_list<C: sea_orm::ColumnTrait + Copy>(
    column: C,
    array_values: &[serde_json::Value],
) -> Option<SimpleExpr> {
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
    column: impl sea_orm::ColumnTrait + Copy,
    is_enum: bool,
    backend: DatabaseBackend,
) -> Option<SimpleExpr> {
    if array_values.is_empty() || array_values.len() > MAX_FILTER_ARRAY_LEN {
        return None;
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
        return Some(Expr::col(column).is_in(uuid_values));
    }

    // Type-matched IN list (integers/floats/bools) so the bound values match the
    // column type on strict backends. Enum columns keep the string path below —
    // their casted/uppercased comparison needs text binds.
    if !is_enum && let Some(expr) = typed_array_in_list(column, array_values) {
        return Some(expr);
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
            // For enum fields, cast column to TEXT and uppercase both sides
            // so that native PostgreSQL ENUMs work with string bind parameters
            let col_expr = match backend {
                DatabaseBackend::Postgres => Expr::cast_as(Expr::col(column), Alias::new("TEXT")),
                _ => Expr::col(column),
            };
            let col_upper = SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(col_expr));
            let upper_values: Vec<String> = in_values.iter().map(|v| v.to_uppercase()).collect();
            return Some(col_upper.is_in(upper_values));
        }
        return Some(Expr::col(column).is_in(in_values));
    }
    None
}

/// Build a Sea-ORM `SimpleExpr` from a column, operator, and a JSON value.
///
/// Used by the derive-macro-generated `resolve_joined_filters` to translate
/// parsed [`crate::filtering::joined::JoinedFilter`] entries into concrete
/// sub-query conditions on child tables.
///
/// Unlike the main-entity filter path, this builder does not apply enum or
/// fulltext normalization — joined filters target plain columns (strings,
/// numbers, UUIDs, bools). Attempts to use range operators (`_gt`, `_gte`,
/// `_lt`, `_lte`) against unsupported value kinds return `None` so the caller
/// can silently skip the filter, matching the existing "skip invalid filters"
/// convention.
///
/// Returns `None` for:
/// - empty strings / overlong strings (> `10_000` chars)
/// - range operators against UUIDs, bools, arrays, or null
/// - `IsNull` / `In` operators against non-matching value kinds
/// - objects as values
#[must_use]
pub fn build_comparison_expr<C>(
    column: C,
    operator: super::joined::FilterOperator,
    value: &serde_json::Value,
) -> Option<SimpleExpr>
where
    C: sea_orm::ColumnTrait + Copy,
{
    use super::joined::FilterOperator;
    use serde_json::Value;

    let col = || Expr::col(column);

    match value {
        Value::String(s) => {
            if !validate_field_value(s) {
                return None;
            }
            let trimmed = s.trim();
            if trimmed.is_empty() {
                return None;
            }

            // Try UUID first — ranges on UUIDs are meaningless, so only allow eq/neq
            if let Ok(uuid_val) = Uuid::parse_str(trimmed) {
                return match operator {
                    FilterOperator::Eq => Some(col().eq(uuid_val)),
                    FilterOperator::Neq => Some(col().ne(uuid_val)),
                    _ => None,
                };
            }

            match operator {
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
            }
        }
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                return match operator {
                    FilterOperator::Eq => Some(col().eq(i)),
                    FilterOperator::Neq => Some(col().ne(i)),
                    FilterOperator::Gt => Some(col().gt(i)),
                    FilterOperator::Gte => Some(col().gte(i)),
                    FilterOperator::Lt => Some(col().lt(i)),
                    FilterOperator::Lte => Some(col().lte(i)),
                    _ => None,
                };
            }
            // Values above i64::MAX bind as u64 rather than falling through to a
            // lossy f64 (a BIGINT UNSIGNED column would otherwise mis-compare).
            if let Some(u) = n.as_u64() {
                return match operator {
                    FilterOperator::Eq => Some(col().eq(u)),
                    FilterOperator::Neq => Some(col().ne(u)),
                    FilterOperator::Gt => Some(col().gt(u)),
                    FilterOperator::Gte => Some(col().gte(u)),
                    FilterOperator::Lt => Some(col().lt(u)),
                    FilterOperator::Lte => Some(col().lte(u)),
                    _ => None,
                };
            }
            if let Some(f) = n.as_f64() {
                return match operator {
                    FilterOperator::Eq => Some(col().eq(f)),
                    FilterOperator::Neq => Some(col().ne(f)),
                    FilterOperator::Gt => Some(col().gt(f)),
                    FilterOperator::Gte => Some(col().gte(f)),
                    FilterOperator::Lt => Some(col().lt(f)),
                    FilterOperator::Lte => Some(col().lte(f)),
                    _ => None,
                };
            }
            None
        }
        Value::Bool(b) => match operator {
            FilterOperator::Eq => Some(col().eq(*b)),
            FilterOperator::Neq => Some(col().ne(*b)),
            _ => None,
        },
        Value::Array(arr) => {
            if arr.is_empty() || arr.len() > MAX_FILTER_ARRAY_LEN {
                return None;
            }
            // Type-matched IN list so integer/float/bool arrays bind as their
            // native type (Postgres rejects `int_col IN ('1','3')`).
            if let Some(expr) = typed_array_in_list(column, arr) {
                return Some(expr);
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
                return None;
            }
            Some(col().is_in(strings))
        }
        Value::Null => match operator {
            FilterOperator::Eq | FilterOperator::IsNull => Some(col().is_null()),
            FilterOperator::Neq => Some(col().is_not_null()),
            _ => None,
        },
        Value::Object(_) => None,
    }
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
/// [`MAX_FILTER_CLAUSES`] keys.
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
            // Handle different value types
            let filter_condition = match value {
                serde_json::Value::String(string_value) => {
                    process_string_filter::<T>(base_field, operator, string_value, *column, backend)
                }
                serde_json::Value::Number(number) => {
                    process_number_filter(key, number, *column, searchable_columns)
                }
                serde_json::Value::Bool(bool_value) => Some(Expr::col(*column).eq(*bool_value)),
                serde_json::Value::Array(array_values) => process_array_filter(
                    array_values,
                    *column,
                    T::is_enum_field(base_field),
                    backend,
                ),
                serde_json::Value::Null => Some(if operator == "!=" {
                    Expr::col(*column).is_not_null()
                } else {
                    Expr::col(*column).is_null()
                }),
                serde_json::Value::Object(_) => None, // Skip unsupported value types
            };

            if let Some(filter_expr) = filter_condition {
                condition = condition.add(filter_expr);
            }
        }
    }

    Ok(condition)
}

#[must_use]
pub fn parse_range(range_str: Option<String>) -> (u64, u64) {
    range_str.map_or((0, 9), |r| {
        serde_json::from_str::<[u64; 2]>(&r).map_or((0, 9), |range| (range[0], range[1]))
    })
}

#[must_use]
pub fn parse_pagination(params: &crate::models::FilterOptions) -> (u64, u64) {
    // `std::cmp::min` is spelled out throughout: SeaQuery's blanket `ExprTrait` impl
    // covers every type, so the `.min()` method call is ambiguous with `Ord::min`.
    if let (Some(page), Some(per_page)) = (params.page, params.per_page) {
        // Standard REST pagination (1-based page numbers)
        // Enforce maximum page size to prevent DoS
        let safe_per_page = std::cmp::min(per_page, MAX_PAGE_SIZE);

        // Use saturating_mul to prevent overflow panic
        let offset = (page.saturating_sub(1)).saturating_mul(safe_per_page);

        // Enforce maximum offset to prevent excessive database queries
        let safe_offset = std::cmp::min(offset, MAX_OFFSET);

        (safe_offset, safe_per_page)
    } else if let Some(range) = &params.range {
        // React Admin pagination
        let (start, end) = parse_range(Some(range.clone()));
        // saturating_add: a client-supplied end of u64::MAX would otherwise overflow
        // the `+ 1` (panic in debug/test, silent wrap-to-zero in release).
        let limit = std::cmp::min(
            end.saturating_sub(start).saturating_add(1),
            MAX_PAGE_SIZE,
        );
        let safe_start = std::cmp::min(start, MAX_OFFSET);
        (safe_start, limit)
    } else {
        // Default pagination
        (0, 10)
    }
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
/// [`MAX_FILTER_CLAUSES`] keys.
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
            // Handle different value types (same as apply_filters)
            let filter_condition = match value {
                serde_json::Value::String(string_value) => {
                    process_string_filter::<T>(base_field, operator, string_value, *column, backend)
                }
                serde_json::Value::Number(number) => {
                    process_number_filter(key, number, *column, searchable_columns)
                }
                serde_json::Value::Bool(bool_value) => Some(Expr::col(*column).eq(*bool_value)),
                serde_json::Value::Array(array_values) => process_array_filter(
                    array_values,
                    *column,
                    T::is_enum_field(base_field),
                    backend,
                ),
                serde_json::Value::Null => Some(if operator == "!=" {
                    Expr::col(*column).is_not_null()
                } else {
                    Expr::col(*column).is_null()
                }),
                serde_json::Value::Object(_) => None,
            };

            if let Some(filter_expr) = filter_condition {
                result.main_condition = result.main_condition.add(filter_expr);
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that malicious field names are rejected
    #[test]
    fn test_field_name_validation_rejects_sql_injection() {
        // These are currently rejected by the basic validation
        let rejected_names = vec![
            "../../../etc/passwd", // Path traversal (contains ..)
            "id..name",            // Double dots
            "_internal",           // Starts with underscore
            "",                    // Empty
        ];

        for malicious_name in rejected_names {
            assert!(
                !is_valid_field_name(malicious_name),
                "Should reject malicious field name: {malicious_name}"
            );
        }

        // Test too long separately
        let too_long = "a".repeat(101);
        assert!(
            !is_valid_field_name(&too_long),
            "Should reject field names longer than 100 chars"
        );
    }

    /// Test that valid field names are accepted
    #[test]
    fn test_field_name_validation_accepts_valid_names() {
        let valid_names = vec!["id", "user_name", "created_at", "field123"];

        for valid_name in valid_names {
            assert!(
                is_valid_field_name(valid_name),
                "Should accept valid field name: {valid_name}"
            );
        }

        // Test max length separately
        let max_length_name = "a".repeat(100);
        assert!(
            is_valid_field_name(&max_length_name),
            "Should accept 100-char field name"
        );
    }

    /// Test that excessively long field values are rejected
    #[test]
    fn test_field_value_length_validation() {
        let short_value = "a".repeat(100);
        let max_value = "a".repeat(MAX_FIELD_VALUE_LENGTH);
        let too_long_value = "a".repeat(MAX_FIELD_VALUE_LENGTH + 1);

        assert!(
            validate_field_value(&short_value),
            "Short values should be valid"
        );
        assert!(
            validate_field_value(&max_value),
            "Max length values should be valid"
        );
        assert!(
            !validate_field_value(&too_long_value),
            "Overly long values should be invalid"
        );
    }

    /// TDD: Pagination should enforce maximum page size
    /// This test will FAIL until we add `MAX_PAGE_SIZE` enforcement
    #[test]
    fn test_pagination_enforces_max_page_size() {
        const MAX_PAGE_SIZE: u64 = 1000;

        let params = crate::models::FilterOptions {
            page: Some(1),
            per_page: Some(999_999), // Requesting huge page size
            ..Default::default()
        };

        let (_offset, limit) = parse_pagination(&params);

        // After fix: Should be capped at MAX_PAGE_SIZE
        assert!(
            limit <= MAX_PAGE_SIZE,
            "Page size should be capped at {MAX_PAGE_SIZE}, got {limit}"
        );
    }

    /// TDD: Pagination should enforce maximum offset
    /// This test will FAIL until we add `MAX_OFFSET` enforcement
    #[test]
    fn test_pagination_enforces_max_offset() {
        const MAX_OFFSET: u64 = 1_000_000;

        let params = crate::models::FilterOptions {
            page: Some(1_000_000), // Huge page number
            per_page: Some(100),
            ..Default::default()
        };

        let (offset, _limit) = parse_pagination(&params);

        // After fix: Should be capped at MAX_OFFSET
        assert!(
            offset <= MAX_OFFSET,
            "Offset should be capped at {MAX_OFFSET}, got {offset}"
        );
    }

    /// TDD: Pagination should handle overflow with `saturating_mul`
    /// This test will FAIL until we fix the overflow panic
    #[test]
    fn test_pagination_handles_overflow_gracefully() {
        let params = crate::models::FilterOptions {
            page: Some(u64::MAX),
            per_page: Some(u64::MAX),
            ..Default::default()
        };

        // Should NOT panic - should use saturating arithmetic
        let (_offset, _limit) = parse_pagination(&params);
        // After fix: This should succeed without panic
    }

    /// Test comparison operator parsing
    #[test]
    fn test_comparison_operator_parsing() {
        assert_eq!(parse_comparison_operator("age_gte"), Some(("age", ">=")));
        assert_eq!(parse_comparison_operator("age_lte"), Some(("age", "<=")));
        assert_eq!(parse_comparison_operator("age_gt"), Some(("age", ">")));
        assert_eq!(parse_comparison_operator("age_lt"), Some(("age", "<")));
        assert_eq!(parse_comparison_operator("age_neq"), Some(("age", "!=")));
        assert_eq!(parse_comparison_operator("age"), None);
    }

    /// The LIKE paths in this module share search.rs's `!`-based escaper, which is
    /// always paired with an explicit `ESCAPE '!'` clause (see
    /// `test_build_comparison_expr_like_escapes_wildcards`). Backslash escaping was
    /// removed because it is a no-op on SQLite (`.like()` emits no ESCAPE clause).
    #[test]
    fn test_escape_like_wildcards() {
        assert_eq!(escape_like_wildcards("normal text"), "normal text");
        assert_eq!(escape_like_wildcards("test%"), "test!%");
        assert_eq!(escape_like_wildcards("test_value"), "test!_value");
        assert_eq!(escape_like_wildcards("%_"), "!%!_");
        assert_eq!(escape_like_wildcards("!"), "!!");
        assert_eq!(escape_like_wildcards("100% complete"), "100!% complete");
    }

    // ========================================================================
    // build_comparison_expr — direct coverage of the public joined-filter
    // expression builder used by derive-generated resolve_joined_filters.
    // ========================================================================

    mod cmp_entity {
        use sea_orm::entity::prelude::*;

        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "cmp_things")]
        pub struct Model {
            #[sea_orm(primary_key)]
            pub id: i32,
            pub name: String,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    }

    use crate::filtering::joined::FilterOperator;

    /// Render an expression to inlined SQLite SQL so the ESCAPE clause and the
    /// (escaped) bound pattern are both visible as text.
    fn cmp_sql(expr: SimpleExpr) -> String {
        use sea_orm::sea_query::{Query, SqliteQueryBuilder};
        Query::select()
            .column(cmp_entity::Column::Id)
            .from(cmp_entity::Entity)
            .and_where(expr)
            .to_string(SqliteQueryBuilder)
    }

    /// A1 regression: the joined `_like` path must escape user wildcards with `!`
    /// AND declare `ESCAPE '!'` so the escaping is not a no-op on SQLite.
    #[test]
    fn test_build_comparison_expr_like_escapes_wildcards() {
        let expr = build_comparison_expr(
            cmp_entity::Column::Name,
            FilterOperator::Like,
            &serde_json::json!("100%"),
        )
        .expect("Like on a string builds an expression");
        let sql = cmp_sql(expr);
        assert!(
            sql.contains("ESCAPE '!'"),
            "LIKE must declare ESCAPE '!': {sql}"
        );
        assert!(
            sql.contains("100!%"),
            "user-supplied wildcard must be escaped with !: {sql}"
        );
    }

    #[test]
    fn test_build_comparison_expr_string_ops_build() {
        for op in [
            FilterOperator::Eq,
            FilterOperator::Neq,
            FilterOperator::Gt,
            FilterOperator::Gte,
            FilterOperator::Lt,
            FilterOperator::Lte,
        ] {
            assert!(
                build_comparison_expr(cmp_entity::Column::Name, op, &serde_json::json!("abc"))
                    .is_some(),
                "string {op:?} should build an expression"
            );
        }
    }

    #[test]
    fn test_build_comparison_expr_empty_and_overlong_string_none() {
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::Eq,
                &serde_json::json!("")
            )
            .is_none()
        );
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::Eq,
                &serde_json::json!("   "),
            )
            .is_none()
        );
        let overlong = "a".repeat(10_001);
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::Eq,
                &serde_json::json!(overlong),
            )
            .is_none()
        );
    }

    #[test]
    fn test_build_comparison_expr_uuid_only_eq_neq() {
        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::Eq,
                &serde_json::json!(uuid)
            )
            .is_some()
        );
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::Neq,
                &serde_json::json!(uuid)
            )
            .is_some()
        );
        // Ranges and LIKE on a UUID are meaningless -> None.
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::Gt,
                &serde_json::json!(uuid)
            )
            .is_none()
        );
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::Like,
                &serde_json::json!(uuid)
            )
            .is_none()
        );
    }

    #[test]
    fn test_build_comparison_expr_number_ops_and_rejections() {
        for op in [
            FilterOperator::Eq,
            FilterOperator::Neq,
            FilterOperator::Gt,
            FilterOperator::Gte,
            FilterOperator::Lt,
            FilterOperator::Lte,
        ] {
            assert!(
                build_comparison_expr(cmp_entity::Column::Id, op, &serde_json::json!(42)).is_some()
            );
            assert!(
                build_comparison_expr(cmp_entity::Column::Id, op, &serde_json::json!(3.5))
                    .is_some()
            );
        }
        // In / IsNull are not valid against a scalar number -> None.
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Id,
                FilterOperator::In,
                &serde_json::json!(42)
            )
            .is_none()
        );
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Id,
                FilterOperator::IsNull,
                &serde_json::json!(42),
            )
            .is_none()
        );
    }

    /// A JSON integer above `i64::MAX` must bind as an exact `u64`, not fall through
    /// to a lossy `f64`. 9223372036854775810 (= i64::MAX as u64 + 3) is NOT exactly
    /// representable in `f64` (it rounds to 9223372036854775808), so the rendered SQL
    /// proves whether the value was preserved.
    #[test]
    fn test_build_comparison_expr_u64_above_i64_max_binds_exact() {
        let big: u64 = (i64::MAX as u64) + 3;
        assert_eq!(big, 9_223_372_036_854_775_810);
        let v = serde_json::json!(big);
        assert!(v.as_i64().is_none(), "value must exceed i64::MAX");

        let expr = build_comparison_expr(cmp_entity::Column::Id, FilterOperator::Gte, &v)
            .expect("u64 value builds an expression");
        let sql = cmp_sql(expr);
        assert!(
            sql.contains("9223372036854775810"),
            "u64 above i64::MAX must bind exactly, got lossy SQL: {sql}"
        );
    }

    /// Direct callers of the public `build_comparison_expr` are also protected: an
    /// array longer than the element cap yields `None` instead of an oversized `IN`.
    #[test]
    fn test_build_comparison_expr_rejects_overlong_array() {
        let arr: Vec<serde_json::Value> = (0..=super::MAX_FILTER_ARRAY_LEN as i64)
            .map(|n| serde_json::json!(n))
            .collect();
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Id,
                FilterOperator::In,
                &serde_json::Value::Array(arr)
            )
            .is_none(),
            "array over MAX_FILTER_ARRAY_LEN must not build an IN expression"
        );
    }

    #[test]
    fn test_build_comparison_expr_bool_eq_neq_only() {
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::Eq,
                &serde_json::json!(true)
            )
            .is_some()
        );
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::Neq,
                &serde_json::json!(false),
            )
            .is_some()
        );
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::Gt,
                &serde_json::json!(true)
            )
            .is_none()
        );
    }

    #[test]
    fn test_build_comparison_expr_array_null_object() {
        // Non-empty array -> IN.
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::In,
                &serde_json::json!(["a", "b"]),
            )
            .is_some()
        );
        // Empty array, or an array of only objects (no extractable scalars) -> None.
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::In,
                &serde_json::json!([])
            )
            .is_none()
        );
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::In,
                &serde_json::json!([{"k": "v"}]),
            )
            .is_none()
        );
        // Null + Eq/IsNull -> IS NULL; Null + Neq -> IS NOT NULL; other operators -> None.
        let eq_null = build_comparison_expr(
            cmp_entity::Column::Name,
            FilterOperator::Eq,
            &serde_json::Value::Null,
        )
        .expect("Eq + null builds an expression");
        assert!(
            cmp_sql(eq_null).contains("IS NULL"),
            "Eq + null must render IS NULL"
        );
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::IsNull,
                &serde_json::Value::Null,
            )
            .is_some()
        );
        let neq_null = build_comparison_expr(
            cmp_entity::Column::Name,
            FilterOperator::Neq,
            &serde_json::Value::Null,
        )
        .expect("Neq + null builds an expression");
        assert!(
            cmp_sql(neq_null).contains("IS NOT NULL"),
            "Neq + null must render IS NOT NULL (paired/has-value filter)"
        );
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::Gt,
                &serde_json::Value::Null
            )
            .is_none()
        );
        // Object value is unsupported -> None.
        assert!(
            build_comparison_expr(
                cmp_entity::Column::Name,
                FilterOperator::Eq,
                &serde_json::json!({"k": "v"}),
            )
            .is_none()
        );
    }

    /// Each operator renders a native `col <op> value` against the real column, with
    /// no `UPPER()` wrapper and the value bound rather than spliced. The unknown
    /// operator falls back to equality.
    #[test]
    fn test_apply_typed_comparison_operators() {
        // (input operator, symbol sea-query renders — inequality is `<>`, not `!=`)
        let cases = [
            (">=", ">="),
            ("<=", "<="),
            (">", ">"),
            ("<", "<"),
            ("!=", "<>"),
            ("unknown", "="),
        ];
        for (op, rendered) in cases {
            let expr = apply_typed_comparison(cmp_entity::Column::Id, op, 18_i64);
            let sql = cmp_sql(expr);
            assert!(
                sql.contains(&format!(r#""id" {rendered} 18"#)),
                "operator {op} should render `id {rendered} 18`: {sql}"
            );
        }
    }

    /// Test JSON filter parsing
    #[test]
    fn test_parse_filter_json_valid() {
        let filter_str = Some(r#"{"name": "John", "age": 30}"#.to_string());
        let parsed = parse_filter_json(filter_str).expect("valid filter");

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed.get("name").and_then(|v| v.as_str()), Some("John"));
        assert_eq!(parsed.get("age").and_then(|v| v.as_i64()), Some(30));
    }

    #[test]
    fn test_parse_filter_json_invalid() {
        // Invalid JSON historically returns empty (preserved behavior); over-limit
        // is the new strict path tested separately below.
        let filter_str = Some("{invalid json}".to_string());
        let parsed = parse_filter_json(filter_str).expect("invalid-json path is lenient");
        assert_eq!(parsed.len(), 0);
    }

    #[test]
    fn test_parse_filter_json_none() {
        let parsed = parse_filter_json(None).expect("None is valid");
        assert_eq!(parsed.len(), 0);
    }

    #[test]
    fn test_parse_filter_json_empty() {
        let filter_str = Some("{}".to_string());
        let parsed = parse_filter_json(filter_str).expect("empty object is valid");
        assert_eq!(parsed.len(), 0);
    }

    #[test]
    fn test_parse_filter_json_at_limit_is_accepted() {
        let mut entries: Vec<String> = Vec::with_capacity(MAX_FILTER_CLAUSES);
        for i in 0..MAX_FILTER_CLAUSES {
            entries.push(format!("\"f{i}\":{i}"));
        }
        let filter_str = Some(format!("{{{}}}", entries.join(",")));
        let parsed = parse_filter_json(filter_str).expect("at-limit filter must be accepted");
        assert_eq!(parsed.len(), MAX_FILTER_CLAUSES);
    }

    #[test]
    fn test_parse_filter_json_rejects_when_over_limit() {
        let mut entries: Vec<String> = Vec::with_capacity(MAX_FILTER_CLAUSES + 1);
        for i in 0..=MAX_FILTER_CLAUSES {
            entries.push(format!("\"f{i}\":{i}"));
        }
        let filter_str = Some(format!("{{{}}}", entries.join(",")));
        let err = parse_filter_json(filter_str)
            .expect_err("over-limit filter must be rejected, not silently dropped");
        assert!(
            matches!(err, crate::errors::ApiError::BadRequest { .. }),
            "expected BadRequest, got {err:?}"
        );
    }

    /// An array-valued filter at the element cap is accepted, and one element over is
    /// rejected with `BadRequest` rather than fanning out into an oversized `IN (...)`.
    #[test]
    fn test_parse_filter_json_rejects_overlong_array() {
        let at_limit: Vec<i64> = (0..MAX_FILTER_ARRAY_LEN as i64).collect();
        let filter_str = Some(serde_json::json!({ "id": at_limit }).to_string());
        let parsed = parse_filter_json(filter_str).expect("array at the cap is accepted");
        assert_eq!(parsed.len(), 1);

        let over_limit: Vec<i64> = (0..=MAX_FILTER_ARRAY_LEN as i64).collect();
        let filter_str = Some(serde_json::json!({ "id": over_limit }).to_string());
        let err = parse_filter_json(filter_str)
            .expect_err("array one element over the cap must be rejected");
        assert!(
            matches!(err, crate::errors::ApiError::BadRequest { .. }),
            "expected BadRequest, got {err:?}"
        );
    }

    /// Test comparison operators with edge cases
    #[test]
    fn test_comparison_operator_edge_cases() {
        // Field name that ends with operator-like suffix but isn't one
        assert_eq!(parse_comparison_operator("created_at"), None);
        assert_eq!(parse_comparison_operator("_gte"), Some(("", ">=")));

        // Multiple suffixes (should match the longest/last one)
        assert_eq!(
            parse_comparison_operator("field_gte_lte"),
            Some(("field_gte", "<="))
        );
    }

    /// Test field name validation edge cases
    #[test]
    fn test_field_name_validation_edge_cases() {
        // Boundary cases
        assert!(is_valid_field_name("a")); // Single char
        assert!(is_valid_field_name("a".repeat(100).as_str())); // Exactly 100
        assert!(!is_valid_field_name("a".repeat(101).as_str())); // 101

        // Special chars that should be allowed
        assert!(is_valid_field_name("field_123"));
        assert!(is_valid_field_name("Field123"));

        // Special chars that should be rejected
        assert!(!is_valid_field_name("field..name"));
        assert!(!is_valid_field_name(".."));
        assert!(!is_valid_field_name("_private"));
    }

    /// The typed builder accepts the numeric Rust types the number-filter path feeds
    /// it (i64, f64, and u64 above i64::MAX), binding each without a lossy cast.
    #[test]
    fn test_apply_typed_comparison_various_types() {
        let i64_sql = cmp_sql(apply_typed_comparison(
            cmp_entity::Column::Id,
            ">=",
            100_i64,
        ));
        assert!(i64_sql.contains(r#""id" >= 100"#), "{i64_sql}");

        let f64_sql = cmp_sql(apply_typed_comparison(
            cmp_entity::Column::Id,
            "<=",
            99.99_f64,
        ));
        assert!(
            f64_sql.contains(r#""id" <= "#) && f64_sql.contains("99.99"),
            "{f64_sql}"
        );

        // A u64 above i64::MAX must bind exactly, not fall through to a lossy f64.
        let big: u64 = (i64::MAX as u64) + 3;
        let u64_sql = cmp_sql(apply_typed_comparison(cmp_entity::Column::Id, ">", big));
        assert!(u64_sql.contains("9223372036854775810"), "{u64_sql}");
    }

    /// `typed_value_for_column` parses a valid string for every supported column
    /// type; text-like types return `None` so the caller keeps the string path.
    #[test]
    fn typed_value_for_column_parses_supported_types() {
        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        let ok = [
            (ColumnType::Date, "2026-01-15"),
            (ColumnType::Time, "13:45:30"),
            (ColumnType::DateTime, "2026-01-15T13:45:30"),
            (ColumnType::Timestamp, "2026-01-15 13:45:30"),
            (
                ColumnType::TimestampWithTimeZone,
                "2026-01-15T13:45:30+02:00",
            ),
            (ColumnType::Decimal(None), "10.50"),
            (ColumnType::Money(None), "10.50"),
            (ColumnType::Uuid, uuid),
            (ColumnType::Float, "1.5"),
            (ColumnType::Double, "1.5"),
            (ColumnType::TinyInteger, "1"),
            (ColumnType::SmallInteger, "-7"),
            (ColumnType::Integer, "42"),
            (ColumnType::BigInteger, "9000000000"),
            (ColumnType::TinyUnsigned, "1"),
            (ColumnType::SmallUnsigned, "7"),
            (ColumnType::Unsigned, "42"),
            (ColumnType::BigUnsigned, "9000000000"),
            (ColumnType::Boolean, "true"),
        ];
        for (col_type, raw) in ok {
            assert!(
                typed_value_for_column(&col_type, raw).is_some(),
                "{col_type:?} should parse {raw:?} to a typed value"
            );
        }
        // Text-like columns are not typed-bound; the caller keeps the string path.
        assert!(typed_value_for_column(&ColumnType::Text, "anything").is_none());
        assert!(typed_value_for_column(&ColumnType::Char(None), "x").is_none());
    }

    /// An unparseable value for a typed column yields `None`, so the caller drops
    /// that clause (fail-closed) rather than emitting a comparison the backend rejects.
    #[test]
    fn typed_value_for_column_rejects_unparseable_values() {
        let bad = [
            (ColumnType::Date, "not-a-date"),
            (ColumnType::Time, "25:99:99"),
            (ColumnType::DateTime, "nope"),
            (ColumnType::TimestampWithTimeZone, "2026-01-15"), // missing time/offset
            (ColumnType::Decimal(None), "abc"),
            (ColumnType::Uuid, "not-a-uuid"),
            (ColumnType::Float, "one"),
            (ColumnType::Double, "1.2.3"),
            (ColumnType::Integer, "3.5"), // not an integer
            (ColumnType::Unsigned, "-1"), // negative on an unsigned column
            (ColumnType::Boolean, "1"),   // only literal true/false parse
        ];
        for (col_type, raw) in bad {
            assert!(
                typed_value_for_column(&col_type, raw).is_none(),
                "{col_type:?} should reject {raw:?}"
            );
        }
    }

    /// `parse_naive_datetime` accepts the `T`-separated and space-separated forms and
    /// a full RFC 3339 timestamp (normalised to naive UTC), and rejects junk.
    #[test]
    fn parse_naive_datetime_accepts_supported_formats() {
        assert!(parse_naive_datetime("2026-01-15T13:45:30").is_some());
        assert!(parse_naive_datetime("2026-01-15 13:45:30").is_some());
        assert!(parse_naive_datetime("2026-01-15T13:45:30+02:00").is_some());
        assert!(parse_naive_datetime("garbage").is_none());
    }

    /// `binds_typed_value` selects the typed path for non-text columns and leaves
    /// text/char columns on the case-insensitive string path.
    #[test]
    fn binds_typed_value_covers_non_text_columns() {
        for col_type in [
            ColumnType::Date,
            ColumnType::TimestampWithTimeZone,
            ColumnType::Decimal(None),
            ColumnType::Uuid,
            ColumnType::Double,
            ColumnType::BigInteger,
            ColumnType::Boolean,
        ] {
            assert!(
                binds_typed_value(&col_type),
                "{col_type:?} should bind typed"
            );
        }
        assert!(!binds_typed_value(&ColumnType::Text));
        assert!(!binds_typed_value(&ColumnType::Char(None)));
    }

    /// The number-filter range path binds the real column with a typed value for each
    /// JSON numeric kind (i64, u64 above i64::MAX, f64); a bare key is plain equality,
    /// and a key whose base field isn't searchable is dropped.
    #[test]
    fn process_number_filter_binds_typed_values() {
        let cols: &[(&str, cmp_entity::Column)] = &[("id", cmp_entity::Column::Id)];

        let i = serde_json::Number::from(42_i64);
        let gte = process_number_filter("id_gte", &i, cmp_entity::Column::Id, cols).unwrap();
        assert!(cmp_sql(gte).contains(r#""id" >= 42"#));

        // A u64 above i64::MAX must bind exactly, not fall through to a lossy f64.
        let big = serde_json::Number::from((i64::MAX as u64) + 3);
        let lte = process_number_filter("id_lte", &big, cmp_entity::Column::Id, cols).unwrap();
        assert!(cmp_sql(lte).contains("9223372036854775810"));

        let f = serde_json::Number::from_f64(1.5).unwrap();
        let lt = process_number_filter("id_lt", &f, cmp_entity::Column::Id, cols).unwrap();
        let sql = cmp_sql(lt);
        assert!(sql.contains(r#""id" < "#) && sql.contains("1.5"), "{sql}");

        // Bare key -> equality.
        let eq = process_number_filter("id", &i, cmp_entity::Column::Id, cols).unwrap();
        assert!(cmp_sql(eq).contains(r#""id" = 42"#));

        // Base field not in the searchable set -> dropped.
        assert!(process_number_filter("missing_gte", &i, cmp_entity::Column::Id, cols).is_none());
    }

    // ========================================================================
    // PAGINATION TESTS - Range parsing and default pagination
    // ========================================================================

    /// Test parse_range with valid JSON array
    #[test]
    fn test_parse_range_valid() {
        let (start, end) = parse_range(Some("[0,9]".to_string()));
        assert_eq!(start, 0);
        assert_eq!(end, 9);

        let (start, end) = parse_range(Some("[10,19]".to_string()));
        assert_eq!(start, 10);
        assert_eq!(end, 19);

        let (start, end) = parse_range(Some("[50,74]".to_string()));
        assert_eq!(start, 50);
        assert_eq!(end, 74);
    }

    /// Test parse_range with invalid JSON returns default
    #[test]
    fn test_parse_range_invalid_json() {
        let (start, end) = parse_range(Some("invalid".to_string()));
        assert_eq!(start, 0);
        assert_eq!(end, 9);

        let (start, end) = parse_range(Some("[0]".to_string())); // Not enough elements
        assert_eq!(start, 0);
        assert_eq!(end, 9);

        let (start, end) = parse_range(Some("[]".to_string())); // Empty array
        assert_eq!(start, 0);
        assert_eq!(end, 9);
    }

    /// Test parse_range with None returns default
    #[test]
    fn test_parse_range_none() {
        let (start, end) = parse_range(None);
        assert_eq!(start, 0);
        assert_eq!(end, 9);
    }

    /// Test default pagination when no params provided
    #[test]
    fn test_pagination_default_values() {
        let params = crate::models::FilterOptions::default();
        let (offset, limit) = parse_pagination(&params);

        assert_eq!(offset, 0, "Default offset should be 0");
        assert_eq!(limit, 10, "Default limit should be 10");
    }

    /// Test pagination with range format calculates limit correctly
    #[test]
    fn test_pagination_range_calculates_limit() {
        let params = crate::models::FilterOptions {
            range: Some("[0,4]".to_string()),
            ..Default::default()
        };
        let (offset, limit) = parse_pagination(&params);

        assert_eq!(offset, 0, "Offset should be 0");
        assert_eq!(limit, 5, "Limit should be 5 for range [0,4]");

        // Test second page
        let params = crate::models::FilterOptions {
            range: Some("[5,9]".to_string()),
            ..Default::default()
        };
        let (offset, limit) = parse_pagination(&params);

        assert_eq!(offset, 5, "Offset should be 5");
        assert_eq!(limit, 5, "Limit should be 5 for range [5,9]");
    }

    /// Test page/per_page takes priority over range
    #[test]
    fn test_pagination_page_priority_over_range() {
        let params = crate::models::FilterOptions {
            page: Some(2),
            per_page: Some(15),
            range: Some("[0,4]".to_string()), // Should be ignored
            ..Default::default()
        };
        let (offset, limit) = parse_pagination(&params);

        assert_eq!(offset, 15, "Offset should be 15 (page 2 * 15 per_page)");
        assert_eq!(limit, 15, "Limit should be 15");
    }

    /// Test range pagination enforces max limits
    #[test]
    fn test_pagination_range_enforces_max_limits() {
        // Test max page size enforcement
        let params = crate::models::FilterOptions {
            range: Some("[0,9999]".to_string()), // Requesting 10000 items
            ..Default::default()
        };
        let (_offset, limit) = parse_pagination(&params);
        assert!(
            limit <= MAX_PAGE_SIZE,
            "Range limit should be capped at {}",
            MAX_PAGE_SIZE
        );

        // Test max offset enforcement
        let params = crate::models::FilterOptions {
            range: Some("[9999999,10000000]".to_string()), // Very large offset
            ..Default::default()
        };
        let (offset, _limit) = parse_pagination(&params);
        assert!(
            offset <= MAX_OFFSET,
            "Range offset should be capped at {}",
            MAX_OFFSET
        );
    }

    /// A3 regression: a huge `end` in the range branch must not overflow the `+ 1`
    /// (which panics under overflow-checks). Should cap cleanly instead.
    #[test]
    fn test_pagination_range_huge_end_does_not_overflow() {
        let params = crate::models::FilterOptions {
            range: Some(format!("[0,{}]", u64::MAX)),
            ..Default::default()
        };
        let (offset, limit) = parse_pagination(&params);
        assert!(limit <= MAX_PAGE_SIZE, "limit must be capped, got {limit}");
        assert!(offset <= MAX_OFFSET, "offset must be capped, got {offset}");

        // Reversed range (end < start) must not panic either.
        let params = crate::models::FilterOptions {
            range: Some(format!("[{},{}]", u64::MAX, u64::MAX)),
            ..Default::default()
        };
        let (_offset, limit) = parse_pagination(&params);
        assert!(limit <= MAX_PAGE_SIZE);
    }
}

#[cfg(test)]
mod prop_tests {
    use super::*;
    use crate::filtering::joined::FilterOperator;
    use proptest::prelude::*;

    mod pe {
        use sea_orm::entity::prelude::*;

        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "pe_things")]
        pub struct Model {
            #[sea_orm(primary_key)]
            pub id: i32,
            pub name: String,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    }

    const OPS: [FilterOperator; 9] = [
        FilterOperator::Eq,
        FilterOperator::Neq,
        FilterOperator::Gt,
        FilterOperator::Gte,
        FilterOperator::Lt,
        FilterOperator::Lte,
        FilterOperator::Like,
        FilterOperator::In,
        FilterOperator::IsNull,
    ];

    fn json_value() -> impl Strategy<Value = serde_json::Value> {
        prop_oneof![
            any::<i64>().prop_map(|n| serde_json::json!(n)),
            // u64 covers values above i64::MAX, which must bind without a lossy f64 cast.
            any::<u64>().prop_map(|n| serde_json::json!(n)),
            any::<f64>().prop_map(|f| serde_json::json!(f)),
            any::<bool>().prop_map(|b| serde_json::json!(b)),
            "[a-zA-Z0-9 %_!.-]{0,24}".prop_map(|s| serde_json::json!(s)),
            proptest::collection::vec(any::<i64>(), 0..6).prop_map(|v| serde_json::json!(v)),
            proptest::collection::vec("[a-z]{0,6}", 0..6).prop_map(|v| serde_json::json!(v)),
            proptest::collection::vec(any::<bool>(), 0..6).prop_map(|v| serde_json::json!(v)),
            Just(serde_json::Value::Null),
        ]
    }

    proptest! {
        /// `build_comparison_expr` never panics for any operator/value combination on
        /// either an integer or a string column, and is deterministic (the same input
        /// yields the same Some/None outcome). This is the joined-filter path, fed by
        /// attacker-controlled `filter={...}` JSON.
        #[test]
        fn build_comparison_expr_never_panics(value in json_value()) {
            for op in OPS {
                let a = build_comparison_expr(pe::Column::Id, op, &value).is_some();
                let b = build_comparison_expr(pe::Column::Id, op, &value).is_some();
                prop_assert_eq!(a, b);
                let c = build_comparison_expr(pe::Column::Name, op, &value).is_some();
                let d = build_comparison_expr(pe::Column::Name, op, &value).is_some();
                prop_assert_eq!(c, d);
            }
        }

        /// Attacker-controlled string filter values are always bound as parameters,
        /// never spliced into the SQL text. Proven by rendering the parameterised form
        /// and checking the value rides a placeholder.
        #[test]
        fn build_comparison_expr_binds_string_values(s in "[a-z][a-zA-Z0-9 ';-]{0,23}") {
            use sea_orm::sea_query::{Query, SqliteQueryBuilder};
            let expr = build_comparison_expr(pe::Column::Name, FilterOperator::Eq, &serde_json::json!(s));
            prop_assert!(expr.is_some());
            let (sql, values) = Query::select()
                .column(pe::Column::Id)
                .from(pe::Entity)
                .and_where(expr.unwrap())
                .build(SqliteQueryBuilder);
            prop_assert!(sql.contains('?'), "value must ride a bound placeholder: {sql}");
            prop_assert_eq!(values.0.len(), 1);
        }

        /// REST page/per_page pagination always stays within the configured caps and
        /// never panics, even for `u64::MAX` inputs (overflow-checks are on in tests).
        #[test]
        fn parse_pagination_page_respects_caps(page in any::<u64>(), per_page in any::<u64>()) {
            let params = crate::models::FilterOptions {
                page: Some(page),
                per_page: Some(per_page),
                ..Default::default()
            };
            let (offset, limit) = parse_pagination(&params);
            prop_assert!(limit <= MAX_PAGE_SIZE);
            prop_assert!(offset <= MAX_OFFSET);
        }

        /// React-Admin `range=[start,end]` pagination stays within caps and never
        /// panics, including reversed ranges and `u64::MAX` bounds.
        #[test]
        fn parse_pagination_range_respects_caps(start in any::<u64>(), end in any::<u64>()) {
            let params = crate::models::FilterOptions {
                range: Some(format!("[{start},{end}]")),
                ..Default::default()
            };
            let (offset, limit) = parse_pagination(&params);
            prop_assert!(limit <= MAX_PAGE_SIZE);
            prop_assert!(offset <= MAX_OFFSET);
        }
    }
}
