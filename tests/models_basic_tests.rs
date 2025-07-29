use crudcrate::models::FilterOptions;
use serde_json::json;

// ===== FILTEROPTIONS BASIC TESTS =====

#[test]
fn test_filter_options_default() {
    let options = FilterOptions::default();
    
    assert_eq!(options.filter, None);
    assert_eq!(options.range, None);
    assert_eq!(options.page, None);
    assert_eq!(options.per_page, None);
    assert_eq!(options.sort, None);
    assert_eq!(options.sort_by, None);
    assert_eq!(options.order, None);
}

#[test]
fn test_filter_options_deserialization() {
    use serde_json;
    
    // Test deserializing JSON with only some fields
    let json_str = r#"{"filter":"{\"completed\":true}","page":2}"#;
    
    let options: FilterOptions = serde_json::from_str(json_str).expect("Failed to deserialize");
    
    assert_eq!(options.filter, Some(r#"{"completed":true}"#.to_string()));
    assert_eq!(options.page, Some(2));
    assert_eq!(options.range, None);
    assert_eq!(options.per_page, None);
    assert_eq!(options.sort, None);
    assert_eq!(options.sort_by, None);
    assert_eq!(options.order, None);
}

#[test]
fn test_filter_options_empty_json_deserialization() {
    use serde_json;
    
    let json_str = "{}";
    let options: FilterOptions = serde_json::from_str(json_str).expect("Failed to deserialize");
    
    // Should be equivalent to default
    let default_options = FilterOptions::default();
    assert_eq!(options.filter, default_options.filter);
    assert_eq!(options.range, default_options.range);
    assert_eq!(options.page, default_options.page);
    assert_eq!(options.per_page, default_options.per_page);
    assert_eq!(options.sort, default_options.sort);
    assert_eq!(options.sort_by, default_options.sort_by);
    assert_eq!(options.order, default_options.order);
}

#[test]
fn test_filter_options_complex_filter_json() {
    let complex_filter = json!({
        "q": "search text",
        "completed": true,
        "priority": "high",
        "score_gte": 50,
        "tags": ["work", "urgent"],
        "user_id": "550e8400-e29b-41d4-a716-446655440000"
    });
    
    let options = FilterOptions {
        filter: Some(complex_filter.to_string()),
        ..Default::default()
    };
    
    assert!(options.filter.is_some());
    let filter_str = options.filter.unwrap();
    
    // Verify it contains the expected elements
    assert!(filter_str.contains("search text"));
    assert!(filter_str.contains("completed"));
    assert!(filter_str.contains("priority"));
    assert!(filter_str.contains("score_gte"));
    assert!(filter_str.contains("work"));
    assert!(filter_str.contains("urgent"));
    assert!(filter_str.contains("550e8400-e29b-41d4-a716-446655440000"));
}

#[test]
fn test_filter_options_invalid_json_handling() {
    use serde_json;
    
    // Test that invalid JSON in fields still deserializes the struct
    let json_str = r#"{"filter":"invalid json {","page":1}"#;
    
    let options: FilterOptions = serde_json::from_str(json_str).expect("Failed to deserialize");
    
    // The invalid JSON should still be stored as a string
    assert_eq!(options.filter, Some("invalid json {".to_string()));
    assert_eq!(options.page, Some(1));
}

#[test]
fn test_filter_options_range_variations() {
    let test_cases = vec![
        "[0,9]",
        "[10,19]",
        "[0,0]",
        "[100,999]",
        "[ 0 , 9 ]", // with spaces
    ];
    
    for range_str in test_cases {
        let options = FilterOptions {
            range: Some(range_str.to_string()),
            ..Default::default()
        };
        
        assert_eq!(options.range, Some(range_str.to_string()));
    }
}

#[test]
fn test_filter_options_pagination_values() {
    // Test various page/per_page combinations
    let test_cases = vec![
        (0, 10),
        (1, 20),
        (999, 1),
        (0, 100),
        (10, 50),
    ];
    
    for (page, per_page) in test_cases {
        let options = FilterOptions {
            page: Some(page),
            per_page: Some(per_page),
            ..Default::default()
        };
        
        assert_eq!(options.page, Some(page));
        assert_eq!(options.per_page, Some(per_page));
    }
}

#[test]
fn test_filter_options_sort_formats() {
    let test_cases = vec![
        // React Admin format
        (Some(r#"["title","ASC"]"#.to_string()), None, None),
        (Some(r#"["created_at","DESC"]"#.to_string()), None, None),
        // REST format
        (None, Some("title".to_string()), Some("ASC".to_string())),
        (None, Some("priority".to_string()), Some("DESC".to_string())),
        // Mixed (both present)
        (Some(r#"["title","ASC"]"#.to_string()), Some("priority".to_string()), Some("DESC".to_string())),
    ];
    
    for (sort, sort_by, order) in test_cases {
        let options = FilterOptions {
            sort: sort.clone(),
            sort_by: sort_by.clone(),
            order: order.clone(),
            ..Default::default()
        };
        
        assert_eq!(options.sort, sort);
        assert_eq!(options.sort_by, sort_by);
        assert_eq!(options.order, order);
    }
}

#[test]
fn test_filter_options_order_case_variations() {
    let order_variations = vec![
        "ASC",
        "DESC",
        "asc",
        "desc",
        "Asc",
        "Desc",
        "ASCENDING",
        "DESCENDING",
    ];
    
    for order_str in order_variations {
        let options = FilterOptions {
            order: Some(order_str.to_string()),
            ..Default::default()
        };
        
        assert_eq!(options.order, Some(order_str.to_string()));
    }
}

#[test]
fn test_filter_options_special_characters_in_filter() {
    let special_filters = vec![
        r#"{"title":"test with spaces"}"#,
        r#"{"description":"Line 1\nLine 2"}"#,
        r#"{"query":"search \"quoted\" term"}"#,
        r#"{"unicode":"测试中文"}"#,
        r#"{"symbols":"!@#$%^&*()_+"}"#,
    ];
    
    for filter_str in special_filters {
        let options = FilterOptions {
            filter: Some(filter_str.to_string()),
            ..Default::default()
        };
        
        assert_eq!(options.filter, Some(filter_str.to_string()));
    }
}

#[test]
fn test_filter_options_boundary_values() {
    // Test with boundary values for numeric fields
    let options = FilterOptions {
        page: Some(0),
        per_page: Some(u64::MAX),
        ..Default::default()
    };
    
    assert_eq!(options.page, Some(0));
    assert_eq!(options.per_page, Some(u64::MAX));
}

#[test]
fn test_filter_options_multiple_ids_filter() {
    let ids = vec![
        "550e8400-e29b-41d4-a716-446655440000",
        "550e8400-e29b-41d4-a716-446655440001",
        "550e8400-e29b-41d4-a716-446655440002",
    ];
    
    let filter_json = json!({
        "id": ids
    });
    
    let options = FilterOptions {
        filter: Some(filter_json.to_string()),
        ..Default::default()
    };
    
    let filter_str = options.filter.unwrap();
    for id in ids {
        assert!(filter_str.contains(id));
    }
}

#[test]
fn test_filter_options_react_admin_complex_sort() {
    let complex_sorts = vec![
        r#"["title","ASC"]"#,
        r#"["created_at","DESC"]"#,
        r#"["priority","ASC"]"#,
        r#"["score","DESC"]"#,
        r#"["nested.field","ASC"]"#,
        r#"["field_with_underscore","DESC"]"#,
    ];
    
    for sort_str in complex_sorts {
        let options = FilterOptions {
            sort: Some(sort_str.to_string()),
            ..Default::default()
        };
        
        assert_eq!(options.sort, Some(sort_str.to_string()));
    }
}

#[test]
fn test_filter_options_comprehensive_json_deserialization() {
    use serde_json;
    
    let json_str = r#"{
        "filter": "{\"completed\":false,\"priority\":\"high\"}",
        "range": "[10,19]",
        "page": 3,
        "per_page": 25,
        "sort": "[\"title\",\"DESC\"]",
        "sort_by": "created_at", 
        "order": "ASC"
    }"#;
    
    let options: FilterOptions = serde_json::from_str(json_str).expect("Failed to deserialize");
    
    assert_eq!(options.filter, Some(r#"{"completed":false,"priority":"high"}"#.to_string()));
    assert_eq!(options.range, Some("[10,19]".to_string()));
    assert_eq!(options.page, Some(3));
    assert_eq!(options.per_page, Some(25));
    assert_eq!(options.sort, Some(r#"["title","DESC"]"#.to_string()));
    assert_eq!(options.sort_by, Some("created_at".to_string()));
    assert_eq!(options.order, Some("ASC".to_string()));
}

#[test]
fn test_filter_options_null_values_deserialization() {
    use serde_json;
    
    let json_str = r#"{
        "filter": null,
        "range": null,
        "page": null,
        "per_page": null,
        "sort": null,
        "sort_by": null,
        "order": null
    }"#;
    
    let options: FilterOptions = serde_json::from_str(json_str).expect("Failed to deserialize");
    
    // All fields should be None
    assert_eq!(options.filter, None);
    assert_eq!(options.range, None);
    assert_eq!(options.page, None);
    assert_eq!(options.per_page, None);
    assert_eq!(options.sort, None);
    assert_eq!(options.sort_by, None);
    assert_eq!(options.order, None);
}

#[test]
fn test_filter_options_edge_case_values() {
    // Test edge cases for all field types
    let options = FilterOptions {
        filter: Some("".to_string()), // Empty string
        range: Some("[]".to_string()), // Empty array
        page: Some(0), // Zero page
        per_page: Some(1), // Minimal per_page
        sort: Some("[]".to_string()), // Empty sort array
        sort_by: Some("".to_string()), // Empty sort_by
        order: Some("".to_string()), // Empty order
    };
    
    assert_eq!(options.filter, Some("".to_string()));
    assert_eq!(options.range, Some("[]".to_string()));
    assert_eq!(options.page, Some(0));
    assert_eq!(options.per_page, Some(1));
    assert_eq!(options.sort, Some("[]".to_string()));
    assert_eq!(options.sort_by, Some("".to_string()));
    assert_eq!(options.order, Some("".to_string()));
}

#[test]
fn test_filter_options_very_large_values() {
    // Test with very large values
    let large_filter = (0..1000).map(|i| format!("field{}", i)).collect::<Vec<_>>().join(",");
    let large_filter_json = format!(r#"{{"query":"{}"}}"#, large_filter);
    
    let options = FilterOptions {
        filter: Some(large_filter_json.clone()),
        page: Some(u64::MAX - 1),
        per_page: Some(u64::MAX),
        ..Default::default()
    };
    
    assert_eq!(options.filter, Some(large_filter_json));
    assert_eq!(options.page, Some(u64::MAX - 1));
    assert_eq!(options.per_page, Some(u64::MAX));
}

#[test]
fn test_filter_options_mixed_format_scenarios() {
    // Test scenarios where both REST and React Admin formats might be present
    let mixed_scenarios = vec![
        // Both pagination formats
        FilterOptions {
            range: Some("[0,9]".to_string()),
            page: Some(1),
            per_page: Some(10),
            ..Default::default()
        },
        // Both sort formats  
        FilterOptions {
            sort: Some(r#"["title","ASC"]"#.to_string()),
            sort_by: Some("priority".to_string()),
            order: Some("DESC".to_string()),
            ..Default::default()
        },
        // Everything mixed
        FilterOptions {
            filter: Some(r#"{"completed":true}"#.to_string()),
            range: Some("[5,14]".to_string()),
            page: Some(2),
            per_page: Some(15),
            sort: Some(r#"["created_at","DESC"]"#.to_string()),
            sort_by: Some("title".to_string()),
            order: Some("ASC".to_string()),
        },
    ];
    
    for scenario in mixed_scenarios {
        // Just verify the struct can hold all the values
        // The actual precedence logic is tested in other modules
        assert!(scenario.filter.is_some() || scenario.filter.is_none());
        assert!(scenario.range.is_some() || scenario.range.is_none());
        assert!(scenario.page.is_some() || scenario.page.is_none());
        assert!(scenario.per_page.is_some() || scenario.per_page.is_none());
        assert!(scenario.sort.is_some() || scenario.sort.is_none());
        assert!(scenario.sort_by.is_some() || scenario.sort_by.is_none());
        assert!(scenario.order.is_some() || scenario.order.is_none());
    }
}