use sea_orm::{
    Condition, DatabaseBackend,
    sea_query::{Alias, Expr, SimpleExpr},
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;

static FULLTEXT_WARNING_SHOWN: AtomicBool = AtomicBool::new(false);

// Basic safety limits
const MAX_SEARCH_QUERY_LENGTH: usize = 10_000;
const MAX_FIELD_VALUE_LENGTH: usize = 10_000;

/// Simple sanitization for search queries
fn sanitize_search_query(query: &str) -> String {
    if query.len() > MAX_SEARCH_QUERY_LENGTH {
        query[..MAX_SEARCH_QUERY_LENGTH].trim().to_string()
    } else {
        query.trim().to_string()
    }
}

/// Basic field name validation
fn is_valid_field_name(field_name: &str) -> bool {
    !field_name.is_empty()
        && field_name.len() <= 100
        && !field_name.starts_with('_')
        && !field_name.contains("..")
}

/// Basic value length check
const fn validate_field_value(value: &str) -> bool {
    value.len() <= MAX_FIELD_VALUE_LENGTH
}

/// Parse React Admin comparison operator suffixes
/// Returns (`base_field_name`, `sql_operator`) if a suffix is found
fn parse_comparison_operator(field_name: &str) -> Option<(&str, &str)> {
    field_name.strip_suffix("_gte").map_or_else(|| {
        field_name.strip_suffix("_lte").map_or_else(|| {
            field_name.strip_suffix("_gt").map_or_else(|| {
                field_name.strip_suffix("_lt").map_or_else(|| {
                    field_name
                        .strip_suffix("_neq")
                        .map(|base_field| (base_field, "!="))
                }, |base_field| Some((base_field, "<")))
            }, |base_field| Some((base_field, ">")))
        }, |base_field| Some((base_field, "<=")))
    }, |base_field| Some((base_field, ">=")))
}

/// Apply numeric comparison for integer values
fn apply_numeric_comparison(
    field_name: &str,
    operator: &str,
    value: i64,
) -> sea_orm::sea_query::SimpleExpr {
    let column = Expr::col(Alias::new(field_name));
    match operator {
        ">=" => column.gte(value),
        "<=" => column.lte(value),
        ">" => column.gt(value),
        "<" => column.lt(value),
        "!=" => column.ne(value),
        _ => column.eq(value), // fallback to equality
    }
}

/// Apply numeric comparison for float values
fn apply_float_comparison(
    field_name: &str,
    operator: &str,
    value: f64,
) -> sea_orm::sea_query::SimpleExpr {
    let column = Expr::col(Alias::new(field_name));
    match operator {
        ">=" => column.gte(value),
        "<=" => column.lte(value),
        ">" => column.gt(value),
        "<" => column.lt(value),
        "!=" => column.ne(value),
        _ => column.eq(value), // fallback to equality
    }
}

/// Build fulltext search condition with database-specific optimizations
fn build_fulltext_condition<T: crate::traits::CRUDResource>(
    query: &str,
    backend: DatabaseBackend,
) -> Option<SimpleExpr> {
    let fulltext_columns = T::fulltext_searchable_columns();

    if fulltext_columns.is_empty() {
        return None;
    }

    // Show warning once if using fallback on large datasets
    if fulltext_columns.len() > 3
        && backend != DatabaseBackend::Postgres
        && !FULLTEXT_WARNING_SHOWN.load(Ordering::Relaxed)
    {
        eprintln!(
            "Warning: Using inefficient fulltext search fallback for {} columns. Consider PostgreSQL for better performance.",
            fulltext_columns.len()
        );
        FULLTEXT_WARNING_SHOWN.store(true, Ordering::Relaxed);
    }

    match &backend {
        DatabaseBackend::Postgres => build_postgres_fulltext_condition(query, &fulltext_columns),
        _ => build_fallback_fulltext_condition(query, &fulltext_columns),
    }
}

/// Build PostgreSQL-specific fulltext search using trigrams with relevance scoring
fn build_postgres_fulltext_condition(
    query: &str,
    columns: &[(&'static str, impl sea_orm::ColumnTrait)],
) -> Option<SimpleExpr> {
    if columns.is_empty() || query.is_empty() {
        return None;
    }

    let mut concat_parts = Vec::new();

    for (name, _column) in columns {
        // COALESCE(column_name::text, '')
        concat_parts.push(format!("COALESCE({name}::text, '')"));
    }

    let concat_sql = concat_parts.join(" || ' ' || ");
    let sanitized_query = sanitize_search_query(query);
    let escaped_query = sanitized_query.replace('\'', "''");
    
    // Use a consistent approach: combine ILIKE for substring matching with trigram similarity for fuzzy matching
    // This ensures reliable partial matching across all query lengths
    let search_sql = format!(
        "(UPPER({concat_sql}) LIKE UPPER('%{escaped_query}%') OR SIMILARITY({concat_sql}, '{escaped_query}') > 0.1)"
    );

    // Use custom SQL expression
    Some(SimpleExpr::Custom(search_sql))
}

/// Build fallback fulltext search using LIKE concatenation for other databases
fn build_fallback_fulltext_condition(
    query: &str,
    columns: &[(&'static str, impl sea_orm::ColumnTrait)],
) -> Option<SimpleExpr> {
    if columns.is_empty() {
        return None;
    }

    // For SQLite and MySQL, use concatenation with LIKE
    let mut concat_parts = Vec::new();

    for (name, _column) in columns {
        concat_parts.push(format!("CAST({name} AS TEXT)"));
    }

    let concat_sql = concat_parts.join(" || ' ' || ");
    // Additional security: validate and sanitize query
    let sanitized_query = sanitize_search_query(query);
    let like_sql = format!(
        "UPPER({}) LIKE UPPER('%{}%')",
        concat_sql,
        sanitized_query
            .replace('\'', "''")
            .replace('%', "\\%")
            .replace('_', "\\_") // Escape SQL LIKE wildcards
    );

    // Use custom SQL expression
    Some(SimpleExpr::Custom(like_sql))
}

/// Build condition for string field with LIKE queries
fn build_like_condition<T: crate::traits::CRUDResource>(
    key: &str,
    trimmed_value: &str,
) -> SimpleExpr {
    if T::enum_case_sensitive() {
        // Case-sensitive substring matching
        Expr::col(Alias::new(key)).like(format!("%{trimmed_value}%"))
    } else {
        // Case-insensitive substring matching using UPPER()
        use sea_orm::sea_query::SimpleExpr;
        SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(
            Expr::col(Alias::new(key)),
        ))
        .like(format!("%{}%", trimmed_value.to_uppercase()))
    }
}

/// Build condition for exact matching with enum support
fn build_exact_condition<T: crate::traits::CRUDResource>(
    key: &str,
    trimmed_value: &str,
    backend: DatabaseBackend,
) -> SimpleExpr {
    // Check if the value is a UUID only for non-LIKE fields
    if let Ok(uuid) = Uuid::parse_str(trimmed_value) {
        return Expr::col(Alias::new(key)).eq(uuid);
    }
    
    if T::enum_case_sensitive() {
        build_case_sensitive_enum_condition::<T>(key, trimmed_value, backend)
    } else {
        build_case_insensitive_enum_condition::<T>(key, trimmed_value, backend)
    }
}

/// Build case-sensitive enum condition
fn build_case_sensitive_enum_condition<T: crate::traits::CRUDResource>(
    key: &str,
    trimmed_value: &str,
    backend: DatabaseBackend,
) -> SimpleExpr {
    if T::is_enum_field(key) {
        match backend {
            DatabaseBackend::Postgres => {
                // PostgreSQL supports CAST operations on enums
                Expr::expr(Expr::cast_as(
                    Expr::col(Alias::new(key)),
                    Alias::new("TEXT"),
                ))
                .eq(trimmed_value)
            }
            DatabaseBackend::MySql => {
                // MySQL: Use BINARY for case-sensitive comparison (enums stored as strings)
                use sea_orm::sea_query::Func;
                SimpleExpr::FunctionCall(Func::cust("BINARY").arg(Expr::col(Alias::new(key))))
                    .eq(trimmed_value)
            }
            DatabaseBackend::Sqlite => {
                // SQLite: direct string comparison (case-sensitive by default)
                Expr::col(Alias::new(key)).eq(trimmed_value)
            }
        }
    } else {
        // Regular exact matching for non-enum fields
        Expr::col(Alias::new(key)).eq(trimmed_value)
    }
}

/// Build case-insensitive enum condition
fn build_case_insensitive_enum_condition<T: crate::traits::CRUDResource>(
    key: &str,
    trimmed_value: &str,
    backend: DatabaseBackend,
) -> SimpleExpr {
    use sea_orm::sea_query::Func;
    
    if T::is_enum_field(key) {
        match backend {
            DatabaseBackend::Postgres => {
                // PostgreSQL: Cast enum to TEXT then apply UPPER()
                SimpleExpr::FunctionCall(Func::upper(Expr::cast_as(
                    Expr::col(Alias::new(key)),
                    Alias::new("TEXT"),
                )))
                .eq(trimmed_value.to_uppercase())
            }
            DatabaseBackend::MySql | DatabaseBackend::Sqlite => {
                // MySQL/SQLite: Apply UPPER() for case-insensitive matching
                SimpleExpr::FunctionCall(Func::upper(Expr::col(Alias::new(key))))
                    .eq(trimmed_value.to_uppercase())
            }
        }
    } else {
        // Regular case-insensitive matching for non-enum fields
        SimpleExpr::FunctionCall(Func::upper(Expr::col(Alias::new(key))))
            .eq(trimmed_value.to_uppercase())
    }
}

/// Build string condition based on field type
fn build_string_condition<T: crate::traits::CRUDResource>(
    key: &str,
    trimmed_value: &str,
    backend: DatabaseBackend,
) -> SimpleExpr {
    // Check if this field should use LIKE queries
    let use_like = T::like_filterable_columns().contains(&key);

    if use_like {
        build_like_condition::<T>(key, trimmed_value)
    } else {
        build_exact_condition::<T>(key, trimmed_value, backend)
    }
}

/// Build array condition for multiple values
fn build_array_condition(key: &str, value_array: &[serde_json::Value]) -> Condition {
    let mut or_conditions = Condition::any();
    
    if key == "ids" {
        // React Admin GetMany format: {"ids": [uuid1, uuid2, uuid3]}
        // Filter on the 'id' field for any of the provided UUIDs
        for id in value_array {
            if let Some(id_str) = id.as_str()
                && let Ok(uuid) = Uuid::parse_str(id_str) {
                    or_conditions = or_conditions.add(Expr::col(Alias::new("id")).eq(uuid));
                }
        }
    } else {
        // Regular array filtering for other fields
        for id in value_array {
            if let Some(id_str) = id.as_str()
                && let Ok(uuid) = Uuid::parse_str(id_str) {
                    or_conditions = or_conditions.add(Expr::col(Alias::new(key)).eq(uuid));
                }
        }
    }
    
    or_conditions
}

#[allow(clippy::too_many_lines)]
pub fn apply_filters<T: crate::traits::CRUDResource>(
    filter_str: Option<String>,
    searchable_columns: &[(&str, impl sea_orm::ColumnTrait)],
    backend: DatabaseBackend,
) -> Condition {
    // Simple approach: cast enum fields to TEXT for universal database compatibility
    // Parse the filter string into a HashMap
    let filters: HashMap<String, serde_json::Value> = filter_str.map_or_else(HashMap::new, |filter| {
        match serde_json::from_str(&filter) {
            Ok(parsed) => parsed,
            Err(e) => {
                eprintln!("Warning: Invalid JSON in filter string: {e}");
                HashMap::new()
            }
        }
    });

    let mut condition = Condition::all();
    // Check if there is a free-text search ("q") parameter
    if let Some(q_value) = filters.get("q") {
        if let Some(q_value_str) = q_value.as_str() {
            // Try fulltext search first
            if let Some(fulltext_condition) = build_fulltext_condition::<T>(q_value_str, backend) {
                condition = condition.add(fulltext_condition);
            } else {
                // Fallback to original LIKE search on regular searchable columns
                // Handle enum fields by casting to TEXT for PostgreSQL compatibility
                let mut or_conditions = Condition::any();
                for (col_name, col) in searchable_columns {
                    if T::is_enum_field(col_name) {
                        // Cast enum fields to TEXT for LIKE operations
                        match backend {
                            DatabaseBackend::Postgres => {
                                or_conditions = or_conditions.add(
                                    SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(
                                        Expr::cast_as(Expr::col(*col), Alias::new("TEXT"))
                                    ))
                                    .like(format!("%{}%", q_value_str.to_uppercase()))
                                );
                            }
                            _ => {
                                // For SQLite/MySQL, treat enum as string
                                or_conditions = or_conditions.add(
                                    SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(Expr::col(*col)))
                                        .like(format!("%{}%", q_value_str.to_uppercase()))
                                );
                            }
                        }
                    } else {
                        // Regular text fields
                        or_conditions = or_conditions.add(
                            SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(Expr::col(*col)))
                                .like(format!("%{}%", q_value_str.to_uppercase()))
                        );
                    }
                }
                condition = condition.add(or_conditions);
            }
        }
    } else {
        // Iterate over all filters to build conditions
        for (key, value) in filters {
            // Security validation: check field name
            if !is_valid_field_name(&key) {
                eprintln!("Warning: Invalid field name rejected: {key}");
                continue;
            }

            // Check if field exists in filterable columns (handle comparison operators and special cases)
            let base_field_name = if let Some((base_field, _)) = parse_comparison_operator(&key) {
                base_field
            } else if key.ends_with("_eq") {
                key.strip_suffix("_eq").unwrap_or(&key)
            } else {
                &key
            };

            let field_exists = key == "ids"
                || searchable_columns
                    .iter()
                    .any(|(col_name, _)| *col_name == base_field_name);
            if !field_exists {
                // Skip nonexistent fields - don't apply any filter condition
                continue;
            }
            if let Some(value_str) = value.as_str() {
                // Security validation: check field value length
                if !validate_field_value(value_str) {
                    eprintln!(
                        "Warning: Field value too long, rejected: {} chars",
                        value_str.len()
                    );
                    continue;
                }

                let trimmed_value = value_str.trim().to_string();

                // Handle empty strings
                if trimmed_value.is_empty() {
                    // For empty strings, match fields that are exactly empty
                    condition = condition.add(Expr::col(Alias::new(&*key)).eq(""));
                    continue;
                }

                // Handle React Admin string filtering patterns
                if let Some(base_field) = key.strip_suffix("_eq") {
                    // Exact string matching with _eq suffix: {"title_eq": "Exact Title"}
                    condition = condition.add(Expr::col(Alias::new(base_field)).eq(trimmed_value));
                } else {
                    condition = condition.add(build_string_condition::<T>(&key, &trimmed_value, backend));
                }
            } else if let Some(value_int) = value.as_i64() {
                // Handle numeric comparison operators for integers
                if let Some((base_field, operator)) = parse_comparison_operator(&key) {
                    condition =
                        condition.add(apply_numeric_comparison(base_field, operator, value_int));
                } else {
                    condition = condition.add(Expr::col(Alias::new(&*key)).eq(value_int));
                }
            } else if let Some(value_bool) = value.as_bool() {
                // Handle boolean comparison operators and regular boolean values
                if let Some((base_field, operator)) = parse_comparison_operator(&key) {
                    if operator == "!=" {
                        // Support boolean_neq for React Admin
                        condition = condition.add(Expr::col(Alias::new(base_field)).ne(value_bool));
                    } else {
                        // Other operators don't make sense for booleans, treat as regular
                        condition = condition.add(Expr::col(Alias::new(&*key)).eq(value_bool));
                    }
                } else {
                    condition = condition.add(Expr::col(Alias::new(&*key)).eq(value_bool));
                }
            } else if let Some(value_float) = value.as_f64() {
                // Handle numeric comparison operators for floats
                if let Some((base_field, operator)) = parse_comparison_operator(&key) {
                    condition =
                        condition.add(apply_float_comparison(base_field, operator, value_float));
                } else {
                    condition = condition.add(Expr::col(Alias::new(&*key)).eq(value_float));
                }
            } else if value.is_null() {
                // Handle null values for optional fields (no comparison operators for null)
                condition = condition.add(Expr::col(Alias::new(&*key)).is_null());
            } else if let Some(value_array) = value.as_array() {
                condition = condition.add(build_array_condition(&key, value_array));
            }
        }
    }

    condition
}

#[must_use]
pub fn parse_range(range_str: Option<String>) -> (u64, u64) {
    range_str.map_or((0, 10), |range| {
        let range_vec: Vec<u64> = serde_json::from_str(&range).unwrap_or_else(|_| vec![0, 24]);
        let start = range_vec.first().copied().unwrap_or(0);
        let end = range_vec.get(1).copied().unwrap_or(24);
        let limit = end - start + 1;
        (start, limit)
    })
}

/// Parse pagination from `FilterOptions`, supporting both React Admin and standard REST formats
#[must_use]
pub fn parse_pagination(params: &crate::models::FilterOptions) -> (u64, u64) {
    // If ANY standard REST pagination parameters are provided, use them
    if params.page.is_some() || params.per_page.is_some() {
        let page = params.page.unwrap_or(0);
        let per_page = params.per_page.unwrap_or(10);
        let offset = page * per_page; // 0-based pagination
        (offset, per_page)
    }
    // Otherwise fall back to React Admin range format
    else {
        parse_range(params.range.clone())
    }
}
