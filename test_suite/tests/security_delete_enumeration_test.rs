//! Issue 4: Batch delete response leaks which submitted UUIDs existed in the DB.
//!
//! The fix routes the response shape through `SecurityProfile::expose_deleted_ids`.
//! `legacy()` (default) preserves the historical array-of-IDs shape. `secure()` (and
//! an explicit override) collapses it to a count, eliminating the existence
//! enumeration side-channel via the batch endpoint.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use crudcrate::SecurityProfile;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

mod common;
use common::{setup_test_app, setup_test_db};

async fn seed_three_customers(app: &axum::Router) -> Vec<Uuid> {
    let mut ids = Vec::new();
    for i in 0..3 {
        let body =
            format!(r#"{{"name":"Cust {i}","email":"c{i}@example.com","is_private":false}}"#);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/customers")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let created: Value = serde_json::from_slice(&bytes).unwrap();
        ids.push(Uuid::parse_str(created["id"].as_str().unwrap()).unwrap());
    }
    ids
}

async fn batch_delete(app: axum::Router, ids: &[Uuid]) -> (StatusCode, Value) {
    let body = serde_json::to_string(ids).unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/customers/batch")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, json)
}

#[tokio::test]
async fn legacy_profile_returns_deleted_id_array() {
    let db = setup_test_db().await.expect("setup");
    // Explicit legacy() — 0.9.0 flipped the default to secure(), so consumers who
    // want the historical ID-array response shape must opt in.
    let app = setup_test_app(&db).layer(axum::Extension(SecurityProfile::legacy()));
    let real_ids = seed_three_customers(&app).await;

    // Mix two real IDs with two fake ones — explicit legacy() profile.
    let mut ids = vec![real_ids[0], real_ids[1]];
    ids.push(Uuid::new_v4());
    ids.push(Uuid::new_v4());

    let (status, body) = batch_delete(app, &ids).await;
    assert_eq!(status, StatusCode::OK);
    // Legacy: array of the IDs that actually existed. Existence leaks.
    let returned: Vec<Uuid> = serde_json::from_value(body).expect("array of uuids");
    assert_eq!(returned.len(), 2);
    assert!(returned.contains(&real_ids[0]));
    assert!(returned.contains(&real_ids[1]));
}

#[tokio::test]
async fn secure_profile_returns_only_deleted_count() {
    let db = setup_test_db().await.expect("setup");
    let app = setup_test_app(&db).layer(axum::Extension(SecurityProfile::secure()));
    let real_ids = seed_three_customers(&app).await;

    let mut ids = vec![real_ids[0], real_ids[1]];
    ids.push(Uuid::new_v4());

    let (status, body) = batch_delete(app, &ids).await;
    assert_eq!(status, StatusCode::OK);
    // Secure: `{"deleted": 2}` — no IDs, no existence side-channel.
    assert_eq!(body, serde_json::json!({"deleted": 2}));
}

#[tokio::test]
async fn struct_update_can_force_id_array_on_top_of_secure() {
    let db = setup_test_db().await.expect("setup");
    let profile = SecurityProfile {
        expose_deleted_ids: true,
        ..SecurityProfile::secure()
    };
    let app = setup_test_app(&db).layer(axum::Extension(profile));
    let real_ids = seed_three_customers(&app).await;

    let (status, body) = batch_delete(app, &real_ids).await;
    assert_eq!(status, StatusCode::OK);
    // Override flips the response back to the array shape even under an otherwise
    // secure profile.
    let returned: Vec<Uuid> = serde_json::from_value(body).expect("array of uuids");
    assert_eq!(returned.len(), 3);
}
