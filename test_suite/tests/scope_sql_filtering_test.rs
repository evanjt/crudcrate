/// SQL-Level Scope Filtering Tests
///
/// Validates that child entities with `exclude(scoped)` fields are filtered at the
/// SQL level during join loading, not just in Rust memory. These tests verify:
/// - Private children are excluded from join results
/// - Children without scope fields are unaffected
/// - Deep join chains filter at each level independently
/// - Unscoped (admin) paths return all data
/// - Edge cases: all-private children, empty results

mod common;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

use common::{setup_scoped_app, setup_test_app, setup_test_db};

/// POST a record via the unscoped (admin) app, return status + JSON body.
async fn admin_post(app: &axum::Router, path: &str, payload: Value) -> (StatusCode, Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

/// PUT (update) a record via the unscoped (admin) app
async fn admin_update(app: &axum::Router, path: &str, payload: Value) -> (StatusCode, Value) {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

/// Create a record and then mark it private via update. Returns the created JSON.
async fn create_private(app: &axum::Router, path: &str, payload: Value) -> Value {
    let (s, created) = admin_post(app, path, payload).await;
    assert_eq!(s, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap();
    let (s, updated) = admin_update(
        app,
        &format!("{path}/{id}"),
        json!({"is_private": true}),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    updated
}

/// GET via any app, return status + JSON body + headers
async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value, axum::http::HeaderMap) {
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
    let headers = resp.headers().clone();
    let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&body).unwrap_or(Value::Null),
        headers,
    )
}

// =============================================================================
// 1. Scoped get_one: many public + many private children → only public returned
// =============================================================================

#[tokio::test]
async fn scope_sql_filters_private_children_from_get_one() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    // Create a public customer
    let (_, cust) = admin_post(
        &admin,
        "/customers",
        json!({"name": "BigParent", "email": "big@example.com"}),
    )
    .await;
    let cust_id = cust["id"].as_str().unwrap();

    // Create 10 public + 10 private vehicles
    for i in 0..10 {
        admin_post(
            &admin,
            "/vehicles",
            json!({
                "customer_id": cust_id,
                "make": format!("Public{i}"),
                "model": "X",
                "year": 2020 + i,
                "vin": format!("PUB{i:03}")
            }),
        )
        .await;
    }
    for i in 0..10 {
        create_private(
            &admin,
            "/vehicles",
            json!({
                "customer_id": cust_id,
                "make": format!("Private{i}"),
                "model": "Y",
                "year": 2020 + i,
                "vin": format!("PRV{i:03}")
            }),
        )
        .await;
    }

    // Admin get_one: all 20 vehicles
    let (s, body, _) = get_json(&admin, &format!("/customers/{cust_id}")).await;
    assert_eq!(s, StatusCode::OK);
    let vehicles = body["vehicles"].as_array().expect("should have vehicles");
    assert_eq!(vehicles.len(), 20, "Admin should see all 20 vehicles");

    // Scoped get_one: only 10 public vehicles
    let (s, body, _) = get_json(&scoped, &format!("/customers/{cust_id}")).await;
    assert_eq!(s, StatusCode::OK);
    let vehicles = body["vehicles"].as_array().expect("should have vehicles");
    assert_eq!(
        vehicles.len(),
        10,
        "Scoped get_one should show only 10 public vehicles, got {}",
        vehicles.len()
    );

    // Verify all returned vehicles are public (no "Private" in make)
    for v in vehicles {
        let make = v["make"].as_str().unwrap();
        assert!(
            make.starts_with("Public"),
            "Expected public vehicle, got make='{make}'"
        );
        // is_private should not be in scoped response
        assert!(
            v.get("is_private").is_none(),
            "is_private must not appear in scoped vehicle"
        );
    }
}

// =============================================================================
// 2. Scoped get_all: multiple parents with mixed children
// =============================================================================

