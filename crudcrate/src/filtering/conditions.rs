use sea_orm::{
    Condition, DatabaseBackend,
    sea_query::{Alias, Expr, SimpleExpr},
};
use std::collections::HashMap;
use uuid::Uuid;

use super::search::build_fulltext_condition;

// Basic safety limits
const MAX_FIELD_VALUE_LENGTH: usize = 10_000;

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

/// Build condition for string field with LIKE queries (case-insensitive)
#[must_use] pub fn build_like_condition(key: &str, trimmed_value: &str) -> SimpleExpr {
    let escaped_value = trimmed_value.replace('\'', "''");
    let like_sql = format!("UPPER({key}) LIKE UPPER('%{escaped_value}%')");
    SimpleExpr::Custom(like_sql)
}

pub fn apply_filters<T: crate::traits::CRUDResource>(
    filter_str: Option<String>,
    searchable_columns: &[(&str, impl sea_orm::ColumnTrait)],
    backend: DatabaseBackend,
) -> Condition {
    // Parse the filter string into a HashMap
    let filters: HashMap<String, serde_json::Value> =
        filter_str.map_or_else(HashMap::new, |filter| match serde_json::from_str(&filter) {
            Ok(parsed) => parsed,
            Err(e) => {
                eprintln!("Warning: Invalid JSON in filter string: {e}");
                HashMap::new()
            }
        });

    let mut condition = Condition::all();
    
    // Check if there is a free-text search ("q") parameter
    if let Some(q_value) = filters.get("q")
        && let Some(q_value_str) = q_value.as_str() {
            // Try fulltext search first
            if let Some(fulltext_condition) = build_fulltext_condition::<T>(q_value_str, backend) {
                condition = condition.add(fulltext_condition);
            } else {
                // Fallback to original LIKE search on regular searchable columns
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
                                    .like(format!("%{}%", q_value_str.to_uppercase())),
                                );
                            }
                            _ => {
                                // For SQLite/MySQL, treat enum as string
                                or_conditions = or_conditions.add(
                                    SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(
                                        Expr::col(*col),
                                    ))
                                    .like(format!("%{}%", q_value_str.to_uppercase())),
                                );
                            }
                        }
                    } else {
                        // Regular string columns
                        or_conditions = or_conditions.add(
                            SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(
                                Expr::col(*col),
                            ))
                            .like(format!("%{}%", q_value_str.to_uppercase())),
                        );
                    }
                }
                condition = condition.add(or_conditions);
            }
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
            match value {
                serde_json::Value::String(string_value) => {
                    if !validate_field_value(string_value) {
                        continue;
                    }

                    let trimmed_value = string_value.trim();
                    if trimmed_value.is_empty() {
                        continue;
                    }

                    // Check if this field should use LIKE queries
                    if T::like_filterable_columns().contains(&key.as_str()) {
                        condition = condition.add(build_like_condition(key, trimmed_value));
                    } else if T::is_enum_field(key) {
                        // Handle enum fields with case-insensitive matching
                        let enum_condition = match backend {
                            DatabaseBackend::Postgres => {
                                SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(
                                    Expr::cast_as(Expr::col(*column), Alias::new("TEXT")),
                                ))
                                .eq(trimmed_value.to_uppercase())
                            }
                            _ => {
                                // For SQLite/MySQL
                                SimpleExpr::FunctionCall(sea_orm::sea_query::Func::upper(
                                    Expr::col(*column),
                                ))
                                .eq(trimmed_value.to_uppercase())
                            }
                        };
                        condition = condition.add(enum_condition);
                    } else {
                        // Try to parse as UUID first
                        if let Ok(uuid_value) = Uuid::parse_str(trimmed_value) {
                            condition = condition.add(Expr::col(*column).eq(uuid_value));
                        } else {
                            // Case-insensitive string equality
                            let string_condition = SimpleExpr::FunctionCall(
                                sea_orm::sea_query::Func::upper(Expr::col(*column)),
                            )
                            .eq(trimmed_value.to_uppercase());
                            condition = condition.add(string_condition);
                        }
                    }
                }
                serde_json::Value::Number(number) => {
                    if let Some((base_field, operator)) = parse_comparison_operator(key) {
                        // Check if the base field exists in searchable columns
                        if searchable_columns
                            .iter()
                            .any(|(col_name, _)| *col_name == base_field)
                        {
                            if let Some(int_value) = number.as_i64() {
                                condition = condition.add(apply_numeric_comparison(
                                    base_field, operator, int_value,
                                ));
                            } else if let Some(float_value) = number.as_f64() {
                                condition = condition.add(apply_float_comparison(
                                    base_field, operator, float_value,
                                ));
                            }
                        }
                    } else {
                        // Regular number equality
                        if let Some(int_value) = number.as_i64() {
                            condition = condition.add(Expr::col(*column).eq(int_value));
                        } else if let Some(float_value) = number.as_f64() {
                            condition = condition.add(Expr::col(*column).eq(float_value));
                        }
                    }
                }
                serde_json::Value::Bool(bool_value) => {
                    condition = condition.add(Expr::col(*column).eq(*bool_value));
                }
                serde_json::Value::Array(array_values) => {
                    // Handle IN operations for arrays
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
                        condition = condition.add(Expr::col(*column).is_in(in_values));
                    }
                }
                serde_json::Value::Null => {
                    condition = condition.add(Expr::col(*column).is_null());
                }
                _ => {
                    // Skip unsupported value types
                }
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
        let offset = (page.saturating_sub(1)) * per_page;
        (offset, per_page)
    } else if let Some(range) = &params.range {
        // React Admin pagination
        let (start, end) = parse_range(Some(range.clone()));
        let limit = end.saturating_sub(start) + 1;
        (start, limit)
    } else {
        // Default pagination
        (0, 10)
    }
}