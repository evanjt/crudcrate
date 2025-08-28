use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_task_app, setup_test_db_with_tasks, task_entity::Task};

/// Test both substring and exact string filtering patterns
async fn setup_string_test_data() -> axum::Router {
    let db = setup_test_db_with_tasks()
        .await
        .expect("Failed to setup test database");
    let app = setup_task_app(db);

    let test_tasks = vec![
        json!({
            "title": "Alpha Work Project",
            "description": "Complete the Alpha project setup",
            "completed": false,
            "priority": "High",
            "status": "Todo",
            "score": 95.5,
            "points": 100,
            "estimated_hours": 8.0,
            "assignee_count": 2,
            "is_public": true
        }),
        json!({
            "title": "Beta Testing",
            "description": "Test the Beta release features",
            "completed": false,
            "priority": "Medium",
            "status": "InProgress",
            "score": 67.25,
            "points": 75,
            "estimated_hours": 12.0,
            "assignee_count": 3,
            "is_public": false
        }),
        json!({
            "title": "Alpha",
            "description": "Just Alpha with no additional text",
            "completed": true,
            "priority": "Low",
            "status": "Done",
            "score": 50.0,
            "points": 25,
            "estimated_hours": 4.0,
            "assignee_count": 1,
            "is_public": true
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

async fn filter_tasks_by_string(app: &axum::Router, filter_json: &str) -> Vec<Task> {
    let filter_param = url_escape::encode_component(filter_json);
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tasks?filter={filter_param}"))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn test_string_substring_matching_default() {
    let app = setup_string_test_data().await;

    // Test default substring matching: {"title": "Alpha"}
    let tasks = filter_tasks_by_string(&app, "{\"title\":\"Alpha\"}").await;

    // Should find both "Alpha Work Project" and "Alpha" (substring match)
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 tasks containing 'Alpha' in title, found {}",
        tasks.len()
    );

    for task in &tasks {
        assert!(
            task.title.contains("Alpha"),
            "Task '{}' should contain 'Alpha'",
            task.title
        );
    }
}

#[tokio::test]
async fn test_string_exact_matching_with_eq() {
    let app = setup_string_test_data().await;

    // Test exact matching with _eq: {"title_eq": "Alpha"}
    let tasks = filter_tasks_by_string(&app, "{\"title_eq\":\"Alpha\"}").await;

    // Should find only the task with title exactly "Alpha"
    assert_eq!(
        tasks.len(),
        1,
        "Should find exactly 1 task with title='Alpha', found {}",
        tasks.len()
    );
    assert_eq!(
        tasks[0].title, "Alpha",
        "Found task should have exact title 'Alpha'"
    );
}

#[tokio::test]
async fn test_string_exact_matching_with_eq_full_title() {
    let app = setup_string_test_data().await;

    // Test exact matching with _eq: {"title_eq": "Alpha Work Project"}
    let tasks = filter_tasks_by_string(&app, "{\"title_eq\":\"Alpha Work Project\"}").await;

    // Should find only the task with the full exact title
    assert_eq!(
        tasks.len(),
        1,
        "Should find exactly 1 task with full title, found {}",
        tasks.len()
    );
    assert_eq!(
        tasks[0].title, "Alpha Work Project",
        "Found task should have exact full title"
    );
}

#[tokio::test]
async fn test_string_exact_matching_no_results() {
    let app = setup_string_test_data().await;

    // Test exact matching with _eq for non-existent exact match
    let tasks = filter_tasks_by_string(&app, "{\"title_eq\":\"Nonexistent Title\"}").await;

    // Should find no tasks
    assert_eq!(
        tasks.len(),
        0,
        "Should find no tasks with non-existent exact title, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_description_substring_vs_exact() {
    let app = setup_string_test_data().await;

    // Test substring matching on description
    let substring_tasks = filter_tasks_by_string(&app, "{\"description\":\"Alpha\"}").await;
    assert_eq!(
        substring_tasks.len(),
        2,
        "Substring search should find 2 tasks with 'Alpha' in description"
    );

    // Test exact matching on description
    let exact_tasks = filter_tasks_by_string(
        &app,
        "{\"description_eq\":\"Just Alpha with no additional text\"}",
    )
    .await;
    assert_eq!(
        exact_tasks.len(),
        1,
        "Exact search should find 1 task with exact description"
    );
    assert_eq!(
        exact_tasks[0].description.as_ref().unwrap(),
        "Just Alpha with no additional text"
    );
}

#[tokio::test]
async fn test_enum_fields_still_use_exact_matching() {
    let app = setup_string_test_data().await;

    // Enum fields should always use exact matching, no _eq needed
    let tasks = filter_tasks_by_string(&app, "{\"priority\":\"High\"}").await;

    assert_eq!(tasks.len(), 1, "Should find exactly 1 High priority task");

    // Test that partial enum values don't work (no substring matching for enums)
    let no_tasks = filter_tasks_by_string(&app, "{\"priority\":\"Hi\"}").await;
    assert_eq!(
        no_tasks.len(),
        0,
        "Should find no tasks with partial enum value 'Hi'"
    );
}
