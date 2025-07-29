mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{setup_task_app, setup_test_db_with_tasks, task_entity::Task};
use serde_json::json;
use tower::ServiceExt;

/// Set up test data with specific scenarios for edge case testing
async fn setup_edge_case_test_data() -> axum::Router {
    let db = setup_test_db_with_tasks()
        .await
        .expect("Failed to setup test database");
    let app = setup_task_app(db);

    let test_tasks = vec![
        json!({
            "title": "Task with Special Characters: @#$%^&*()",
            "description": "Contains special chars: quotes \"hello\" and apostrophes 'world'",
            "completed": false,
            "priority": "High",
            "status": "Todo",
            "score": 100.0,
            "points": 50,
            "estimated_hours": 5.0,
            "assignee_count": 1,
            "is_public": true
        }),
        json!({
            "title": "Empty Description Task",
            "description": "",
            "completed": true,
            "priority": "Low",
            "status": "Done",
            "score": 0.0,
            "points": 0,
            "estimated_hours": null,
            "assignee_count": 0,
            "is_public": false
        }),
        json!({
            "title": "SQL Injection Attempt'; DROP TABLE tasks; --",
            "description": "Testing SQL injection protection",
            "completed": false,
            "priority": "Medium",
            "status": "InProgress",
            "score": 75.5,
            "points": 25,
            "estimated_hours": 3.0,
            "assignee_count": 2,
            "is_public": true
        }),
        json!({
            "title": "Unicode Test: 测试 🚀 émojis",
            "description": "Unicode and emoji support test",
            "completed": false,
            "priority": "Urgent",
            "status": "Todo",
            "score": 90.0,
            "points": 75,
            "estimated_hours": 8.0,
            "assignee_count": 3,
            "is_public": false
        }),
    ];

    for task_data in test_tasks {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/tasks")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&task_data).unwrap()))
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert!(response.status().is_success(), "Failed to create test task");
    }

    app
}

async fn filter_tasks(app: &axum::Router, filter_json: &str) -> (StatusCode, Vec<Task>) {
    let filter_param = url_escape::encode_component(filter_json);
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    
    let tasks = if status == StatusCode::OK {
        serde_json::from_slice(&body).unwrap_or_default()
    } else {
        vec![]
    };

    (status, tasks)
}

#[tokio::test]
async fn test_non_filterable_field_is_ignored() {
    let app = setup_edge_case_test_data().await;

    // Try to filter on 'created_at' which is not in filterable_columns for Task
    let (status, tasks) = filter_tasks(&app, r#"{"created_at":"2024-01-01"}"#).await;
    
    // Should return all tasks since the filter is ignored
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 4, "Non-filterable field filter should be ignored, returning all tasks");
}

#[tokio::test]
async fn test_completely_nonexistent_field_is_ignored() {
    let app = setup_edge_case_test_data().await;

    // Try to filter on a field that doesn't exist at all
    let (status, tasks) = filter_tasks(&app, r#"{"nonexistent_field":"value"}"#).await;
    
    // Should return all tasks since the filter is ignored
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 4, "Nonexistent field filter should be ignored, returning all tasks");
}

