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
        sort_vec
            .first()
            .cloned()
            .unwrap_or(DEFAULT_SORT_COLUMN.to_string()),
        sort_vec
            .get(1)
            .cloned()
            .unwrap_or(DEFAULT_SORT_ORDER.to_string()),
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
    let (sort_column, sort_order) = sort.map_or(
        (
            DEFAULT_SORT_COLUMN.to_string(),
            DEFAULT_SORT_ORDER.to_string(),
        ),
        parse_json_sort,
    );

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
        (
            sort_by.clone(),
            params
                .order
                .as_deref()
                .unwrap_or(DEFAULT_SORT_ORDER)
                .to_string(),
        )
    } else if let Some(sort) = &params.sort {
        // Check if sort is a simple column name (REST) or JSON array (React Admin)
        if sort.starts_with('[') {
            // React Admin format: sort=["column", "ASC"]
            parse_json_sort(sort)
        } else {
            // REST format: sort=column&order=ASC/DESC
            (
                sort.clone(),
                params
                    .order
                    .as_deref()
                    .unwrap_or(DEFAULT_SORT_ORDER)
                    .to_string(),
            )
        }
    } else {
        (
            DEFAULT_SORT_COLUMN.to_string(),
            DEFAULT_SORT_ORDER.to_string(),
        )
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
/// A dot-notation path (e.g. `vehicles.year`) yields `SortConfig::Joined` only
/// when it appears in [`crate::traits::CRUDResource::joined_sortable_columns`];
/// any other dot path falls back to a regular `SortConfig::Column` on the
/// default index column. The `get_all` handler dispatches `SortConfig::Joined`
/// to [`crate::traits::CRUDResource::get_all_joined_sorted`], which orders the
/// parent rows by a correlated sub-query over the child column.
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
/// // handler orders customers by (SELECT MIN(vehicles.year) WHERE customer_id = customers.id) DESC
/// ```
pub fn parse_sorting_with_joins<T, C>(
    params: &crate::models::FilterOptions,
    order_column_logic: &[(&str, C)],
    default_column: C,
    scoped_excluded: &[&str],
) -> super::joined::SortConfig<C>
where
    T: crate::traits::CRUDResource,
    C: ColumnTrait + Copy,
{
    use super::joined::SortConfig;

    let (sort_column, sort_order) = if let Some(sort_by) = &params.sort_by {
        (
            sort_by.clone(),
            params
                .order
                .as_deref()
                .unwrap_or(DEFAULT_SORT_ORDER)
                .to_string(),
        )
    } else if let Some(sort) = &params.sort {
        if sort.starts_with('[') {
            parse_json_sort(sort)
        } else {
            (
                sort.clone(),
                params
                    .order
                    .as_deref()
                    .unwrap_or(DEFAULT_SORT_ORDER)
                    .to_string(),
            )
        }
    } else {
        (
            DEFAULT_SORT_COLUMN.to_string(),
            DEFAULT_SORT_ORDER.to_string(),
        )
    };

    let order_direction = parse_order(&sort_order);

    // Check if this is a dot-notation sort (e.g., "vehicles.year")
    if sort_column.contains('.') {
        let parts: Vec<&str> = sort_column.splitn(2, '.').collect();
        if parts.len() == 2 {
            let join_field = parts[0];
            let column = parts[1];

            // Validate against allowed joined sortable columns. When scoped, drop any
            // whose column is scope-excluded, so a hidden column can't be ordered on
            // through a join.
            let joined_sortable = T::joined_sortable_columns();
            let is_allowed = joined_sortable.iter().any(|c| c.full_path == sort_column)
                && !scoped_excluded.contains(&column);

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
#[path = "sort_tests.rs"]
mod tests;
