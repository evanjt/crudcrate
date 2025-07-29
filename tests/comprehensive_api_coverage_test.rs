/// Comprehensive integration test to achieve 80%+ coverage
/// Tests all API endpoints, error conditions, and edge cases through full HTTP requests
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

mod common;
use common::{setup_test_app, setup_test_db, todo_entity::Todo};

/// Test all sorting functionality to improve sort.rs coverage
#[tokio::test]
async fn test_comprehensive_sorting_coverage() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create diverse test data for sorting
    let test_todos = vec![
        json!({"title": "Alpha Task", "completed": false}),
        json!({"title": "Beta Task", "completed": true}),
        json!({"title": "Charlie Task", "completed": false}),
        json!({"title": "Delta Task", "completed": true}),
        json!({"title": "Echo Task", "completed": false}),
    ];

    for todo_data in test_todos {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
            .unwrap();
        app.clone().oneshot(request).await.unwrap();
    }

    // Test all possible sorting combinations to hit uncovered sort.rs paths
    let sort_test_cases = vec![
        // React Admin format with valid JSON
        (r#"["title","ASC"]"#, None, None),
        (r#"["title","DESC"]"#, None, None),
        
        // React Admin format with invalid JSON (should fallback to defaults)
        ("{invalid json", None, None),
        ("[incomplete", None, None),
        ("[]", None, None), // Empty array
        (r#"["only_one_element"]"#, None, None), // Single element
        (r#"["nonexistent_column","ASC"]"#, None, None), // Unknown column
        
        // REST format
        ("title", Some("ASC"), None),
        ("title", Some("DESC"), None),
        ("title", Some("asc"), None), // Lowercase
        ("title", Some("desc"), None), // Lowercase
        ("title", Some("invalid_order"), None), // Invalid order
        ("title", None, None), // No order (should default)
        ("nonexistent_column", Some("ASC"), None), // Unknown column
        ("empty", Some("ASC"), None), // Empty column name test
        
        // Mixed format precedence testing
        ("title", Some("ASC"), Some(r#"["title","DESC"]"#)), // REST should win
    ];

    for (sort_param, order_param, react_sort_param) in sort_test_cases {
        let mut params = Vec::new();
        
        if let Some(react_sort) = react_sort_param {
            params.push(format!("sort={}", url_escape::encode_component(react_sort)));
        } else if sort_param.starts_with('[') {
            // React Admin format
            params.push(format!("sort={}", url_escape::encode_component(sort_param)));
        } else {
            // REST format
            params.push(format!("sort_by={}", url_escape::encode_component(sort_param)));
        }
        
        if let Some(order) = order_param {
            params.push(format!("order={}", url_escape::encode_component(order)));
        }
        
        params.push("per_page=10".to_string()); // Add pagination
        
        let uri = format!("/api/v1/todos?{}", params.join("&"));

        let request = Request::builder()
            .method("GET")
            .uri(&uri)
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        // Should not crash, but may return different sort orders
        assert!(response.status().is_success() || response.status().is_client_error());
    }
}

/// Test error conditions to improve routes.rs coverage  
#[tokio::test]
async fn test_comprehensive_error_conditions() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Test invalid JSON payloads
    let invalid_payloads = vec![
        "{invalid json",
        r#"{"title": }"#,
        r#"{"completed": "not_a_boolean"}"#,
        "", // Empty body
        "null",
        "[]", // Array instead of object
        r#"{"title": null}"#, // Null title (should be invalid)
    ];

    for payload in invalid_payloads {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(payload))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        // Should return client error for invalid data
        assert!(response.status().is_client_error());
    }

    // Test invalid content types
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "text/plain")
        .body(Body::from(r#"{"title": "Test"}"#))
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert!(response.status().is_client_error());

    // Test missing content type
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .body(Body::from(r#"{"title": "Test"}"#))
        .unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    assert!(response.status().is_client_error());

    // Test operations on non-existent resources
    let non_existent_id = Uuid::new_v4();
    
    // GET non-existent
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/todos/{}", non_existent_id))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // UPDATE non-existent
    let request = Request::builder()
        .method("PUT")
        .uri(&format!("/api/v1/todos/{}", non_existent_id))
        .header("content-type", "application/json")
        .body(Body::from(r#"{"title": "Updated"}"#))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    // DELETE non-existent
    let request = Request::builder()
        .method("DELETE")
        .uri(&format!("/api/v1/todos/{}", non_existent_id))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Test complex filtering scenarios to improve filter.rs coverage
#[tokio::test]
async fn test_comprehensive_filtering_edge_cases() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create test data
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"title": "Test Todo", "completed": false}"#))
        .unwrap();
    app.clone().oneshot(request).await.unwrap();

    // Create large filter string
    let large_field = "x".repeat(1000);
    let large_filter = format!(r#"{{"field_{}": "value"}}"#, large_field);
    
    // Test edge cases in filtering that might not be covered
    let filter_edge_cases = vec![
        // Malformed JSON filters
        r#"{"incomplete json"#,
        r#"{invalid: "json"}"#,
        "null",
        "[]", // Array instead of object
        r#"{"": "empty key"}"#,
        r#"{"key": ""}"#, // Empty value
        
        // Very large filter objects
        large_filter.as_str(),
        
        // Unicode and special characters
        r#"{"title": "测试中文"}"#,
        r#"{"title": "emoji 🚀 test"}"#,
        r#"{"title": "quotes \"and\" backslashes \\"}"#,
        r#"{"title": "newlines\nand\ttabs"}"#,
        
        // SQL injection attempts (should be safely handled)
        r#"{"title": "'; DROP TABLE todos; --"}"#,
        r#"{"title": "' OR '1'='1"}"#,
        
        // Complex nested structures
        r#"{"filter": {"nested": {"deep": "value"}}}"#,
        
        // Multiple filter combinations
        r#"{"completed": true, "title": "test", "nonexistent": "field"}"#,
        
        // Boolean edge cases
        r#"{"completed": null}"#,
        r#"{"completed": "true"}"#, // String instead of boolean
        r#"{"completed": 1}"#, // Number instead of boolean
    ];

    for filter_json in filter_edge_cases {
        let encoded_filter = url_escape::encode_component(filter_json);
        let request = Request::builder()
            .method("GET")
            .uri(&format!("/api/v1/todos?filter={}", encoded_filter))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        // Should not crash, might return empty results or all results
        assert!(response.status().is_success() || response.status().is_client_error());
    }
}

/// Test pagination edge cases to improve coverage
#[tokio::test]
async fn test_comprehensive_pagination_edge_cases() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create some test data
    for i in 0..5 {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"title": "Todo {}", "completed": false}}"#, i)))
            .unwrap();
        app.clone().oneshot(request).await.unwrap();
    }

    // Test pagination edge cases (avoiding overflow conditions)
    let pagination_test_cases = vec![
        // Valid cases
        ("page=0&per_page=10", true),
        ("page=1&per_page=5", true),
        ("range=%5B0%2C4%5D", true), // [0,4] URL encoded
        ("range=%5B2%2C3%5D", true), // [2,3] URL encoded
        
        // Safe edge cases that won't cause overflow
        ("page=1&per_page=1", true), // Minimal pagination
        ("page=10&per_page=10", true), // Way beyond data
        ("per_page=100", true), // Large per_page with default page
        
        // Invalid cases that should be handled gracefully
        ("page=abc&per_page=10", false), // Non-numeric page
        ("page=0&per_page=abc", false), // Non-numeric per_page
        
        // Range edge cases - properly encoded
        ("range=%5B%5D", false), // [] empty range
        ("range=%5B0%2C100%5D", true), // [0,100] very large end
        ("range=invalid", false), // Invalid range format
        
        // Combined edge cases
        ("page=0&per_page=10&range=%5B0%2C4%5D", true), // Both formats
        ("page=&per_page=", false), // Empty values
    ];

    for (query_params, should_succeed) in pagination_test_cases {
        let request = Request::builder()
            .method("GET")
            .uri(&format!("/api/v1/todos?{}", query_params))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        
        if should_succeed {
            assert!(response.status().is_success(), "Failed for: {}", query_params);
        } else {
            // Some might succeed with defaults, others might fail
            assert!(response.status().is_success() || response.status().is_client_error(), 
                   "Unexpected server error for: {}", query_params);
        }
    }
}

/// Test CRUD operations with various data types and edge cases
#[tokio::test]
async fn test_comprehensive_crud_data_variations() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Test CREATE with various data combinations
    let create_test_cases = vec![
        // Valid cases
        json!({"title": "Normal Todo", "completed": false}),
        json!({"title": "Completed Todo", "completed": true}),
        
        // Edge cases
        json!({"title": "", "completed": false}), // Empty title
        json!({"title": "a", "completed": false}), // Single character
        json!({"title": "x".repeat(1000), "completed": false}), // Very long title
        json!({"title": "Unicode 测试 🚀", "completed": false}), // Unicode
        json!({"title": "Special chars !@#$%^&*()", "completed": false}), // Special characters
        json!({"title": "Newlines\nand\ttabs", "completed": false}), // Control characters
        
        // Missing fields (should use defaults or fail gracefully)
        json!({"title": "Missing completed field"}),
        json!({"completed": false}), // Missing title (should fail)
        
        // Extra fields (should be ignored)
        json!({"title": "Extra fields", "completed": false, "extra": "ignored", "number": 123}),
        
        // Type variations
        json!({"title": "String completed", "completed": "false"}), // String instead of bool
        json!({"title": "Number completed", "completed": 0}), // Number instead of bool
        json!({"title": "Null completed", "completed": null}), // Null completed
    ];

    let mut created_ids = Vec::new();

    for (i, todo_data) in create_test_cases.into_iter().enumerate() {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        
        // Some should succeed, others should fail with client errors
        if response.status().is_success() {
            let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
            if let Ok(todo) = serde_json::from_slice::<Todo>(&body) {
                created_ids.push(todo.id);
            }
        } else {
            assert!(response.status().is_client_error(), "Unexpected error for case {}: {}", i, response.status());
        }
    }

    // Test UPDATE with various data on created todos
    for &id in &created_ids {
        let update_test_cases = vec![
            json!({"title": "Updated Title"}),
            json!({"completed": true}),
            json!({"title": "Both Updated", "completed": true}),
            json!({}), // Empty update (should be valid)
            json!({"title": null}), // Null title (might clear or fail)
            json!({"completed": null}), // Null completed (might clear or fail)
            json!({"nonexistent": "field"}), // Non-existent field (should be ignored)
        ];

        for update_data in update_test_cases {
            let request = Request::builder()
                .method("PUT")
                .uri(&format!("/api/v1/todos/{}", id))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&update_data).unwrap()))
                .unwrap();

            let response = app.clone().oneshot(request).await.unwrap();
            // Should succeed or fail gracefully
            assert!(response.status().is_success() || response.status().is_client_error());
        }
    }

    // Test GET on all created todos to exercise different data paths
    for &id in &created_ids {
        let request = Request::builder()
            .method("GET")
            .uri(&format!("/api/v1/todos/{}", id))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert!(response.status().is_success());
    }

    // Test DELETE on created todos
    for &id in &created_ids {
        let request = Request::builder()
            .method("DELETE")
            .uri(&format!("/api/v1/todos/{}", id))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert!(response.status().is_success());
    }
}

/// Test complex query combinations to exercise all code paths
#[tokio::test]
async fn test_comprehensive_query_combinations() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create diverse test data
    let test_data = vec![
        json!({"title": "Alpha Work", "completed": false}),
        json!({"title": "Beta Personal", "completed": true}),
        json!({"title": "Charlie Work", "completed": false}),
        json!({"title": "Delta Personal", "completed": true}),
        json!({"title": "Echo Work", "completed": false}),
    ];

    for todo_data in test_data {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&todo_data).unwrap()))
            .unwrap();
        app.clone().oneshot(request).await.unwrap();
    }

    // Test complex combinations that exercise multiple code paths simultaneously
    let complex_queries = vec![
        // All parameters combined - properly encoded
        format!("filter={}&sort={}&range={}", 
               url_escape::encode_component(r#"{"completed":false}"#),
               url_escape::encode_component(r#"["title","ASC"]"#),
               url_escape::encode_component("[0,2]")),
        format!("filter={}&sort_by=title&order=DESC&page=0&per_page=3",
               url_escape::encode_component(r#"{"title":"Work"}"#)),
        
        // Mixed format combinations - properly encoded
        format!("filter={}&sort_by=title&order=ASC&range={}&page=0&per_page=5",
               url_escape::encode_component(r#"{"completed":true}"#),
               url_escape::encode_component("[0,1]")),
        
        // Invalid combinations (should handle gracefully) - properly encoded
        format!("filter={}&sort={}&range={}&page=abc&per_page=xyz",
               url_escape::encode_component("{invalid}"),
               url_escape::encode_component("[invalid]"),
               url_escape::encode_component("invalid")),
        
        // Empty and null combinations
        "filter=&sort=&range=&page=&per_page=".to_string(),
        format!("filter={}&sort={}&range={}",
               url_escape::encode_component("{}"),
               url_escape::encode_component("[]"),
               url_escape::encode_component("[]")),
        
        // Stress test with many parameters - properly encoded
        format!("filter={}&sort={}&range={}&page=1&per_page=20&order=ASC&sort_by=title&extra=ignored",
               url_escape::encode_component(r#"{"completed":false,"title":"test"}"#),
               url_escape::encode_component(r#"["title","DESC"]"#),
               url_escape::encode_component("[0,10]")),
    ];

    for query in complex_queries {
        let request = Request::builder()
            .method("GET")
            .uri(&format!("/api/v1/todos?{}", query))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        // Should handle all combinations gracefully
        assert!(response.status().is_success() || response.status().is_client_error());
        
        // Verify Content-Range header is present for successful responses
        if response.status().is_success() {
            let headers = response.headers();
            assert!(headers.contains_key("content-range"), "Missing Content-Range header for query: {}", query);
        }
    }
}

/// Test HTTP method variations and edge cases
#[tokio::test]
async fn test_comprehensive_http_methods() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Create a test todo first
    let create_request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"title": "Test Todo", "completed": false}"#))
        .unwrap();
    
    let create_response = app.clone().oneshot(create_request).await.unwrap();
    assert!(create_response.status().is_success());
    
    let body = axum::body::to_bytes(create_response.into_body(), usize::MAX).await.unwrap();
    let created_todo: Todo = serde_json::from_slice(&body).unwrap();

    // Test unsupported HTTP methods (behavior may vary by framework)
    let unsupported_methods = vec!["PATCH", "HEAD", "OPTIONS", "CONNECT", "TRACE"];
    
    for method in unsupported_methods {
        let request = Request::builder()
            .method(method)
            .uri("/api/v1/todos")
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        // Some methods might be handled by Axum differently
        assert!(response.status().is_client_error() || response.status().is_success());
    }

    // Test correct methods on wrong endpoints
    let request = Request::builder()
        .method("DELETE")
        .uri("/api/v1/todos") // DELETE on collection (should be Method Not Allowed)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);

    let request = Request::builder()
        .method("POST")
        .uri(&format!("/api/v1/todos/{}", created_todo.id)) // POST on item (should be Method Not Allowed)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"title": "test"}"#))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

/// Test large payloads and stress scenarios
#[tokio::test]
async fn test_comprehensive_stress_scenarios() {
    let db = setup_test_db().await.expect("Failed to setup test database");
    let app = setup_test_app(db);

    // Test very large titles (within reasonable limits)
    let large_title = "x".repeat(10000);
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/todos")
        .header("content-type", "application/json")
        .body(Body::from(format!(r#"{{"title": "{}", "completed": false}}"#, large_title)))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    // Should either succeed or fail with client error (not server error)
    assert!(response.status().is_success() || response.status().is_client_error());

    // Test many sequential operations (simulate multiple requests)
    for i in 0..10 {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/todos")
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"title": "Sequential Todo {}", "completed": false}}"#, i)))
            .unwrap();
        
        let response = app.clone().oneshot(request).await.unwrap();
        assert!(response.status().is_success(), "Sequential request {} failed", i);
    }

    // Test batch operations by getting all todos
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/todos?per_page=100") // Get many at once
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert!(response.status().is_success());
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let todos: Vec<Todo> = serde_json::from_slice(&body).unwrap();
    assert!(todos.len() >= 10); // Should have at least the sequential todos
}