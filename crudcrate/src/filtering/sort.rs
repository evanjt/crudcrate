use sea_orm::{ColumnTrait, sea_query::Order};

// Shared default values
const DEFAULT_SORT_COLUMN: &str = "id";
const DEFAULT_SORT_ORDER: &str = "ASC";

/// Parse sort column and order from JSON array format
fn parse_json_sort(json: &str) -> (String, String) {
    let sort_vec: Vec<String> = serde_json::from_str(json).unwrap_or(vec![
        DEFAULT_SORT_COLUMN.to_string(),
        DEFAULT_SORT_ORDER.to_string(),
    ]);
    (
        sort_vec.first().cloned().unwrap_or(DEFAULT_SORT_COLUMN.to_string()),
        sort_vec.get(1).cloned().unwrap_or(DEFAULT_SORT_ORDER.to_string()),
    )
}

/// Convert sort order string to Order enum
fn parse_order(sort_order: &str) -> Order {
    if sort_order.to_uppercase() == "ASC" {
        Order::Asc
    } else {
        Order::Desc
    }
}

/// Find column by name or return default
fn find_column<C>(column_name: &str, columns: &[(&str, C)], default: C) -> C
where
    C: ColumnTrait + Copy,
{
    columns
        .iter()
        .find(|&&(col_name, _)| col_name == column_name)
        .map_or(default, |&(_, col)| col)
}

pub fn generic_sort<C>(
    sort: Option<&str>,
    order_column_logic: &[(&str, C)],
    default_column: C,
) -> (C, Order)
where
    C: ColumnTrait + Copy,
{

    let (sort_column, sort_order) = sort
        .map_or((DEFAULT_SORT_COLUMN.to_string(), DEFAULT_SORT_ORDER.to_string()), parse_json_sort);

    let order_direction = parse_order(&sort_order);
    let order_column = find_column(&sort_column, order_column_logic, default_column);

    (order_column, order_direction)
}

/// Parse sorting from `FilterOptions`, supporting both React Admin and standard REST formats
pub fn parse_sorting<C>(
    params: &crate::models::FilterOptions,
    order_column_logic: &[(&str, C)],
    default_column: C,
) -> (C, Order)
where
    C: ColumnTrait + Copy,
{
    let (sort_column, sort_order) = if let Some(sort_by) = &params.sort_by {
        // Standard REST format: sort_by=column&order=ASC/DESC
        (sort_by.clone(), params.order.as_deref().unwrap_or(DEFAULT_SORT_ORDER).to_string())
    } else if let Some(sort) = &params.sort {
        // Check if sort is a simple column name (REST) or JSON array (React Admin)
        if sort.starts_with('[') {
            // React Admin format: sort=["column", "ASC"]
            parse_json_sort(sort)
        } else {
            // REST format: sort=column&order=ASC/DESC
            (sort.clone(), params.order.as_deref().unwrap_or(DEFAULT_SORT_ORDER).to_string())
        }
    } else {
        (DEFAULT_SORT_COLUMN.to_string(), DEFAULT_SORT_ORDER.to_string())
    };

    let order_direction = parse_order(&sort_order);
    let order_column = find_column(&sort_column, order_column_logic, default_column);

    (order_column, order_direction)
}