#[tokio::test]
async fn test_uuid_values_in_filterable_fields_use_exact_matching() {
    let app = setup_edge_case_test_data().await;

    // Get all tasks to understand the data
    let (_, all_tasks) = filter_tasks(&app, "{}").await;
    
    // Test that UUID strings in the title field (which is filterable) use LIKE by default
    // but only match when the UUID is actually present in the title as a substring
    let (status, tasks) = filter_tasks(&app, r#"{"title":"Empty"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Should find task with 'Empty' in title using LIKE");

    // Test that fake UUID strings in title don't match anything
    let fake_uuid = "12345678-1234-1234-1234-123456789012";
    let (status, tasks) = filter_tasks(&app, &format!(r#"{{"title":"{}"}}"#, fake_uuid)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 0, "Fake UUID in title should not match any tasks");
    
    // Test the `ids` special case for actual UUID filtering (this works even if id is not in filterable_columns)
    let first_task_id = &all_tasks[0].id;
    let uuid_str = first_task_id.to_string();
    let (status, tasks) = filter_tasks(&app, &format!(r#"{{"ids":["{}"]}}"#, uuid_str)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "IDs array filter should work for UUID matching");
    assert_eq!(tasks[0].id, *first_task_id);
}

#[tokio::test]
async fn test_id_field_not_filterable_by_default() {
    let app = setup_edge_case_test_data().await;

    // Get all tasks to find a real UUID
    let (_, all_tasks) = filter_tasks(&app, "{}").await;
    let first_task_id = &all_tasks[0].id;
    let uuid_str = first_task_id.to_string();

    // Test that filtering by 'id' field doesn't work because it's not in filterable_columns
    let (status, tasks) = filter_tasks(&app, &format!(r#"{{"id":"{}"}}"#, uuid_str)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 4, "ID field filter should be ignored, returning all tasks");
}

#[tokio::test]
async fn test_enum_field_filtering_works() {
    let app = setup_edge_case_test_data().await;

    // Test that enum filtering works (priority field is in filterable_columns)
    let (status, tasks) = filter_tasks(&app, r#"{"priority":"High"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Should find exactly one High priority task");
    
    // Test case-insensitive enum matching
    let (status, tasks) = filter_tasks(&app, r#"{"priority":"high"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Enum filtering should be case-insensitive");

    // Test that partial enum values don't work (exact matching for enums)
    let (status, tasks) = filter_tasks(&app, r#"{"priority":"Hi"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 0, "Partial enum values should not match");
}

#[tokio::test]
async fn test_special_characters_in_like_queries() {
    let app = setup_edge_case_test_data().await;

    // Test that special characters work in LIKE queries
    let (status, tasks) = filter_tasks(&app, r#"{"title":"Special Characters"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Should find task with special characters in title");

    // Test that quotes in description work
    let (status, tasks) = filter_tasks(&app, r#"{"description":"quotes"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Should find task with quotes in description");

    // Test Unicode characters
    let (status, tasks) = filter_tasks(&app, r#"{"title":"测试"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Should find task with Unicode characters");

    // Test emoji characters
    let (status, tasks) = filter_tasks(&app, r#"{"title":"🚀"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Should find task with emoji characters");
}

#[tokio::test]
async fn test_sql_injection_protection() {
    let app = setup_edge_case_test_data().await;

    // Test that SQL injection attempts are treated as literal strings
    let (status, tasks) = filter_tasks(&app, r#"{"title":"SQL Injection"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Should find the SQL injection test task");

    // Test that the actual injection string is treated literally
    let (status, tasks) = filter_tasks(&app, r#"{"title":"'; DROP TABLE"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "SQL injection string should be treated as literal text");
}

#[tokio::test]
async fn test_empty_string_filtering() {
    let app = setup_edge_case_test_data().await;

    // Test filtering by empty string (should match fields that are exactly empty)
    let (status, tasks) = filter_tasks(&app, r#"{"description":""}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Should find exactly one task with empty description");
    assert_eq!(tasks[0].description.as_ref().unwrap(), "", "Found task should have empty description");
}

#[tokio::test]
async fn test_null_value_filtering() {
    let app = setup_edge_case_test_data().await;

    // Test filtering by null value for optional fields
    let (status, tasks) = filter_tasks(&app, r#"{"estimated_hours":null}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Should find exactly one task with null estimated_hours");
    assert!(tasks[0].estimated_hours.is_none(), "Found task should have null estimated_hours");
}

#[tokio::test]
async fn test_case_sensitivity_for_like_queries() {
    let app = setup_edge_case_test_data().await;

    // Test case-insensitive LIKE queries (default behavior)
    let (status, tasks) = filter_tasks(&app, r#"{"title":"EMPTY"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "LIKE queries should be case-insensitive by default");

    let (status, tasks) = filter_tasks(&app, r#"{"title":"empty"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "LIKE queries should match different case");
}

#[tokio::test]
async fn test_combined_filterable_and_non_filterable_fields() {
    let app = setup_edge_case_test_data().await;

    // Test combining a filterable field with a non-filterable field
    // The non-filterable field should be ignored, only the filterable field should apply
    let (status, tasks) = filter_tasks(&app, r#"{"title":"Empty","created_at":"2024-01-01"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Should apply only the filterable field filter");
    assert!(tasks[0].title.contains("Empty"), "Should match the filterable field filter");
}

#[tokio::test]
async fn test_multiple_like_fields_in_same_query() {
    let app = setup_edge_case_test_data().await;

    // Test filtering on multiple string fields that both use LIKE
    let (status, tasks) = filter_tasks(&app, r#"{"title":"Unicode","description":"Unicode"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Should find task matching both LIKE conditions");
    assert!(tasks[0].title.contains("Unicode") && tasks[0].description.as_ref().unwrap().contains("Unicode"));
}

#[tokio::test]
async fn test_like_field_with_exact_field_combination() {
    let app = setup_edge_case_test_data().await;

    // Test combining a LIKE field (string) with an exact field (enum)
    let (status, tasks) = filter_tasks(&app, r#"{"title":"Task","priority":"High"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Should find task matching both LIKE and exact conditions");
    assert!(tasks[0].title.contains("Task"));
}

#[tokio::test]
async fn test_eq_suffix_overrides_like_behavior() {
    let app = setup_edge_case_test_data().await;

    // Test that _eq suffix forces exact matching even for string fields
    let (status, tasks) = filter_tasks(&app, r#"{"title_eq":"Empty"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 0, "_eq suffix should require exact match, not finding substring matches");

    // Test exact match with _eq
    let (status, tasks) = filter_tasks(&app, r#"{"title_eq":"Empty Description Task"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "_eq suffix should find exact match");
    assert_eq!(tasks[0].title, "Empty Description Task");
}

#[tokio::test]
async fn test_whitespace_handling_in_filters() {
    let app = setup_edge_case_test_data().await;

    // Test that leading/trailing whitespace is trimmed
    let (status, tasks) = filter_tasks(&app, r#"{"title":"  Empty  "}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Whitespace should be trimmed from filter values");

    // Test filtering by value that contains internal whitespace
    let (status, tasks) = filter_tasks(&app, r#"{"title":"Description Task"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tasks.len(), 1, "Should find task with internal whitespace");
}