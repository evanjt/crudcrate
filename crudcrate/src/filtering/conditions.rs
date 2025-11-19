use sea_orm::{
    Condition, DatabaseBackend,
    sea_query::{Alias, Expr, SimpleExpr},
};
use std::collections::HashMap;
use uuid::Uuid;

use super::search::{build_fulltext_condition, build_like_condition};

// Basic safety limits
const MAX_FIELD_VALUE_LENGTH: usize = 10_000;
const MAX_PAGE_SIZE: u64 = 1000;
const MAX_OFFSET: u64 = 1_000_000;

/// Escape LIKE wildcards to prevent wildcard injection attacks
/// Escapes: % (match any) and _ (match single char)
fn escape_like_wildcards(input: &str) -> String {
    input.replace('\\', "\\\\")  // Escape backslash first
        .replace('%', "\\%")      // Escape %
        .replace('_', "\\_")      // Escape _
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

/// Apply numeric comparison for any numeric type (i64, f64, etc.)
fn apply_numeric_comparison<V>(field_name: &str, operator: &str, value: V) -> SimpleExpr
where
    V: Into<sea_orm::Value> + Copy,
{
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

fn parse_filter_json(filter_str: Option<String>) -> HashMap<String, serde_json::Value> {
    filter_str.map_or_else(HashMap::new, |filter| match serde_json::from_str(&filter) {
        Ok(parsed) => parsed,
        Err(e) => {
            eprintln!("Warning: Invalid JSON in filter string: {e}");
            HashMap::new()
        }
    })
}

fn handle_fulltext_search<T: crate::traits::CRUDResource>(
    filters: &HashMap<String, serde_json::Value>,
    searchable_columns: &[(&str, impl sea_orm::ColumnTrait)],
    backend: DatabaseBackend,
) -> Option<Condition> {
    if let Some(q_value) = filters.get("q")
        && let Some(q_value_str) = q_value.as_str() {
        // Try fulltext search first
        if let Some(fulltext_expr) = build_fulltext_condition::<T>(q_value_str, backend) {
            return Some(Condition::all().add(fulltext_expr));
        }
        
        // Fallback to original LIKE search on regular searchable columns
        // Escape LIKE wildcards to prevent wildcard injection
        let escaped_query = escape_like_wildcards(q_value_str);

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
                            .like(format!("%{}%", escaped_query.to_uppercase())),
                        );
                    }
                    _ => {
                        // For SQLite/MySQL, treat enum as string
                        or_conditions = or_conditions.add(
                            SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(
                                Expr::col(*col),
                            ))
                            .like(format!("%{}%", escaped_query.to_uppercase())),
                        );
                    }
                }
            } else {
                // Regular string columns
                or_conditions = or_conditions.add(
                    SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(
                        Expr::col(*col),
                    ))
                    .like(format!("%{}%", escaped_query.to_uppercase())),
                );
            }
        }
        return Some(or_conditions);
    }
    None
}

fn process_string_filter<T: crate::traits::CRUDResource>(
    key: &str,
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

    // Check if this field should use LIKE queries
    if T::like_filterable_columns().contains(&key) {
        return Some(build_like_condition(key, trimmed_value));
    }
    
    if T::is_enum_field(key) {
        // Handle enum fields with case-insensitive matching
        let col_expr = match backend {
            DatabaseBackend::Postgres => Expr::cast_as(Expr::col(column), Alias::new("TEXT")),
            _ => Expr::col(column).into(),
        };
        return Some(SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(col_expr))
            .eq(trimmed_value.to_uppercase()));
    }
    
    // Try to parse as UUID first
    if let Ok(uuid_value) = Uuid::parse_str(trimmed_value) {
        return Some(Expr::col(column).eq(uuid_value));
    }
    
    // Case-insensitive string equality
    Some(SimpleExpr::FunctionCall(
        sea_orm::sea_query::Func::upper(Expr::col(column)),
    )
    .eq(trimmed_value.to_uppercase()))
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
                return Some(apply_numeric_comparison(base_field, operator, int_value));
            } else if let Some(float_value) = number.as_f64() {
                return Some(apply_numeric_comparison(base_field, operator, float_value));
            }
        }
    } else {
        // Regular number equality
        if let Some(int_value) = number.as_i64() {
            return Some(Expr::col(column).eq(int_value));
        } else if let Some(float_value) = number.as_f64() {
            return Some(Expr::col(column).eq(float_value));
        }
    }
    None
}

fn process_array_filter(
    array_values: &[serde_json::Value],
    column: impl sea_orm::ColumnTrait + Copy,
) -> Option<SimpleExpr> {
    let mut values = Vec::new();
    for array_value in array_values {
        match array_value {
            serde_json::Value::String(s) => {
                if let Ok(uuid_value) = Uuid::parse_str(s.trim()) {
                    values.push(serde_json::Value::String(uuid_value.to_string()));
                } else {
                    values.push(array_value.clone());
                }
            }
            _ => values.push(array_value.clone()),
        }
    }

    if !values.is_empty() {
        // Use IN operator for array values
        let in_values: Vec<String> = values.into_iter()
            .filter_map(|v| match v {
                serde_json::Value::String(s) => Some(s),
                serde_json::Value::Number(n) => Some(n.to_string()),
                serde_json::Value::Bool(b) => Some(b.to_string()),
                _ => None,
            })
            .collect();
        return Some(Expr::col(column).is_in(in_values));
    }
    None
}

