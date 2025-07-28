use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

mod common;
use common::{setup_task_app, setup_test_db_with_tasks, task_entity::Task};

/// Setup function that creates diverse numeric test data and returns the app with data
async fn setup_app_with_numeric_test_data() -> axum::Router {
    let db = setup_test_db_with_tasks()
        .await
        .expect("Failed to setup test database");
    let app = setup_task_app(db);

    // Create diverse numeric test data for comparison operators
    let test_tasks = vec![
        // Task with very low values
        json!({
            "title": "Low Value Task",
            "description": "Task with minimal values",
            "completed": false,
            "priority": "Low",
            "status": "Todo",
            "score": 10.5,           // Low score
            "points": 5,             // Low points
            "estimated_hours": 1.0,  // Low hours
            "assignee_count": 1,     // Min assignees
            "is_public": true
        }),
        // Task with medium values
        json!({
            "title": "Medium Value Task",
            "description": "Task with average values",
            "completed": false,
            "priority": "Medium",
            "status": "InProgress",
            "score": 50.0,           // Medium score
            "points": 50,            // Medium points
            "estimated_hours": 8.0,  // Medium hours
            "assignee_count": 3,     // Medium assignees
            "is_public": true
        }),
        // Task with high values
        json!({
            "title": "High Value Task",
            "description": "Task with maximum values",
            "completed": true,
            "priority": "High",
            "status": "Done",
            "score": 95.7,           // High score
            "points": 100,           // High points
            "estimated_hours": 40.0, // High hours
            "assignee_count": 10,    // Max assignees
            "is_public": false
        }),
        // Task with exact boundary values
        json!({
            "title": "Boundary Value Task",
            "description": "Task with exact test values",
            "completed": true,
            "priority": "Urgent",
            "status": "Done",
            "score": 75.0,           // Exact boundary
            "points": 75,            // Exact boundary
            "estimated_hours": 20.0, // Exact boundary
            "assignee_count": 5,     // Exact boundary
            "is_public": true
        }),
        // Task with zero/null values
        json!({
            "title": "Zero Value Task",
            "description": null,
            "completed": false,
            "priority": "Low",
            "status": "Cancelled",
            "score": 0.0,            // Zero score
            "points": 0,             // Zero points
            "estimated_hours": null, // Null hours
            "assignee_count": 0,     // Zero assignees
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

        let app_clone = app.clone();
        let response = app_clone.oneshot(request).await.unwrap();

        let status = response.status();
        if !status.is_success() {
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            panic!(
                "Task creation failed with status {}: {}",
                status,
                String::from_utf8_lossy(&body)
            );
        }
    }

    app
}

/// Helper function to execute a numeric comparison filter and return tasks
async fn filter_tasks_with_comparison(app: &axum::Router, filter_json: &str) -> Vec<Task> {
    let filter_param = url_escape::encode_component(filter_json);
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/api/v1/tasks?filter={}", filter_param))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

// ===== GREATER THAN OR EQUAL (_gte) ATOMIC TESTS =====

#[tokio::test]
async fn test_filter_score_gte_float_50() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"score_gte\":50.0}").await;

    for task in &tasks {
        assert!(
            task.score >= 50.0,
            "Task '{}' has score {} which is < 50.0",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        3,
        "Should find 3 tasks with score >= 50.0 (Medium=50.0, High=95.7, Boundary=75.0), found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_score_gte_float_25() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"score_gte\":25.0}").await;

    for task in &tasks {
        assert!(
            task.score >= 25.0,
            "Task '{}' has score {} which is < 25.0",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        3,
        "Should find 3 tasks with score >= 25.0, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_points_gte_integer_50() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"points_gte\":50}").await;

    for task in &tasks {
        assert!(
            task.points >= 50,
            "Task '{}' has points {} which is < 50",
            task.title,
            task.points
        );
    }
    assert_eq!(
        tasks.len(),
        3,
        "Should find 3 tasks with points >= 50 (Medium=50, High=100, Boundary=75), found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_points_gte_integer_20() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"points_gte\":20}").await;

    for task in &tasks {
        assert!(
            task.points >= 20,
            "Task '{}' has points {} which is < 20",
            task.title,
            task.points
        );
    }
    assert_eq!(
        tasks.len(),
        3,
        "Should find 3 tasks with points >= 20, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_assignee_count_gte_small_integer_3() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"assignee_count_gte\":3}").await;

    for task in &tasks {
        assert!(
            task.assignee_count >= 3,
            "Task '{}' has assignee_count {} which is < 3",
            task.title,
            task.assignee_count
        );
    }
    assert_eq!(
        tasks.len(),
        3,
        "Should find 3 tasks with assignee_count >= 3 (Medium=3, High=10, Boundary=5), found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_assignee_count_gte_small_integer_5() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"assignee_count_gte\":5}").await;

    for task in &tasks {
        assert!(
            task.assignee_count >= 5,
            "Task '{}' has assignee_count {} which is < 5",
            task.title,
            task.assignee_count
        );
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 tasks with assignee_count >= 5 (High=10, Boundary=5), found {}",
        tasks.len()
    );
}

// ===== LESS THAN OR EQUAL (_lte) ATOMIC TESTS =====

#[tokio::test]
async fn test_filter_score_lte_float_50() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"score_lte\":50.0}").await;

    for task in &tasks {
        assert!(
            task.score <= 50.0,
            "Task '{}' has score {} which is > 50.0",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        3,
        "Should find 3 tasks with score <= 50.0 (Low=10.5, Medium=50.0, Zero=0.0), found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_score_lte_float_75() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"score_lte\":75.0}").await;

    for task in &tasks {
        assert!(
            task.score <= 75.0,
            "Task '{}' has score {} which is > 75.0",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        4,
        "Should find 4 tasks with score <= 75.0, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_points_lte_integer_50() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"points_lte\":50}").await;

    for task in &tasks {
        assert!(
            task.points <= 50,
            "Task '{}' has points {} which is > 50",
            task.title,
            task.points
        );
    }
    assert_eq!(
        tasks.len(),
        3,
        "Should find 3 tasks with points <= 50 (Low=5, Medium=50, Zero=0), found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_points_lte_integer_75() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"points_lte\":75}").await;

    for task in &tasks {
        assert!(
            task.points <= 75,
            "Task '{}' has points {} which is > 75",
            task.title,
            task.points
        );
    }
    assert_eq!(
        tasks.len(),
        4,
        "Should find 4 tasks with points <= 75, found {}",
        tasks.len()
    );
}

// ===== GREATER THAN (_gt) ATOMIC TESTS =====

#[tokio::test]
async fn test_filter_score_gt_float_50() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"score_gt\":50.0}").await;

    for task in &tasks {
        assert!(
            task.score > 50.0,
            "Task '{}' has score {} which is <= 50.0",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 tasks with score > 50.0 (High=95.7, Boundary=75.0), found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_score_gt_float_10() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"score_gt\":10.0}").await;

    for task in &tasks {
        assert!(
            task.score > 10.0,
            "Task '{}' has score {} which is <= 10.0",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        4,
        "Should find 4 tasks with score > 10.0 (Low=10.5, Medium=50.0, High=95.7, Boundary=75.0), found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_points_gt_integer_50() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"points_gt\":50}").await;

    for task in &tasks {
        assert!(
            task.points > 50,
            "Task '{}' has points {} which is <= 50",
            task.title,
            task.points
        );
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 tasks with points > 50 (High=100, Boundary=75), found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_points_gt_integer_5() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"points_gt\":5}").await;

    for task in &tasks {
        assert!(
            task.points > 5,
            "Task '{}' has points {} which is <= 5",
            task.title,
            task.points
        );
    }
    assert_eq!(
        tasks.len(),
        3,
        "Should find 3 tasks with points > 5, found {}",
        tasks.len()
    );
}

// ===== LESS THAN (_lt) ATOMIC TESTS =====

#[tokio::test]
async fn test_filter_score_lt_float_50() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"score_lt\":50.0}").await;

    for task in &tasks {
        assert!(
            task.score < 50.0,
            "Task '{}' has score {} which is >= 50.0",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 tasks with score < 50.0 (Low=10.5, Zero=0.0), found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_score_lt_float_25() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"score_lt\":25.0}").await;

    for task in &tasks {
        assert!(
            task.score < 25.0,
            "Task '{}' has score {} which is >= 25.0",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 tasks with score < 25.0, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_assignee_count_lt_small_integer_3() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"assignee_count_lt\":3}").await;

    for task in &tasks {
        assert!(
            task.assignee_count < 3,
            "Task '{}' has assignee_count {} which is >= 3",
            task.title,
            task.assignee_count
        );
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 tasks with assignee_count < 3 (Low=1, Zero=0), found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_assignee_count_lt_small_integer_5() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"assignee_count_lt\":5}").await;

    for task in &tasks {
        assert!(
            task.assignee_count < 5,
            "Task '{}' has assignee_count {} which is >= 5",
            task.title,
            task.assignee_count
        );
    }
    assert_eq!(
        tasks.len(),
        3,
        "Should find 3 tasks with assignee_count < 5, found {}",
        tasks.len()
    );
}

// ===== NOT EQUAL (_neq) ATOMIC TESTS =====

#[tokio::test]
async fn test_filter_score_neq_float_50() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"score_neq\":50.0}").await;

    for task in &tasks {
        assert!(
            task.score != 50.0,
            "Task '{}' has score {} which equals 50.0",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        4,
        "Should find 4 tasks with score != 50.0, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_score_neq_float_0() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"score_neq\":0.0}").await;

    for task in &tasks {
        assert!(
            task.score != 0.0,
            "Task '{}' has score {} which equals 0.0",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        4,
        "Should find 4 tasks with score != 0.0, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_points_neq_integer_50() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"points_neq\":50}").await;

    for task in &tasks {
        assert!(
            task.points != 50,
            "Task '{}' has points {} which equals 50",
            task.title,
            task.points
        );
    }
    assert_eq!(
        tasks.len(),
        4,
        "Should find 4 tasks with points != 50, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_points_neq_integer_100() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"points_neq\":100}").await;

    for task in &tasks {
        assert!(
            task.points != 100,
            "Task '{}' has points {} which equals 100",
            task.title,
            task.points
        );
    }
    assert_eq!(
        tasks.len(),
        4,
        "Should find 4 tasks with points != 100, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_assignee_count_neq_small_integer_3() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"assignee_count_neq\":3}").await;

    for task in &tasks {
        assert!(
            task.assignee_count != 3,
            "Task '{}' has assignee_count {} which equals 3",
            task.title,
            task.assignee_count
        );
    }
    assert_eq!(
        tasks.len(),
        4,
        "Should find 4 tasks with assignee_count != 3, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_assignee_count_neq_small_integer_10() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"assignee_count_neq\":10}").await;

    for task in &tasks {
        assert!(
            task.assignee_count != 10,
            "Task '{}' has assignee_count {} which equals 10",
            task.title,
            task.assignee_count
        );
    }
    assert_eq!(
        tasks.len(),
        4,
        "Should find 4 tasks with assignee_count != 10, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_completed_neq_boolean_false() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"completed_neq\":false}").await;

    for task in &tasks {
        assert!(
            task.completed,
            "Task '{}' should be completed (completed != false)",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 completed tasks (completed != false), found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_completed_neq_boolean_true() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"completed_neq\":true}").await;

    for task in &tasks {
        assert!(
            !task.completed,
            "Task '{}' should not be completed (completed != true)",
            task.title
        );
    }
    assert_eq!(
        tasks.len(),
        3,
        "Should find 3 incomplete tasks (completed != true), found {}",
        tasks.len()
    );
}

// ===== RANGE COMBINATION TESTS =====

#[tokio::test]
async fn test_filter_score_range_gte_25_lte_75() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"score_gte\":25.0,\"score_lte\":75.0}").await;

    for task in &tasks {
        assert!(
            task.score >= 25.0 && task.score <= 75.0,
            "Task '{}' has score {} which is not between 25.0 and 75.0",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 tasks with score between 25.0 and 75.0, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_points_range_gte_20_lte_80() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"points_gte\":20,\"points_lte\":80}").await;

    for task in &tasks {
        assert!(
            task.points >= 20 && task.points <= 80,
            "Task '{}' has points {} which is not between 20 and 80",
            task.title,
            task.points
        );
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 tasks with points between 20 and 80, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_assignee_count_range_gte_1_lte_5() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks =
        filter_tasks_with_comparison(&app, "{\"assignee_count_gte\":1,\"assignee_count_lte\":5}")
            .await;

    for task in &tasks {
        assert!(
            task.assignee_count >= 1 && task.assignee_count <= 5,
            "Task '{}' has assignee_count {} which is not between 1 and 5",
            task.title,
            task.assignee_count
        );
    }
    assert_eq!(
        tasks.len(),
        3,
        "Should find 3 tasks with assignee_count between 1 and 5, found {}",
        tasks.len()
    );
}

// ===== NULLABLE FIELD COMPARISON TESTS =====

#[tokio::test]
async fn test_filter_estimated_hours_gte_nullable_10() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"estimated_hours_gte\":10.0}").await;

    for task in &tasks {
        match task.estimated_hours {
            Some(hours) => assert!(
                hours >= 10.0,
                "Task '{}' has estimated_hours {} which is < 10.0",
                task.title,
                hours
            ),
            None => panic!(
                "Task '{}' has null estimated_hours, should be filtered out",
                task.title
            ),
        }
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 tasks with estimated_hours >= 10.0 (High=40.0, Boundary=20.0), found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_estimated_hours_lte_nullable_15() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"estimated_hours_lte\":15.0}").await;

    for task in &tasks {
        match task.estimated_hours {
            Some(hours) => assert!(
                hours <= 15.0,
                "Task '{}' has estimated_hours {} which is > 15.0",
                task.title,
                hours
            ),
            None => panic!(
                "Task '{}' has null estimated_hours, should be filtered out",
                task.title
            ),
        }
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 tasks with estimated_hours <= 15.0 (Low=1.0, Medium=8.0), found {}",
        tasks.len()
    );
}

// ===== EDGE CASE TESTS =====

#[tokio::test]
async fn test_filter_score_gt_zero_boundary() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"score_gt\":0.0}").await;

    for task in &tasks {
        assert!(
            task.score > 0.0,
            "Task '{}' has score {} which is <= 0.0",
            task.title,
            task.score
        );
    }
    assert_eq!(
        tasks.len(),
        4,
        "Should find 4 tasks with score > 0.0 (excluding zero), found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_points_gte_zero_boundary() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"points_gte\":0}").await;

    for task in &tasks {
        assert!(
            task.points >= 0,
            "Task '{}' has points {} which is < 0",
            task.title,
            task.points
        );
    }
    assert_eq!(
        tasks.len(),
        5,
        "Should find all 5 tasks with points >= 0, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_invalid_comparison_operator() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"score_invalid\":50.0}").await;

    // Invalid operator should be ignored - returns all tasks or error handled gracefully
    assert!(
        tasks.len() <= 5,
        "Should not return more tasks than exist, found {}",
        tasks.len()
    );
}

