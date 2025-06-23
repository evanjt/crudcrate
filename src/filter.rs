use sea_orm::{
    sea_query::{extension::postgres::PgExpr, Alias, Expr},
    Condition, ColumnTrait, ColumnType,
};
use std::collections::HashMap;
use uuid::Uuid;

/// Check if a column type is likely an enum by examining its type name
fn is_enum_column<C: ColumnTrait>(column: &C) -> bool {
    let column_def = column.def();
    let column_type = column_def.get_column_type();
    
    // Add some debug logging to see what we're getting
    eprintln!("DEBUG: Column type: {:?}", column_type);
    
    match column_type {
        // Standard SQL types that are definitely NOT enums
        ColumnType::Char(_) | 
        ColumnType::String(_) | 
        ColumnType::Text |
        ColumnType::TinyInteger |
        ColumnType::SmallInteger |
        ColumnType::Integer |
        ColumnType::BigInteger |
        ColumnType::TinyUnsigned |
        ColumnType::SmallUnsigned |
        ColumnType::Unsigned |
        ColumnType::BigUnsigned |
        ColumnType::Float |
        ColumnType::Double |
        ColumnType::Decimal(_) |
        ColumnType::DateTime |
        ColumnType::Timestamp |
        ColumnType::TimestampWithTimeZone |
        ColumnType::Time |
        ColumnType::Date |
        ColumnType::Year |
        ColumnType::Interval(_, _) |
        ColumnType::Binary(_) |
        ColumnType::VarBinary(_) |
        ColumnType::Bit(_) |
        ColumnType::VarBit(_) |
        ColumnType::Blob |
        ColumnType::Boolean |
        ColumnType::Money(_) |
        ColumnType::Json |
        ColumnType::JsonBinary |
        ColumnType::Uuid |
        ColumnType::Array(_) |
        ColumnType::Cidr |
        ColumnType::Inet |
        ColumnType::MacAddr => {
            eprintln!("DEBUG: Identified as standard type, NOT enum");
            false
        },
        // Custom types are likely enums
        ColumnType::Custom(type_name) => {
            eprintln!("DEBUG: Custom type '{:?}' - treating as enum", type_name);
            true
        },
        // Enum types
        ColumnType::Enum { .. } => {
            eprintln!("DEBUG: Explicit enum type - treating as enum");
            true
        },
        // Any other type we don't recognize - be conservative and treat as enum
        _ => {
            eprintln!("DEBUG: Unknown type - treating as enum");
            true
        }
    }
}

pub fn apply_filters(
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
                    or_conditions.add(Expr::col(*col).ilike(format!("%{q_value_str}%")));
            }
            condition = condition.add(or_conditions);
        }
    } else {
        // Iterate over all filters to build conditions
        for (key, value) in filters {
            if let Some(value_str) = value.as_str() {
                let trimmed_value = value_str.trim().to_string();

                // Check if the value is a UUID
                if let Ok(uuid) = Uuid::parse_str(&trimmed_value) {
                    condition = condition.add(Expr::col(Alias::new(&*key)).eq(uuid));
                } else {
                    // Check if this column is an enum by looking up its type
                    let column_info = searchable_columns
                        .iter()
                        .find(|(col_name, _)| *col_name == key);
                    
                    if let Some((_, col)) = column_info {
                        let is_enum = is_enum_column(col);
                        eprintln!("DEBUG: Column '{}' is_enum: {}", key, is_enum);
                        
                        if is_enum {
                            // Use exact matching for enum columns, but cast the enum to text for comparison
                            eprintln!("DEBUG: Using exact match for column '{}' with text cast", key);
                            condition = condition.add(Expr::col(Alias::new(&*key)).cast_as(Alias::new("text")).eq(trimmed_value));
                        } else {
                            // Use ILIKE for text columns
                            eprintln!("DEBUG: Using ILIKE for column '{}'", key);
                            condition = condition
                                .add(Expr::col(Alias::new(&*key)).ilike(format!("%{trimmed_value}%")));
                        }
                    } else {
                        eprintln!("DEBUG: Column '{}' not found in searchable_columns, using ILIKE", key);
                        // Column not found in searchable columns, default to ILIKE
                        condition = condition
                            .add(Expr::col(Alias::new(&*key)).ilike(format!("%{trimmed_value}%")));
                    }
                }
            } else if let Some(value_int) = value.as_i64() {
                condition = condition.add(Expr::col(Alias::new(&*key)).eq(value_int));
            } else if let Some(value_array) = value.as_array() {
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