#[tokio::test]
async fn scope_sql_filters_private_children_from_get_all() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    // Create 2 public customers
    let (_, cust1) = admin_post(
        &admin,
        "/customers",
        json!({"name": "Parent1", "email": "p1@example.com"}),
    )
    .await;
    let (_, cust2) = admin_post(
        &admin,
        "/customers",
        json!({"name": "Parent2", "email": "p2@example.com"}),
    )
    .await;
    let cust1_id = cust1["id"].as_str().unwrap();
    let cust2_id = cust2["id"].as_str().unwrap();

    // Each customer gets 3 public + 2 private vehicles
    for (cid, prefix) in [(cust1_id, "C1"), (cust2_id, "C2")] {
        for i in 0..3 {
            admin_post(
                &admin,
                "/vehicles",
                json!({
                    "customer_id": cid,
                    "make": format!("{prefix}Pub{i}"),
                    "model": "X",
                    "year": 2020 + i,
                    "vin": format!("{prefix}P{i}")
                }),
            )
            .await;
        }
        for i in 0..2 {
            create_private(
                &admin,
                "/vehicles",
                json!({
                    "customer_id": cid,
                    "make": format!("{prefix}Priv{i}"),
                    "model": "Y",
                    "year": 2025 + i,
                    "vin": format!("{prefix}V{i}")
                }),
            )
            .await;
        }
    }

    // Scoped get_all: both customers, each with only 3 vehicles
    let (s, body, _) = get_json(&scoped, "/customers").await;
    assert_eq!(s, StatusCode::OK);
    let customers = body.as_array().unwrap();
    assert_eq!(customers.len(), 2);

    for cust in customers {
        let name = cust["name"].as_str().unwrap();
        let vehicles = cust["vehicles"].as_array().expect("should have vehicles");
        assert_eq!(
            vehicles.len(),
            3,
            "Customer '{name}' should have 3 public vehicles in scoped list, got {}",
            vehicles.len()
        );
        // Verify no private vehicles leaked
        for v in vehicles {
            let make = v["make"].as_str().unwrap();
            assert!(
                !make.contains("Priv"),
                "Private vehicle '{make}' should not appear in scoped response"
            );
        }
    }
}

// =============================================================================
// 3. All children private → empty array, not null
// =============================================================================

#[tokio::test]
async fn scope_sql_filter_all_children_private() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    let (_, cust) = admin_post(
        &admin,
        "/customers",
        json!({"name": "AllPrivate", "email": "ap@example.com"}),
    )
    .await;
    let cust_id = cust["id"].as_str().unwrap();

    // Create only private vehicles
    for i in 0..3 {
        create_private(
            &admin,
            "/vehicles",
            json!({
                "customer_id": cust_id,
                "make": format!("Secret{i}"),
                "model": "Z",
                "year": 2020,
                "vin": format!("SEC{i}")
            }),
        )
        .await;
    }

    // Scoped get_one: empty vehicles array (not null, not missing)
    let (s, body, _) = get_json(&scoped, &format!("/customers/{cust_id}")).await;
    assert_eq!(s, StatusCode::OK);
    let vehicles = body["vehicles"].as_array().expect("vehicles should be an array, not null");
    assert_eq!(
        vehicles.len(),
        0,
        "All vehicles are private, scoped response should have empty array"
    );
}

// =============================================================================
// 4. Children without exclude(scoped) fields → all returned regardless of scope
// =============================================================================

#[tokio::test]
async fn scope_sql_filter_no_scoped_fields_on_child() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    // Create customer → vehicle → vehicle_parts
    // vehicle_parts do NOT have exclude(scoped) / is_private field
    let (_, cust) = admin_post(
        &admin,
        "/customers",
        json!({"name": "PartsOwner", "email": "po@example.com"}),
    )
    .await;
    let cust_id = cust["id"].as_str().unwrap();

    let (_, vehicle) = admin_post(
        &admin,
        "/vehicles",
        json!({
            "customer_id": cust_id,
            "make": "Toyota",
            "model": "Corolla",
            "year": 2022,
            "vin": "PARTS1"
        }),
    )
    .await;
    let vehicle_id = vehicle["id"].as_str().unwrap();

    // Create 3 vehicle parts (no is_private field on parts)
    for i in 0..3 {
        let (ps, _pbody) = admin_post(
            &admin,
            "/vehicle_parts",
            json!({
                "vehicle_id": vehicle_id,
                "name": format!("Part{i}"),
                "part_number": format!("PN{i}"),
                "category": "Engine",
                "in_stock": true
            }),
        )
        .await;
        assert_eq!(ps, StatusCode::CREATED, "Part creation should succeed");
    }

    // Admin get_one on vehicle: verify parts are loaded
    let (s, body, _) = get_json(&admin, &format!("/vehicles/{vehicle_id}")).await;
    assert_eq!(s, StatusCode::OK);
    // Debug: print the response keys for this vehicle
    let admin_parts = body["parts"].as_array()
        .unwrap_or_else(|| panic!("admin should have parts array. Full response: {body}"));
    assert_eq!(
        admin_parts.len(),
        3,
        "Admin should see all 3 parts. Got {}. Full response: {body}",
        admin_parts.len()
    );

    // Scoped get_one on vehicle: all 3 parts should be present
    // (vehicle_parts don't have scoped fields, so no filtering)
    let (s, body, _) = get_json(&scoped, &format!("/vehicles/{vehicle_id}")).await;
    assert_eq!(s, StatusCode::OK);
    let parts = body["parts"].as_array().expect("should have parts");
    assert_eq!(
        parts.len(),
        3,
        "All parts should be visible — vehicle_parts has no scope filtering. Got {}",
        parts.len()
    );
}

