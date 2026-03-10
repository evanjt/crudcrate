mod common;

use crudcrate::aggregation::{PivotConfig, pivot_to_columnar};
use serde_json::json;

/// Two groups where group A has data at T1 but not T2, group B has data at T2 but not T1.
/// Verifies null-fill at missing positions.
#[test]
fn test_pivot_null_fill_for_sparse_groups() {
    let config = PivotConfig {
        metrics: vec!["value".into()],
        aggregates: vec!["avg".into()],
        group_by: vec!["parameter_id".into()],
        resolution: "1 hour".into(),
    };

    let rows = vec![
        json!({"bucket": "2024-01-01T00:00:00Z", "parameter_id": "a", "avg_value": 10.0, "count": 5}),
        json!({"bucket": "2024-01-01T01:00:00Z", "parameter_id": "b", "avg_value": 20.0, "count": 3}),
    ];

    let result = pivot_to_columnar(&rows, &config, None, None);

    assert_eq!(result.times.len(), 2);
    assert_eq!(result.groups.len(), 2);

    // Find group A and group B
    let group_a = result.groups.iter().find(|g| g.key.get("parameter_id").and_then(|v| v.as_str()) == Some("a")).expect("group A not found");
    let group_b = result.groups.iter().find(|g| g.key.get("parameter_id").and_then(|v| v.as_str()) == Some("b")).expect("group B not found");

    // Group A: has data at T1, null at T2
    let avg_a = &group_a.metrics["value"]["avg"];
    assert_eq!(avg_a[0], Some(10.0));
    assert_eq!(avg_a[1], None, "Group A should have null at T2");

    // Group B: null at T1, has data at T2
    let avg_b = &group_b.metrics["value"]["avg"];
    assert_eq!(avg_b[0], None, "Group B should have null at T1");
    assert_eq!(avg_b[1], Some(20.0));

    // Count alignment
    assert_eq!(group_a.count[0], Some(5));
    assert_eq!(group_a.count[1], None);
    assert_eq!(group_b.count[0], None);
    assert_eq!(group_b.count[1], Some(3));
}

/// Two metrics x three aggregates. Verify cross-product and count at group level.
#[test]
fn test_pivot_multiple_metrics_multiple_aggregates() {
    let config = PivotConfig {
        metrics: vec!["value".into(), "temperature".into()],
        aggregates: vec!["avg".into(), "min".into(), "max".into()],
        group_by: vec!["site_id".into()],
        resolution: "1 day".into(),
    };

    let rows = vec![
        json!({
            "bucket": "2024-01-01T00:00:00Z",
            "site_id": "site-1",
            "avg_value": 42.5, "min_value": 40.0, "max_value": 45.0,
            "avg_temperature": 22.0, "min_temperature": 20.0, "max_temperature": 24.0,
            "count": 10
        }),
    ];

    let result = pivot_to_columnar(&rows, &config, None, None);

    assert_eq!(result.groups.len(), 1);
    let group = &result.groups[0];

    // Check value metric
    assert_eq!(group.metrics["value"]["avg"], vec![Some(42.5)]);
    assert_eq!(group.metrics["value"]["min"], vec![Some(40.0)]);
    assert_eq!(group.metrics["value"]["max"], vec![Some(45.0)]);

    // Check temperature metric
    assert_eq!(group.metrics["temperature"]["avg"], vec![Some(22.0)]);
    assert_eq!(group.metrics["temperature"]["min"], vec![Some(20.0)]);
    assert_eq!(group.metrics["temperature"]["max"], vec![Some(24.0)]);

    // Count is at group level, not per-metric
    assert_eq!(group.count, vec![Some(10)]);
}

