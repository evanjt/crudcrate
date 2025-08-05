use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

/// Query parameters for filtering, pagination, and sorting resources.
///
/// # Filtering
/// The `filter` parameter accepts a JSON-encoded string with various options:
/// - **Free text search:** Use the key `"q"` with a string value, for example:
///   ```json
///   {"q": "search text"}
///   ```
/// - **Filter by a single ID:** Use the key `"id"` with a UUID string, for example:
///   ```json
///   {"id": "550e8400-e29b-41d4-a716-446655440000"}
///   ```
/// - **Filter by multiple IDs:** Use the key `"id"` with an array of UUID strings, for example:
///   ```json
///   {"id": ["550e8400-e29b-41d4-a716-446655440000", "550e8400-e29b-41d4-a716-446655440001"]}
///   ```
/// - **Filter by other columns:** Include any additional key-value pairs, for example:
///   ```json
///   {"name": "example"}
///   ```
///
/// # Pagination
/// Two pagination formats are supported:
/// - **React Admin format:** Use the `range` parameter with JSON array format, for example: `[0,9]`
/// - **Standard REST format:** Use `page` and `per_page` parameters, for example: `page=1&per_page=10`
///
/// # Sorting
/// The `sort` parameter should be a JSON array with the column name and sort order, for example:
/// ```json
/// ["id", "ASC"]
/// ```
#[derive(Deserialize, IntoParams, ToSchema, Default)]
#[into_params(parameter_in = Query)]
pub struct FilterOptions {
    /// JSON-encoded filter for querying resources.
    ///
    /// This parameter supports various filtering options:
    /// - Free text search: `{"q": "search text"}`
    /// - Filtering by a single ID: `{"id": "550e8400-e29b-41d4-a716-446655440000"}`
    /// - Filtering by multiple IDs: `{"id": ["550e8400-e29b-41d4-a716-446655440000", "550e8400-e29b-41d4-a716-446655440001"]}`
    /// - Filtering on other columns: `{"name": "example"}`
    #[param(example = json!({
        "q": "search text",
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "name": "example"
    }))]
    pub filter: Option<String>,
    /// Range for pagination in the format "[start, end]".
    ///
    /// Example: `[0,9]`
    #[param(example = "[0,9]")]
    pub range: Option<String>,
    /// Page number for standard REST pagination (1-based).
    ///
    /// Example: `1`
    #[param(example = 1)]
    pub page: Option<u64>,
    /// Number of items per page for standard REST pagination.
    ///
    /// Example: `10`
    #[param(example = 10)]
    pub per_page: Option<u64>,
    /// Sort order for the results in the format `["column", "order"]`.
    ///
    /// Example: `["id", "ASC"]`
    #[param(example = r#"["id", "ASC"]"#)]
    pub sort: Option<String>,
    /// Sort column for standard REST format.
    ///
    /// Example: `title`
    #[param(example = "title")]
    pub sort_by: Option<String>,
    /// Sort order for standard REST format (ASC or DESC).
    ///
    /// Example: `ASC`
    #[param(example = "ASC")]
    pub order: Option<String>,
}
