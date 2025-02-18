use sea_orm::{sea_query::Order, ColumnTrait};

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
                .get(0)
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
        .map(|&(_, col)| col)
        .unwrap_or(default_column);

    (order_column, order_direction)
}