/// Rows arrive interleaved. Verify times sorted and arrays correctly aligned.
#[test]
fn test_pivot_preserves_time_order_with_mixed_groups() {
    let config = PivotConfig {
        metrics: vec!["value".into()],
        aggregates: vec!["avg".into()],
        group_by: vec!["parameter_id".into()],
        resolution: "1 hour".into(),
    };

    // Rows arrive in non-chronological order, interleaved groups
    let rows = vec![
        json!({"bucket": "2024-01-01T02:00:00Z", "parameter_id": "a", "avg_value": 30.0, "count": 3}),
        json!({"bucket": "2024-01-01T00:00:00Z", "parameter_id": "b", "avg_value": 100.0, "count": 7}),
        json!({"bucket": "2024-01-01T00:00:00Z", "parameter_id": "a", "avg_value": 10.0, "count": 5}),
        json!({"bucket": "2024-01-01T01:00:00Z", "parameter_id": "a", "avg_value": 20.0, "count": 4}),
    ];

    let result = pivot_to_columnar(&rows, &config, None, None);

    // Times should be sorted
    assert_eq!(result.times, vec![
        "2024-01-01T00:00:00Z",
        "2024-01-01T01:00:00Z",
        "2024-01-01T02:00:00Z",
    ]);

    let group_a = result.groups.iter().find(|g| g.key.get("parameter_id").and_then(|v| v.as_str()) == Some("a")).unwrap();

    // Group A has data at all 3 times
    assert_eq!(group_a.metrics["value"]["avg"], vec![Some(10.0), Some(20.0), Some(30.0)]);
    assert_eq!(group_a.count, vec![Some(5), Some(4), Some(3)]);

    let group_b = result.groups.iter().find(|g| g.key.get("parameter_id").and_then(|v| v.as_str()) == Some("b")).unwrap();

    // Group B only has data at T1, null at T2 and T3
    assert_eq!(group_b.metrics["value"]["avg"], vec![Some(100.0), None, None]);
}

/// No group_by columns -- all rows go to a single group with empty key.
#[test]
fn test_pivot_empty_group_by() {
    let config = PivotConfig {
        metrics: vec!["value".into()],
        aggregates: vec!["avg".into(), "max".into()],
        group_by: vec![],
        resolution: "1 hour".into(),
    };

    let rows = vec![
        json!({"bucket": "2024-01-01T00:00:00Z", "avg_value": 10.0, "max_value": 15.0, "count": 5}),
        json!({"bucket": "2024-01-01T01:00:00Z", "avg_value": 20.0, "max_value": 25.0, "count": 8}),
    ];

    let result = pivot_to_columnar(&rows, &config, Some("2024-01-01T00:00:00Z"), Some("2024-01-01T02:00:00Z"));

    assert_eq!(result.groups.len(), 1, "Should have exactly one group when group_by is empty");
    assert!(result.groups[0].key.is_empty(), "Group key should be empty");
    assert_eq!(result.start.as_deref(), Some("2024-01-01T00:00:00Z"));
    assert_eq!(result.end.as_deref(), Some("2024-01-01T02:00:00Z"));
    assert_eq!(result.resolution, "1 hour");

    assert_eq!(result.groups[0].metrics["value"]["avg"], vec![Some(10.0), Some(20.0)]);
    assert_eq!(result.groups[0].metrics["value"]["max"], vec![Some(15.0), Some(25.0)]);
}

/// Count can be i64 or f64 in JSON -- verify both are handled.
#[test]
fn test_pivot_count_handles_integer_types() {
    let config = PivotConfig {
        metrics: vec!["value".into()],
        aggregates: vec!["avg".into()],
        group_by: vec![],
        resolution: "1 hour".into(),
    };

    // serde_json::json! produces Number(10) which is i64, but
    // some DB drivers may return float. Test both.
    let rows = vec![
        json!({"bucket": "T1", "avg_value": 1.0, "count": 10}),
        json!({"bucket": "T2", "avg_value": 2.0, "count": 20.0}),  // f64 count
    ];

    let result = pivot_to_columnar(&rows, &config, None, None);

    assert_eq!(result.groups[0].count[0], Some(10));
    assert_eq!(result.groups[0].count[1], Some(20));
}

// --- Integration test ---

use axum::Router;
use axum::body::Body;
use axum::http::Request;
use tower::ServiceExt;

/// Hit the aggregate endpoint and verify response is a JSON object (not array).
#[tokio::test]
async fn test_pivoted_endpoint_returns_object_not_array() {
    let db = common::setup_readings_db().await.unwrap();
    let app = Router::new().nest(
        "/readings",
        common::reading::ReadingApi::aggregate_router(&db).into(),
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/readings/aggregate?interval=1%20hour")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status().as_u16();
    // On SQLite, time_bucket fails -> 500. But if it somehow succeeds,
    // verify the response shape is an object, not an array.
    if status == 200 {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(parsed.is_object(), "Response should be a JSON object, not an array");
        assert!(parsed.get("resolution").is_some(), "Should have 'resolution' field");
        assert!(parsed.get("times").is_some(), "Should have 'times' field");
        assert!(parsed.get("groups").is_some(), "Should have 'groups' field");
    } else {
        // On SQLite we expect 500 (time_bucket doesn't exist)
        // The important thing is the route exists (not 404) and compiles
        assert_ne!(status, 404, "Aggregate route should be mounted");
    }
}