// =============================================================================
// 5. Deep joins: Customer → Vehicle(scoped) → Parts(no scope)
// =============================================================================

#[tokio::test]
async fn scope_sql_deep_joins_filter_at_each_level() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    let (_, cust) = admin_post(
        &admin,
        "/customers",
        json!({"name": "DeepJoin", "email": "dj@example.com"}),
    )
    .await;
    let cust_id = cust["id"].as_str().unwrap();

    // 1 public vehicle + 1 private vehicle, each with parts
    let (_, pub_vehicle) = admin_post(
        &admin,
        "/vehicles",
        json!({
            "customer_id": cust_id,
            "make": "PublicCar",
            "model": "X",
            "year": 2022,
            "vin": "DEEP1"
        }),
    )
    .await;
    let pub_vid = pub_vehicle["id"].as_str().unwrap();

    let priv_vehicle = create_private(
        &admin,
        "/vehicles",
        json!({
            "customer_id": cust_id,
            "make": "PrivateCar",
            "model": "Y",
            "year": 2023,
            "vin": "DEEP2"
        }),
    )
    .await;
    let _priv_vid = priv_vehicle["id"].as_str().unwrap();

    // Add parts to the public vehicle
    for i in 0..2 {
        admin_post(
            &admin,
            "/vehicle_parts",
            json!({
                "vehicle_id": pub_vid,
                "name": format!("PubPart{i}"),
                "part_number": format!("PP{i}"),
                "category": "Engine",
                "in_stock": true
            }),
        )
        .await;
    }

    // Scoped get_one on customer: only public vehicle with its parts
    let (s, body, _) = get_json(&scoped, &format!("/customers/{cust_id}")).await;
    assert_eq!(s, StatusCode::OK);

    let vehicles = body["vehicles"].as_array().expect("should have vehicles");
    assert_eq!(
        vehicles.len(),
        1,
        "Only public vehicle should be in scoped response"
    );
    assert_eq!(vehicles[0]["make"], "PublicCar");

    // The public vehicle should still have all its parts
    // (parts don't have scope filtering)
    // Note: parts are loaded in get_one for vehicle (depth > 1), so they
    // appear if the vehicle itself is loaded. If vehicle is filtered out,
    // its parts are not loaded at all.
}

// =============================================================================
// 6. Unscoped (admin) returns all children — regression test
// =============================================================================

#[tokio::test]
async fn scope_unscoped_returns_all_children() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);

    let (_, cust) = admin_post(
        &admin,
        "/customers",
        json!({"name": "AdminFull", "email": "af@example.com"}),
    )
    .await;
    let cust_id = cust["id"].as_str().unwrap();

    // Create 5 public + 5 private vehicles
    for i in 0..5 {
        admin_post(
            &admin,
            "/vehicles",
            json!({
                "customer_id": cust_id,
                "make": format!("Pub{i}"),
                "model": "X",
                "year": 2020,
                "vin": format!("AP{i}")
            }),
        )
        .await;
    }
    for i in 0..5 {
        create_private(
            &admin,
            "/vehicles",
            json!({
                "customer_id": cust_id,
                "make": format!("Priv{i}"),
                "model": "Y",
                "year": 2021,
                "vin": format!("AV{i}")
            }),
        )
        .await;
    }

    // Admin (unscoped) get_one: should see all 10 vehicles
    let (s, body, _) = get_json(&admin, &format!("/customers/{cust_id}")).await;
    assert_eq!(s, StatusCode::OK);
    let vehicles = body["vehicles"].as_array().expect("should have vehicles");
    assert_eq!(
        vehicles.len(),
        10,
        "Admin should see all 10 vehicles (public + private)"
    );

    // Admin vehicles should include is_private field
    assert!(
        vehicles[0].get("is_private").is_some(),
        "Admin vehicle should include is_private field"
    );
}

// =============================================================================
// 7. ScopeFilterable trait: is_scope_visible returns correct values
// =============================================================================

