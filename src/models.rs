use serde::Deserialize;
use serde_json::json;
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
/// The `range` parameter should be a JSON array with two numbers representing the start and end indices, for example:
/// ```json
/// [0,9]
/// ```
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
    /// Sort order for the results in the format '["column", "order"]'.
    ///
    /// Example: `["id", "ASC"]`
    #[param(example = r#"["id", "ASC"]"#)]
    pub sort: Option<String>,
}
