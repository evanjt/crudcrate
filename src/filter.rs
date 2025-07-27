use sea_orm::{
    sea_query::{Alias, Expr},
    Condition,
};
use std::collections::HashMap;
use uuid::Uuid;

/// Determine if a field should use substring matching (for text search) or exact matching
fn is_text_field(field_name: &str) -> bool {
    // Common text field patterns that should support substring search
    matches!(field_name, 
        "title" | "name" | "description" | "content" | "text" | "message" | 
        "comment" | "note" | "summary" | "body" | "bio" | "about" |
        "address" | "location" | "email" | "username" | "first_name" | "last_name"
    ) || field_name.ends_with("_name") 
      || field_name.ends_with("_title") 
      || field_name.ends_with("_description")
      || field_name.ends_with("_text")
      || field_name.ends_with("_content")
}

/// Parse React Admin comparison operator suffixes
/// Returns (base_field_name, sql_operator) if a suffix is found
fn parse_comparison_operator(field_name: &str) -> Option<(&str, &str)> {
    if let Some(base_field) = field_name.strip_suffix("_gte") {
        Some((base_field, ">="))
    } else if let Some(base_field) = field_name.strip_suffix("_lte") {
        Some((base_field, "<="))
    } else if let Some(base_field) = field_name.strip_suffix("_gt") {
        Some((base_field, ">"))
    } else if let Some(base_field) = field_name.strip_suffix("_lt") {
        Some((base_field, "<"))
    } else if let Some(base_field) = field_name.strip_suffix("_neq") {
        Some((base_field, "!="))
    } else {
        None
    }
}

/// Apply numeric comparison for integer values
fn apply_numeric_comparison(field_name: &str, operator: &str, value: i64) -> sea_orm::sea_query::SimpleExpr {
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
fn apply_float_comparison(field_name: &str, operator: &str, value: f64) -> sea_orm::sea_query::SimpleExpr {
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


pub fn apply_filters<T: crate::traits::CRUDResource>(
    filter_str: Option<String>,
    searchable_columns: &[(&str, impl sea_orm::ColumnTrait)],
) -> Condition {
    // Parse the filter string into a HashMap
    let filters: HashMap<String, serde_json::Value> = if let Some(filter) = filter_str {
        serde_json::from_str(&filter).unwrap_or_default()
    } else {
        HashMap::new()
    };

    let mut condition = Condition::all();
    // Check if there is a free-text search ("q") parameter
    if let Some(q_value) = filters.get("q") {
        if let Some(q_value_str) = q_value.as_str() {
            let mut or_conditions = Condition::any();
            for (_col_name, col) in searchable_columns {
                or_conditions =
                    or_conditions.add(Expr::col(*col).like(format!("%{q_value_str}%")));
            }
            condition = condition.add(or_conditions);
        }
    } else {
        // Iterate over all filters to build conditions
        for (key, value) in filters {
            // Check if field exists in filterable columns (handle comparison operators and special cases)
            let base_field_name = if let Some((base_field, _)) = parse_comparison_operator(&key) {
                base_field
            } else if key.ends_with("_eq") {
                key.strip_suffix("_eq").unwrap_or(&key)
            } else {
                &key
            };
            
            let field_exists = key == "ids" || searchable_columns.iter().any(|(col_name, _)| *col_name == base_field_name);
            if !field_exists {
                // Skip nonexistent fields - don't apply any filter condition
                continue;
            }
            if let Some(value_str) = value.as_str() {
                let trimmed_value = value_str.trim().to_string();
                
                // Handle empty strings
                if trimmed_value.is_empty() {
                    // For empty strings, match fields that are exactly empty
                    condition = condition.add(Expr::col(Alias::new(&*key)).eq(""));
                    continue;
                }

                // Check if the value is a UUID
                if let Ok(uuid) = Uuid::parse_str(&trimmed_value) {
                    condition = condition.add(Expr::col(Alias::new(&*key)).eq(uuid));
                } else {
                    // Handle React Admin string filtering patterns
                    if let Some(base_field) = key.strip_suffix("_eq") {
                        // Exact string matching with _eq suffix: {"title_eq": "Exact Title"}
                        condition = condition.add(Expr::col(Alias::new(base_field)).eq(trimmed_value));
                    } else if is_text_field(&key) {
                        // Default substring matching for text fields: {"title": "Alpha"}
                        condition = condition.add(Expr::col(Alias::new(&*key)).like(format!("%{}%", trimmed_value)));
                    } else {
                        // Enum and other field matching with configurable case sensitivity
                        if T::enum_case_sensitive() {
                            // Case-sensitive exact matching: {"priority": "High"} matches only "High"
                            condition = condition.add(Expr::col(Alias::new(&*key)).eq(trimmed_value));
                        } else {
                            // Case-insensitive matching: {"priority": "high"} matches "High"
                            // Use UPPER() function for case-insensitive comparison
                            use sea_orm::sea_query::SimpleExpr;
                            condition = condition.add(
                                SimpleExpr::FunctionCall(
                                    sea_orm::sea_query::Func::upper(Expr::col(Alias::new(&*key)))
                                ).eq(trimmed_value.to_uppercase())
                            );
                        }
                    }
                }
            } else if let Some(value_int) = value.as_i64() {
                // Handle numeric comparison operators for integers
                if let Some((base_field, operator)) = parse_comparison_operator(&key) {
                    condition = condition.add(apply_numeric_comparison(base_field, operator, value_int));
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
                    condition = condition.add(apply_float_comparison(base_field, operator, value_float));
                } else {
                    condition = condition.add(Expr::col(Alias::new(&*key)).eq(value_float));
                }
            } else if value.is_null() {
                // Handle null values for optional fields (no comparison operators for null)
                condition = condition.add(Expr::col(Alias::new(&*key)).is_null());
            } else if let Some(value_array) = value.as_array() {
                if key == "ids" {
                    // React Admin GetMany format: {"ids": [uuid1, uuid2, uuid3]}
                    // Filter on the 'id' field for any of the provided UUIDs
                    let mut or_conditions = Condition::any();
                    for id in value_array {
                        if let Some(id_str) = id.as_str() {
                            if let Ok(uuid) = Uuid::parse_str(id_str) {
                                or_conditions = or_conditions.add(Expr::col(Alias::new("id")).eq(uuid));
                            }
                        }
                    }
                    condition = condition.add(or_conditions);
                } else {
                    // Regular array filtering for other fields
                    let mut or_conditions = Condition::any();
                    for id in value_array {
                        if let Some(id_str) = id.as_str() {
                            if let Ok(uuid) = Uuid::parse_str(id_str) {
                                or_conditions =
                                    or_conditions.add(Expr::col(Alias::new(&*key)).eq(uuid));
                            }
                        }
                    }
                    condition = condition.add(or_conditions);
                }
            }
        }
    }

    condition
}

#[must_use]
pub fn parse_range(range_str: Option<String>) -> (u64, u64) {
    if let Some(range) = range_str {
        let range_vec: Vec<u64> = serde_json::from_str(&range).unwrap_or(vec![0, 24]);
        let start = range_vec.first().copied().unwrap_or(0);
        let end = range_vec.get(1).copied().unwrap_or(24);
        let limit = end - start + 1;
        (start, limit)
    } else {
        (0, 25)
    }
}
