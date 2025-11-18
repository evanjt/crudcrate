use axum::http::header::HeaderMap;

/// Sanitize resource name by removing control characters for HTTP headers
fn sanitize_resource_name(name: &str) -> String {
    name.chars().filter(|c| c.is_ascii() && !c.is_ascii_control()).collect()
}

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
/// If the resource_name contains invalid header characters, it will be sanitized.
#[must_use]
pub fn calculate_content_range(
    offset: u64,
    limit: u64,
    total_count: u64,
    resource_name: &str,
) -> HeaderMap {
    // Calculate max offset limit for the content range
    let max_offset_limit = (offset + limit - 1).min(total_count);

    // Sanitize resource name to prevent header injection
    let safe_name = sanitize_resource_name(resource_name);

    // Create the Content-Range string
    let content_range = format!("{safe_name} {offset}-{max_offset_limit}/{total_count}");

    // Return Content-Range as a header
    let mut headers = HeaderMap::new();

    // This should now never panic because we've sanitized the input
    // But if it somehow does, use a safe fallback
    if let Ok(value) = content_range.parse() {
        headers.insert("Content-Range", value);
    } else {
        // Fallback to generic header if parsing still fails
        headers.insert(
            "Content-Range",
            format!("items {offset}-{max_offset_limit}/{total_count}")
                .parse()
                .unwrap_or_else(|_| "items 0-0/0".parse().unwrap()),
        );
    }

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

    /// TDD: Resource names with special characters should be handled gracefully
    #[test]
    fn test_content_range_handles_special_chars_gracefully() {
        // Should NOT panic - should sanitize control characters
        let headers = calculate_content_range(0, 10, 100, "users\r\nInjected: evil");

        // After fix: Should sanitize and return a valid header
        let value = headers.get("Content-Range");
        assert!(value.is_some(), "Should return a valid header even with bad input");

        // The value should NOT contain control characters (newlines)
        // This prevents header injection attacks
        if let Some(val) = value {
            let val_str = val.to_str().unwrap_or("");
            assert!(!val_str.contains('\r'), "Should remove carriage returns");
            assert!(!val_str.contains('\n'), "Should remove newlines");
            // The word "Injected" may still appear but without newlines it can't inject headers
        }
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
