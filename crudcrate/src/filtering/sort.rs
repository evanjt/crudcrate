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