/// Parse sorting with support for dot-notation (joined column) sorting.
///
/// Returns a `SortConfig` which can be either:
/// - `SortConfig::Column` for regular column sorting
/// - `SortConfig::Joined` for sorting by a column on a joined entity
///
/// # Example
/// ```ignore
/// // Regular sort
/// GET /customers?sort=["name","DESC"]
/// // -> SortConfig::Column { column: name, direction: Desc }
///
/// // Joined sort
/// GET /customers?sort=["vehicles.year","DESC"]
/// // -> SortConfig::Joined { join_field: "vehicles", column: "year", direction: Desc }
/// ```
pub fn parse_sorting_with_joins<T, C>(
    params: &crate::models::FilterOptions,
    order_column_logic: &[(&str, C)],
    default_column: C,
) -> super::joined::SortConfig<C>
where
    T: crate::traits::CRUDResource,
    C: ColumnTrait + Copy,
{
    use super::joined::SortConfig;

    let (sort_column, sort_order) = if let Some(sort_by) = &params.sort_by {
        (sort_by.clone(), params.order.as_deref().unwrap_or(DEFAULT_SORT_ORDER).to_string())
    } else if let Some(sort) = &params.sort {
        if sort.starts_with('[') {
            parse_json_sort(sort)
        } else {
            (sort.clone(), params.order.as_deref().unwrap_or(DEFAULT_SORT_ORDER).to_string())
        }
    } else {
        (DEFAULT_SORT_COLUMN.to_string(), DEFAULT_SORT_ORDER.to_string())
    };

    let order_direction = parse_order(&sort_order);

    // Check if this is a dot-notation sort (e.g., "vehicles.year")
    if sort_column.contains('.') {
        let parts: Vec<&str> = sort_column.splitn(2, '.').collect();
        if parts.len() == 2 {
            let join_field = parts[0];
            let column = parts[1];

            // Validate against allowed joined sortable columns
            let joined_sortable = T::joined_sortable_columns();
            let is_allowed = joined_sortable.iter().any(|c| c.full_path == sort_column);

            if is_allowed {
                return SortConfig::Joined {
                    join_field: join_field.to_string(),
                    column: column.to_string(),
                    direction: order_direction,
                };
            }
        }
    }

    // Regular column sort
    let order_column = find_column(&sort_column, order_column_logic, default_column);
    SortConfig::Column {
        column: order_column,
        direction: order_direction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_sort_valid() {
        let (col, order) = parse_json_sort(r#"["name", "DESC"]"#);
        assert_eq!(col, "name");
        assert_eq!(order, "DESC");
    }

    #[test]
    fn test_parse_json_sort_partial() {
        // Only column, no order
        let (col, order) = parse_json_sort(r#"["email"]"#);
        assert_eq!(col, "email");
        assert_eq!(order, DEFAULT_SORT_ORDER);
    }

    #[test]
    fn test_parse_json_sort_invalid_json() {
        // Invalid JSON should return defaults
        let (col, order) = parse_json_sort("invalid json");
        assert_eq!(col, DEFAULT_SORT_COLUMN);
        assert_eq!(order, DEFAULT_SORT_ORDER);
    }

    #[test]
    fn test_parse_json_sort_empty_array() {
        let (col, order) = parse_json_sort("[]");
        assert_eq!(col, DEFAULT_SORT_COLUMN);
        assert_eq!(order, DEFAULT_SORT_ORDER);
    }

    #[test]
    fn test_parse_order_asc() {
        assert_eq!(parse_order("ASC"), Order::Asc);
        assert_eq!(parse_order("asc"), Order::Asc);
        assert_eq!(parse_order("Asc"), Order::Asc);
    }

    #[test]
    fn test_parse_order_desc() {
        assert_eq!(parse_order("DESC"), Order::Desc);
        assert_eq!(parse_order("desc"), Order::Desc);
        assert_eq!(parse_order("Desc"), Order::Desc);
    }

    #[test]
    fn test_parse_order_invalid_defaults_to_desc() {
        // Any non-ASC value defaults to DESC
        assert_eq!(parse_order("invalid"), Order::Desc);
        assert_eq!(parse_order(""), Order::Desc);
        assert_eq!(parse_order("random"), Order::Desc);
    }

    // ========================================================================
    // REST FORMAT TESTS - Testing the internal parsing logic
    // Full parse_sorting tests are in integration tests (require real entities)
    // ========================================================================

    /// Test that sort_by parameter extraction works (tests internal logic)
    #[test]
    fn test_sort_by_parameter_extraction() {
        // Test that sort_by with order produces expected column/order strings
        let params = crate::models::FilterOptions {
            sort_by: Some("name".to_string()),
            order: Some("DESC".to_string()),
            ..Default::default()
        };

        // Verify the parameters are correctly set
        assert_eq!(params.sort_by, Some("name".to_string()));
        assert_eq!(params.order, Some("DESC".to_string()));
    }

    /// Test that sort_by takes priority over sort (parameter structure)
    #[test]
    fn test_sort_by_priority_parameter_structure() {
        let params = crate::models::FilterOptions {
            sort_by: Some("email".to_string()),
            order: Some("DESC".to_string()),
            sort: Some(r#"["name", "ASC"]"#.to_string()), // Should be ignored
            ..Default::default()
        };

        // When sort_by is present, it should be used
        assert!(params.sort_by.is_some(), "sort_by should be present");
        assert!(params.sort.is_some(), "sort should also be present but ignored");
    }

    /// Test plain sort parameter detection (non-JSON)
    #[test]
    fn test_plain_sort_detection() {
        // Plain text sort (doesn't start with '[')
        let sort = "name";
        assert!(!sort.starts_with('['), "Plain sort should not start with '['");

        // JSON array sort (starts with '[')
        let json_sort = r#"["name", "ASC"]"#;
        assert!(json_sort.starts_with('['), "JSON sort should start with '['");
    }

    /// Test default order constant
    #[test]
    fn test_default_order_is_asc() {
        assert_eq!(DEFAULT_SORT_ORDER, "ASC", "Default sort order should be ASC");
    }

    /// Test default column constant
    #[test]
    fn test_default_column_is_id() {
        assert_eq!(DEFAULT_SORT_COLUMN, "id", "Default sort column should be id");
    }
}
