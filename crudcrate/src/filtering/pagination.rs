use axum::http::header::HeaderMap;

/// Function to calculate the total count and generate the Content-Range header.
///
/// # Arguments
///
/// * `offset` - The starting point of the range.
/// * `limit` - The maximum number of items to include in the range.
/// * `total_count` - The total number of items available.
/// * `resource_name` - The name of the resource being paginated.
///
/// # Returns
///
/// A `HeaderMap` containing the Content-Range header.
///
/// # Panics
///
/// This function will panic if the `content_range` string cannot be parsed into a valid header value.
#[must_use]
pub fn calculate_content_range(
    offset: u64,
    limit: u64,
    total_count: u64,
    resource_name: &str,
) -> HeaderMap {
    // Calculate max offset limit for the content range
    let max_offset_limit = (offset + limit - 1).min(total_count);

    // Create the Content-Range string
    let content_range = format!("{resource_name} {offset}-{max_offset_limit}/{total_count}");

    // Return Content-Range as a header
    let mut headers = HeaderMap::new();
    headers.insert("Content-Range", content_range.parse().unwrap());

    headers
}