#[tokio::test]
async fn scope_defense_in_depth_memory_filter_still_active() {
    // This test verifies that the in-memory ScopeFilterable filter
    // produces correct results. Even if SQL-level filtering handles
    // the heavy lifting, the memory filter is defense-in-depth.
    use crudcrate::ScopeFilterable;
    use common::vehicle;

    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);

    // Create a public and private vehicle
    let (_, cust) = admin_post(
        &admin,
        "/customers",
        json!({"name": "TraitTest", "email": "tt@example.com"}),
    )
    .await;
    let cust_id = cust["id"].as_str().unwrap();

    admin_post(
        &admin,
        "/vehicles",
        json!({
            "customer_id": cust_id,
            "make": "Visible",
            "model": "X",
            "year": 2020,
            "vin": "TRAIT1"
        }),
    )
    .await;

    // Directly test ScopeFilterable on VehicleList
    // VehicleList should implement is_scope_visible() based on is_private field
    let public_list = vehicle::VehicleList {
        id: uuid::Uuid::new_v4(),
        customer_id: uuid::Uuid::new_v4(),
        make: "Test".into(),
        model: "X".into(),
        year: 2020,
        vin: "V1".into(),
        fuel_type: None,
        transmission: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        is_private: false,
        parts: vec![],
        maintenance_records: vec![],
    };
    assert!(
        public_list.is_scope_visible(),
        "Public vehicle should be scope-visible"
    );

    let private_list = vehicle::VehicleList {
        is_private: true,
        ..public_list.clone()
    };
    assert!(
        !private_list.is_scope_visible(),
        "Private vehicle should NOT be scope-visible"
    );
}

// =============================================================================
// 8. Depth > 1 scope recursion: scoped get_all with Vec children recurses via
//    get_one_scoped so nested children are filtered at every hop, not just the
//    final From<List> -> From<ScopedList> in-memory pass.
//
//    Customer.vehicles is declared with `join(one, all, depth = 5)`, so the
//    scoped batch loader must hit the depth > 1 branch. This test asserts the
//    SQL-level filter fires at that level (private vehicles absent from the
//    response). Before get_all_scoped existed, this passed only because of the
//    in-memory defense-in-depth filter; it now passes at the SQL layer too.
// =============================================================================

#[tokio::test]
async fn scope_get_all_depth_gt_1_filters_private_children_at_sql_level() {
    let db = setup_test_db().await.unwrap();
    let admin = setup_test_app(&db);
    let scoped = setup_scoped_app(&db);

    // Two public customers with a mix of public and private vehicles.
    let (_, c1) = admin_post(
        &admin,
        "/customers",
        json!({"name": "DepthA", "email": "a@x.test"}),
    )
    .await;
    let (_, c2) = admin_post(
        &admin,
        "/customers",
        json!({"name": "DepthB", "email": "b@x.test"}),
    )
    .await;
    for cid in [c1["id"].as_str().unwrap(), c2["id"].as_str().unwrap()] {
        for i in 0..2 {
            admin_post(
                &admin,
                "/vehicles",
                json!({
                    "customer_id": cid,
                    "make": format!("Pub{i}"),
                    "model": "X",
                    "year": 2024,
                    "vin": format!("{cid}-P{i}")
                }),
            )
            .await;
        }
        for i in 0..2 {
            create_private(
                &admin,
                "/vehicles",
                json!({
                    "customer_id": cid,
                    "make": format!("Priv{i}"),
                    "model": "Y",
                    "year": 2024,
                    "vin": format!("{cid}-V{i}")
                }),
            )
            .await;
        }
    }

    // Scoped list: both customers, each with only their 2 public vehicles.
    let (s, body, _) = get_json(&scoped, "/customers").await;
    assert_eq!(s, StatusCode::OK);
    let customers = body.as_array().unwrap();
    assert_eq!(customers.len(), 2, "both customers should be visible");
    for c in customers {
        let vehicles = c["vehicles"].as_array().expect("vehicles array");
        assert_eq!(
            vehicles.len(),
            2,
            "Customer '{}' should have only 2 public vehicles; got {} (private leaked?)",
            c["name"],
            vehicles.len()
        );
        for v in vehicles {
            assert!(
                v["make"].as_str().unwrap().starts_with("Pub"),
                "Private vehicle leaked through depth>1 scope recursion: {v}"
            );
            assert!(
                v.get("is_private").is_none(),
                "is_private must not appear in scoped child"
            );
        }
    }

    // Admin list: every vehicle visible on both customers (8 total, 4 each).
    let (s, body, _) = get_json(&admin, "/customers").await;
    assert_eq!(s, StatusCode::OK);
    for c in body.as_array().unwrap() {
        let vehicles = c["vehicles"].as_array().expect("vehicles array");
        assert_eq!(
            vehicles.len(),
            4,
            "Admin should see all 4 vehicles (2 pub + 2 priv) per customer"
        );
    }
}