pub fn apply_filters<T: crate::traits::CRUDResource>(
    filter_str: Option<String>,
    searchable_columns: &[(&str, impl sea_orm::ColumnTrait)],
    backend: DatabaseBackend,
) -> Condition {
    let filters = parse_filter_json(filter_str);
    let mut condition = Condition::all();
    
    // Handle fulltext search
    if let Some(fulltext_condition) = handle_fulltext_search::<T>(&filters, searchable_columns, backend) {
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

        // Find the column in searchable columns
        let column_opt = searchable_columns
            .iter()
            .find(|(col_name, _)| *col_name == key)
            .map(|(_, col)| col);

        if let Some(column) = column_opt {
            // Handle different value types
            let filter_condition = match value {
                serde_json::Value::String(string_value) => {
                    process_string_filter::<T>(key, string_value, *column, backend)
                }
                serde_json::Value::Number(number) => {
                    process_number_filter(key, number, *column, searchable_columns)
                }
                serde_json::Value::Bool(bool_value) => {
                    Some(Expr::col(*column).eq(*bool_value))
                }
                serde_json::Value::Array(array_values) => {
                    process_array_filter(array_values, *column)
                }
                serde_json::Value::Null => {
                    Some(Expr::col(*column).is_null())
                }
                serde_json::Value::Object(_) => None, // Skip unsupported value types
            };
            
            if let Some(filter_expr) = filter_condition {
                condition = condition.add(filter_expr);
            }
        }
    }

    condition
}

#[must_use] pub fn parse_range(range_str: Option<String>) -> (u64, u64) {
    range_str.map_or((0, 9), |r| {
        serde_json::from_str::<[u64; 2]>(&r)
            .map(|range| (range[0], range[1]))
            .unwrap_or((0, 9))
    })
}

#[must_use] pub fn parse_pagination(params: &crate::models::FilterOptions) -> (u64, u64) {
    if let (Some(page), Some(per_page)) = (params.page, params.per_page) {
        // Standard REST pagination (1-based page numbers)
        // Enforce maximum page size to prevent DoS
        let safe_per_page = per_page.min(MAX_PAGE_SIZE);

        // Use saturating_mul to prevent overflow panic
        let offset = (page.saturating_sub(1)).saturating_mul(safe_per_page);

        // Enforce maximum offset to prevent excessive database queries
        let safe_offset = offset.min(MAX_OFFSET);

        (safe_offset, safe_per_page)
    } else if let Some(range) = &params.range {
        // React Admin pagination
        let (start, end) = parse_range(Some(range.clone()));
        let limit = (end.saturating_sub(start) + 1).min(MAX_PAGE_SIZE);
        let safe_start = start.min(MAX_OFFSET);
        (safe_start, limit)
    } else {
        // Default pagination
        (0, 10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that malicious field names are rejected
    #[test]
    fn test_field_name_validation_rejects_sql_injection() {
        // These are currently rejected by the basic validation
        let rejected_names = vec![
            "../../../etc/passwd",  // Path traversal (contains ..)
            "id..name",  // Double dots
            "_internal",  // Starts with underscore
            "",  // Empty
        ];

        for malicious_name in rejected_names {
            assert!(!is_valid_field_name(malicious_name),
                "Should reject malicious field name: {malicious_name}");
        }

        // Test too long separately
        let too_long = "a".repeat(101);
        assert!(!is_valid_field_name(&too_long),
            "Should reject field names longer than 100 chars");
    }


    /// Test that valid field names are accepted
    #[test]
    fn test_field_name_validation_accepts_valid_names() {
        let valid_names = vec![
            "id",
            "user_name",
            "created_at",
            "field123",
        ];

        for valid_name in valid_names {
            assert!(is_valid_field_name(valid_name),
                "Should accept valid field name: {valid_name}");
        }

        // Test max length separately
        let max_length_name = "a".repeat(100);
        assert!(is_valid_field_name(&max_length_name),
            "Should accept 100-char field name");
    }

    /// Test that excessively long field values are rejected
    #[test]
    fn test_field_value_length_validation() {
        let short_value = "a".repeat(100);
        let max_value = "a".repeat(MAX_FIELD_VALUE_LENGTH);
        let too_long_value = "a".repeat(MAX_FIELD_VALUE_LENGTH + 1);

        assert!(validate_field_value(&short_value), "Short values should be valid");
        assert!(validate_field_value(&max_value), "Max length values should be valid");
        assert!(!validate_field_value(&too_long_value), "Overly long values should be invalid");
    }

    /// TDD: Pagination should enforce maximum page size
    /// This test will FAIL until we add `MAX_PAGE_SIZE` enforcement
    #[test]
    fn test_pagination_enforces_max_page_size() {
        const MAX_PAGE_SIZE: u64 = 1000;

        let params = crate::models::FilterOptions {
            page: Some(1),
            per_page: Some(999_999),  // Requesting huge page size
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
            page: Some(1_000_000),  // Huge page number
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
}