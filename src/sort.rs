use sea_orm::{ColumnTrait, sea_query::Order};

pub fn generic_sort<C>(
    sort: Option<String>,
    order_column_logic: &[(&str, C)],
    default_column: C,
) -> (C, Order)
where
    C: ColumnTrait,
{
    // Default sorting values
    let default_sort_column = "id";
    let default_sort_order = "ASC";

    // Parse the sort column and order
    let (sort_column, sort_order) = if let Some(sort) = sort {
        let sort_vec: Vec<String> = serde_json::from_str(&sort).unwrap_or(vec![
            default_sort_column.to_string(),
            default_sort_order.to_string(),
        ]);
        (
            sort_vec
                .first()
                .cloned()
                .unwrap_or(default_sort_column.to_string()),
            sort_vec
                .get(1)
                .cloned()
                .unwrap_or(default_sort_order.to_string()),
        )
    } else {
        (
            default_sort_column.to_string(),
            default_sort_order.to_string(),
        )
    };

    // Determine order direction
    let order_direction = if sort_order == "ASC" {
        Order::Asc
    } else {
        Order::Desc
    };

    // Find the corresponding column in the logic or use the default column
    let order_column = order_column_logic
        .iter()
        .find(|&&(col_name, _)| col_name == sort_column)
        .map_or(default_column, |&(_, col)| col);

    (order_column, order_direction)
}

/// Parse sorting from FilterOptions, supporting both React Admin and standard REST formats
pub fn parse_sorting<C>(
    params: &crate::models::FilterOptions,
    order_column_logic: &[(&str, C)],
    default_column: C,
) -> (C, Order)
where
    C: ColumnTrait + Copy,
{
    // Default sorting values
    let default_sort_column = "id";
    let default_sort_order = "ASC";

    // Parse the sort column and order
    let (sort_column, sort_order) = if let Some(sort_by) = &params.sort_by {
        // Standard REST format: sort_by=column&order=ASC/DESC
        let order = params.order.as_deref().unwrap_or(default_sort_order);
        (sort_by.clone(), order.to_string())
    } else if let Some(sort) = &params.sort {
        // Check if sort is a simple column name (REST) or JSON array (React Admin)
        if sort.starts_with('[') {
            // React Admin format: sort=["column", "ASC"]
            let sort_vec: Vec<String> = serde_json::from_str(sort).unwrap_or(vec![
                default_sort_column.to_string(),
                default_sort_order.to_string(),
            ]);
            (
                sort_vec
                    .first()
                    .cloned()
                    .unwrap_or(default_sort_column.to_string()),
                sort_vec
                    .get(1)
                    .cloned()
                    .unwrap_or(default_sort_order.to_string()),
            )
        } else {
            // REST format: sort=column&order=ASC/DESC
            let order = params.order.as_deref().unwrap_or(default_sort_order);
            (sort.clone(), order.to_string())
        }
    } else {
        // No sorting specified, use defaults
        (
            default_sort_column.to_string(),
            default_sort_order.to_string(),
        )
    };

    // Determine order direction
    let order_direction = if sort_order.to_uppercase() == "ASC" {
        Order::Asc
    } else {
        Order::Desc
    };

    // Find the corresponding column in the logic or use the default column
    let order_column = order_column_logic
        .iter()
        .find(|&&(col_name, _)| col_name == sort_column)
        .map_or(default_column, |&(_, col)| col);

    (order_column, order_direction)
}
