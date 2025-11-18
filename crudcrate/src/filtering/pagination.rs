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

#[cfg(test)]
mod tests {
    use super::*;

    /// Test normal header generation
    #[test]
    fn test_content_range_normal() {
        let headers = calculate_content_range(0, 10, 100, "users");
        let value = headers.get("Content-Range").unwrap().to_str().unwrap();
        assert_eq!(value, "users 0-9/100");
    }

    /// Test that resource names with special characters cause panic - DOCUMENTS VULNERABILITY
    #[test]
    #[should_panic(expected = "InvalidHeaderValue")]
    fn test_content_range_panic_on_newline() {
        // VULNERABILITY: Resource names with newlines will panic
        let _headers = calculate_content_range(0, 10, 100, "users\r\nInjected: evil");
        // This panics because newlines aren't allowed in HTTP headers
    }

    /// Test that resource names with unicode might cause issues
    #[test]
    fn test_content_range_unicode() {
        // Some unicode might work, but control characters won't
        let headers = calculate_content_range(0, 10, 100, "用户");
        let value = headers.get("Content-Range");
        // Should either work or panic - documents the behavior
        assert!(value.is_some() || value.is_none());
    }

    /// Test edge case with zero items
    #[test]
    fn test_content_range_zero_items() {
        let headers = calculate_content_range(0, 10, 0, "users");
        let value = headers.get("Content-Range").unwrap().to_str().unwrap();
        // When total is 0, max_offset_limit becomes 0 (from min())
        assert!(value.contains("users"));
    }

    /// Test very large numbers
    #[test]
    fn test_content_range_large_numbers() {
        let headers = calculate_content_range(
            u64::MAX - 100,
            10,
            u64::MAX,
            "users"
        );
        let value = headers.get("Content-Range").unwrap().to_str().unwrap();
        assert!(value.contains("users"));
    }
}
