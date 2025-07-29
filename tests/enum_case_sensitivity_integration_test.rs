use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{
    setup_task_app, setup_test_db_with_tasks,
    task_entity::{Model, Priority, Task},
};
use crudcrate::CRUDResource;

/// A wrapper around Task that implements case-sensitive enum filtering
pub struct CaseSensitiveTask(Task);

impl From<Task> for CaseSensitiveTask {
    fn from(task: Task) -> Self {
        CaseSensitiveTask(task)
    }
}

impl From<Model> for CaseSensitiveTask {
    fn from(model: Model) -> Self {
        CaseSensitiveTask(Task::from(model))
    }
}

impl std::ops::Deref for CaseSensitiveTask {
    type Target = Task;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait::async_trait]
impl CRUDResource for CaseSensitiveTask {
    type EntityType = <Task as CRUDResource>::EntityType;
    type ColumnType = <Task as CRUDResource>::ColumnType;
    type ActiveModelType = <Task as CRUDResource>::ActiveModelType;
    type CreateModel = <Task as CRUDResource>::CreateModel;
    type UpdateModel = <Task as CRUDResource>::UpdateModel;

    const ID_COLUMN: Self::ColumnType = <Task as CRUDResource>::ID_COLUMN;
    const RESOURCE_NAME_SINGULAR: &'static str = "case_sensitive_task";
    const RESOURCE_NAME_PLURAL: &'static str = "case_sensitive_tasks";
    const RESOURCE_DESCRIPTION: &'static str = "Task with case-sensitive enum filtering";

    fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        Task::sortable_columns()
    }

    fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        Task::filterable_columns()
    }

    /// Override to enable case-sensitive enum filtering
    fn enum_case_sensitive() -> bool {
        true
    }
}

/// Helper function to filter tasks using case-sensitive wrapper
async fn filter_tasks_case_sensitive(app: &axum::Router, filter_json: &str) -> Vec<Task> {
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

/// Helper function to filter tasks using the default case-insensitive Task
async fn filter_tasks_case_insensitive(app: &axum::Router, filter_json: &str) -> Vec<Task> {
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

async fn create_enum_test_data(app: &axum::Router) {
    let test_tasks = vec![
        json!({
            "title": "High Priority Task",
            "description": "Important task",
            "completed": false,
            "priority": "High",
            "status": "Todo",
            "score": 90.0,
            "points": 100,
            "estimated_hours": 8.0,
            "assignee_count": 2,
            "is_public": true
        }),
        json!({
            "title": "Low Priority Task",
            "description": "Less important task",
            "completed": false,
            "priority": "Low",
            "status": "Todo",
            "score": 30.0,
            "points": 25,
            "estimated_hours": 2.0,
            "assignee_count": 1,
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
}

#[tokio::test]
async fn test_case_insensitive_vs_case_sensitive_enum_filtering() {
    let db = setup_test_db_with_tasks()
        .await
        .expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_enum_test_data(&app).await;

    // Test case-insensitive filtering (default Task behavior)
    // Should find the "High" priority task when searching for "high"
    let case_insensitive_tasks =
        filter_tasks_case_insensitive(&app, r#"{"priority":"high"}"#).await;
    assert_eq!(
        case_insensitive_tasks.len(),
        1,
        "Case-insensitive: should find 1 task with 'high' priority"
    );
    assert_eq!(case_insensitive_tasks[0].priority, Priority::High);

    // Should also work with exact case
    let exact_case_tasks = filter_tasks_case_insensitive(&app, r#"{"priority":"High"}"#).await;
    assert_eq!(
        exact_case_tasks.len(),
        1,
        "Case-insensitive: should find 1 task with 'High' priority"
    );

    // Should work with mixed case
    let mixed_case_tasks = filter_tasks_case_insensitive(&app, r#"{"priority":"HIGH"}"#).await;
    assert_eq!(
        mixed_case_tasks.len(),
        1,
        "Case-insensitive: should find 1 task with 'HIGH' priority"
    );
}

#[tokio::test]
async fn test_trait_method_configuration() {
    // Test that the trait methods return expected values
    assert!(
        !Task::enum_case_sensitive(),
        "Default Task should be case-insensitive"
    );
    assert!(
        CaseSensitiveTask::enum_case_sensitive(),
        "CaseSensitiveTask should be case-sensitive"
    );
}

#[tokio::test]
async fn test_comprehensive_case_scenarios() {
    let db = setup_test_db_with_tasks()
        .await
        .expect("Failed to setup test database");
    let app = setup_task_app(db);

    create_enum_test_data(&app).await;

    // Test various case combinations with case-insensitive filtering
    let test_cases = vec![
        ("high", 1), // lowercase
        ("High", 1), // proper case
        ("HIGH", 1), // uppercase
        ("hIgH", 1), // mixed case
        ("low", 1),  // lowercase
        ("Low", 1),  // proper case
        ("LOW", 1),  // uppercase
    ];

    for (priority_value, expected_count) in test_cases {
        let filter = format!(r#"{{"priority":"{priority_value}"}}"#);
        let tasks = filter_tasks_case_insensitive(&app, &filter).await;

        assert_eq!(
            tasks.len(),
            expected_count,
            "Case-insensitive filter '{}' should find {} tasks, found {}",
            priority_value,
            expected_count,
            tasks.len()
        );
    }
}
