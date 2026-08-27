//! A joined filter must resolve to a correlated subquery, not materialise every
//! matching child row and build an `id IN (<one bind per row>)` list. The
//! duplicate-laden IN-list overflows the backend bind-parameter ceiling (`SQLite`
//! 32766, Postgres/MySQL 65535) and 500s. Below that it is a memory-amplification
//! `DoS`.
//!
//! Seeding is done directly on the connection (70k HTTP POSTs would be absurd).
//! `70_000` > 65535 so the pre-fix IN-list overflows on every backend CI runs.

use test_suite as common;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use common::{setup_test_app, setup_test_db, vehicle};

const MATCHING_CHILDREN: usize = 70_000;
const CHUNK: usize = 2_000;

async fn seed_customer(db: &sea_orm::DatabaseConnection, name: &str) -> Uuid {
    let id = Uuid::new_v4();
    common::customer::ActiveModel {
        id: Set(id),
        name: Set(name.to_string()),
        email: Set(format!("{name}@example.com")),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        is_private: Set(false),
    }
    .insert(db)
    .await
    .expect("seed customer");
    id
}

fn vehicle_am(customer_id: Uuid, make: &str, i: usize) -> vehicle::ActiveModel {
    vehicle::ActiveModel {
        id: Set(Uuid::new_v4()),
        customer_id: Set(customer_id),
        make: Set(make.to_string()),
        model: Set("model".to_string()),
        year: Set(2020),
        vin: Set(format!("VIN-{i}")),
        fuel_type: Set(None),
        transmission: Set(None),
        created_at: Set(Utc::now()),
        updated_at: Set(Utc::now()),
        is_private: Set(false),
    }
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

/// A broad joined filter over a parent with tens of thousands of matching children
/// returns exactly the matching parent, without a bind-parameter overflow.
#[tokio::test]
async fn broad_joined_filter_does_not_overflow_bind_params() {
    let db = setup_test_db().await.expect("setup");

    // One customer owns MATCHING_CHILDREN BMWs, another owns a single non-match.
    let bmw_owner = seed_customer(&db, "bmw-owner").await;
    let other = seed_customer(&db, "other").await;
    vehicle::Entity::insert(vehicle_am(other, "Toyota", 0))
        .exec(&db)
        .await
        .expect("seed non-matching vehicle");

    let mut i = 0;
    while i < MATCHING_CHILDREN {
        let end = (i + CHUNK).min(MATCHING_CHILDREN);
        let chunk: Vec<vehicle::ActiveModel> =
            (i..end).map(|n| vehicle_am(bmw_owner, "BMW", n)).collect();
        vehicle::Entity::insert_many(chunk)
            .exec(&db)
            .await
            .expect("seed BMW chunk");
        i = end;
    }

    let app = setup_test_app(&db);
    let (status, body) = get_json(
        &app,
        "/customers?filter=%7B%22vehicles.make%22%3A%22BMW%22%7D",
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "broad joined filter must not 500 on a bind-parameter overflow"
    );
    let arr = body.as_array().expect("array body");
    assert_eq!(arr.len(), 1, "exactly the one BMW-owning customer matches");
    assert_eq!(
        arr[0]["id"].as_str().unwrap(),
        bmw_owner.to_string(),
        "the matched customer is the BMW owner"
    );
}
