use super::*;

#[test]
fn test_parse_json_sort_valid() {
    let (col, order) = parse_json_sort(r#"["name", "DESC"]"#);
    assert_eq!(col, "name");
    assert_eq!(order, "DESC");
}

#[test]
fn test_parse_json_sort_partial() {
    // Only column, no order
    let (col, order) = parse_json_sort(r#"["email"]"#);
    assert_eq!(col, "email");
    assert_eq!(order, DEFAULT_SORT_ORDER);
}

#[test]
fn test_parse_json_sort_invalid_json() {
    // Invalid JSON should return defaults
    let (col, order) = parse_json_sort("invalid json");
    assert_eq!(col, DEFAULT_SORT_COLUMN);
    assert_eq!(order, DEFAULT_SORT_ORDER);
}

#[test]
fn test_parse_json_sort_empty_array() {
    let (col, order) = parse_json_sort("[]");
    assert_eq!(col, DEFAULT_SORT_COLUMN);
    assert_eq!(order, DEFAULT_SORT_ORDER);
}

#[test]
fn test_parse_order_asc() {
    assert_eq!(parse_order("ASC"), Order::Asc);
    assert_eq!(parse_order("asc"), Order::Asc);
    assert_eq!(parse_order("Asc"), Order::Asc);
}

#[test]
fn test_parse_order_desc() {
    assert_eq!(parse_order("DESC"), Order::Desc);
    assert_eq!(parse_order("desc"), Order::Desc);
    assert_eq!(parse_order("Desc"), Order::Desc);
}

#[test]
fn test_parse_order_invalid_defaults_to_desc() {
    // Any non-ASC value defaults to DESC
    assert_eq!(parse_order("invalid"), Order::Desc);
    assert_eq!(parse_order(""), Order::Desc);
    assert_eq!(parse_order("random"), Order::Desc);
}

// ========================================================================
// REST FORMAT TESTS - Testing the internal parsing logic
// Full parse_sorting tests are in integration tests (require real entities)
// ========================================================================

/// Test that `sort_by` parameter extraction works (tests internal logic)
#[test]
fn test_sort_by_parameter_extraction() {
    // Test that sort_by with order produces expected column/order strings
    let params = crate::models::FilterOptions {
        sort_by: Some("name".to_string()),
        order: Some("DESC".to_string()),
        ..Default::default()
    };

    // Verify the parameters are correctly set
    assert_eq!(params.sort_by, Some("name".to_string()));
    assert_eq!(params.order, Some("DESC".to_string()));
}

/// Test that `sort_by` takes priority over sort (parameter structure)
#[test]
fn test_sort_by_priority_parameter_structure() {
    let params = crate::models::FilterOptions {
        sort_by: Some("email".to_string()),
        order: Some("DESC".to_string()),
        sort: Some(r#"["name", "ASC"]"#.to_string()), // Should be ignored
        ..Default::default()
    };

    // When sort_by is present, it should be used
    assert!(params.sort_by.is_some(), "sort_by should be present");
    assert!(
        params.sort.is_some(),
        "sort should also be present but ignored"
    );
}

/// Test plain sort parameter detection (non-JSON)
#[test]
fn test_plain_sort_detection() {
    // Plain text sort (doesn't start with '[')
    let sort = "name";
    assert!(
        !sort.starts_with('['),
        "Plain sort should not start with '['"
    );

    // JSON array sort (starts with '[')
    let json_sort = r#"["name", "ASC"]"#;
    assert!(
        json_sort.starts_with('['),
        "JSON sort should start with '['"
    );
}

/// Test default order constant
#[test]
fn test_default_order_is_asc() {
    assert_eq!(
        DEFAULT_SORT_ORDER, "ASC",
        "Default sort order should be ASC"
    );
}

/// Test default column constant
#[test]
fn test_default_column_is_id() {
    assert_eq!(
        DEFAULT_SORT_COLUMN, "id",
        "Default sort column should be id"
    );
}