// ===== MULTI-OPERATOR COMBINATION TESTS =====

#[tokio::test]
async fn test_filter_multiple_operators_score_gte_points_lte() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(&app, "{\"score_gte\":40.0,\"points_lte\":80}").await;

    for task in &tasks {
        assert!(
            task.score >= 40.0,
            "Task '{}' has score {} which is < 40.0",
            task.title,
            task.score
        );
        assert!(
            task.points <= 80,
            "Task '{}' has points {} which is > 80",
            task.title,
            task.points
        );
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 tasks matching both conditions, found {}",
        tasks.len()
    );
}

#[tokio::test]
async fn test_filter_multiple_operators_complex_combination() {
    let app = setup_app_with_numeric_test_data().await;
    let tasks = filter_tasks_with_comparison(
        &app,
        "{\"score_gte\":40.0,\"points_lte\":80,\"assignee_count_neq\":10}",
    )
    .await;

    for task in &tasks {
        assert!(
            task.score >= 40.0,
            "Task '{}' has score {} which is < 40.0",
            task.title,
            task.score
        );
        assert!(
            task.points <= 80,
            "Task '{}' has points {} which is > 80",
            task.title,
            task.points
        );
        assert!(
            task.assignee_count != 10,
            "Task '{}' has assignee_count {} which equals 10",
            task.title,
            task.assignee_count
        );
    }
    assert_eq!(
        tasks.len(),
        2,
        "Should find 2 tasks matching all three conditions, found {}",
        tasks.len()
    );
}
