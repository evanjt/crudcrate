use axum::http::header::HeaderMap;

pub(crate) const MAX_PAGE_SIZE: u64 = 1000;
pub(crate) const MAX_OFFSET: u64 = 1_000_000;

/// Sanitize resource name by removing control characters for HTTP headers
fn sanitize_resource_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii() && !c.is_ascii_control())
        .collect()
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
/// If the `resource_name` contains invalid header characters, it will be sanitized.
///
/// # Panics
///
/// This function includes a fallback mechanism and should never panic in practice.
/// The fallback uses a hardcoded valid header string `"items 0-0/0"` which is
/// guaranteed to parse successfully.
#[must_use]
pub fn calculate_content_range(
    offset: u64,
    limit: u64,
    total_count: u64,
    resource_name: &str,
) -> HeaderMap {
    // Calculate max offset limit for the content range
    // Use saturating arithmetic to prevent integer overflow
    // When offset >= total_count (past the end), produce a valid range with start=end
    let max_offset_limit = if total_count == 0 || offset >= total_count {
        offset // start == end for empty/out-of-range
    } else {
        offset
            .saturating_add(limit)
            .saturating_sub(1)
            .min(total_count.saturating_sub(1))
    };

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

#[must_use]
pub fn parse_range(range_str: Option<String>) -> (u64, u64) {
    range_str.map_or((0, 9), |r| {
        serde_json::from_str::<[u64; 2]>(&r).map_or((0, 9), |range| (range[0], range[1]))
    })
}

#[must_use]
pub fn parse_pagination(params: &crate::models::FilterOptions) -> (u64, u64) {
    // `std::cmp::min` is spelled out throughout: SeaQuery's blanket `ExprTrait` impl
    // covers every type, so the `.min()` method call is ambiguous with `Ord::min`.
    if let (Some(page), Some(per_page)) = (params.page, params.per_page) {
        // Standard REST pagination (1-based page numbers)
        // Clamp on both ends: the upper bound prevents DoS, the lower bound keeps a
        // `per_page=0` from producing an empty page that never advances (and from
        // reaching backends that reject a zero page size).
        let safe_per_page = Ord::clamp(per_page, 1, MAX_PAGE_SIZE);

        // Use saturating_mul to prevent overflow panic
        let offset = (page.saturating_sub(1)).saturating_mul(safe_per_page);

        // Enforce maximum offset to prevent excessive database queries
        let safe_offset = std::cmp::min(offset, MAX_OFFSET);

        (safe_offset, safe_per_page)
    } else if let Some(range) = &params.range {
        // React Admin pagination
        let (start, end) = parse_range(Some(range.clone()));
        // saturating_add: a client-supplied end of u64::MAX would otherwise overflow
        // the `+ 1` (panic in debug/test, silent wrap-to-zero in release).
        let limit = std::cmp::min(end.saturating_sub(start).saturating_add(1), MAX_PAGE_SIZE);
        let safe_start = std::cmp::min(start, MAX_OFFSET);
        (safe_start, limit)
    } else {
        // Default pagination
        (0, 10)
    }
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
        assert!(
            value.is_some(),
            "Should return a valid header even with bad input"
        );

        // The value should NOT contain control characters (newlines)
        // This prevents header injection attacks
        if let Some(val) = value {
            let val_str = val.to_str().unwrap_or("");
            assert!(!val_str.contains('\r'), "Should remove carriage returns");
            assert!(!val_str.contains('\n'), "Should remove newlines");
            // The word "Injected" may still appear but without newlines it can't inject headers
        }
    }

    /// Test that resource names with non-ASCII unicode are sanitized
    #[test]
    fn test_content_range_unicode() {
        // Non-ASCII characters are stripped by sanitize_resource_name
        let headers = calculate_content_range(0, 10, 100, "用户");
        let value = headers.get("Content-Range");
        // After sanitization, non-ASCII chars are removed, leaving empty name
        // The header should still be valid
        assert!(
            value.is_some(),
            "Should produce a valid header even with non-ASCII input"
        );
    }

    /// Test Content-Range with offset exceeding total count
    #[test]
    fn test_content_range_offset_exceeds_total() {
        let headers = calculate_content_range(100, 10, 50, "items");
        let value = headers.get("Content-Range").unwrap().to_str().unwrap();
        // Start should not exceed end in the range
        // Parse "items START-END/TOTAL"
        let parts: Vec<&str> = value.split(' ').collect();
        let range_part = parts[1].split('/').next().unwrap();
        let range_nums: Vec<u64> = range_part.split('-').map(|s| s.parse().unwrap()).collect();
        assert!(
            range_nums[0] <= range_nums[1] || range_nums[0] == range_nums[1],
            "Range start ({}) should not exceed end ({}) in: {}",
            range_nums[0],
            range_nums[1],
            value
        );
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
        let headers = calculate_content_range(u64::MAX - 100, 10, u64::MAX, "users");
        let value = headers.get("Content-Range").unwrap().to_str().unwrap();
        assert!(value.contains("users"));
    }
}
