//! Field-type round-trip coverage for crudcrate-derive `type_resolution` + model codegen.
//!
//! Exercises varied column types (Uuid PK, required String, nullable `Option<String>`,
//! integer, bool-with-default, two timestamp columns) through the generated Create /
//! Update / List / Response models and the HTTP router. Focuses on:
//!   - `create.rs`: `on_create` defaults, `Option<T>` create handling, bool default
//!   - `update.rs`: three-way `Option<Option<T>>` field shape in the Update model
//!   - `merge.rs`: `Some(Some)` / `Some(None)` / `None` merge semantics
//!   - `list_response.rs`: null vs value serialisation for `Option` columns
//!
//! Self-contained: defines its own entity, schema, and router with the `ftr` slug.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use chrono::{DateTime, Utc};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Table;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

pub mod gadget {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ftr_gadgets")]
    #[crudcrate(generate_router, api_struct = "FtrGadget")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        // Nullable column: present in Create/Update as Option, in responses as null when absent.
        #[crudcrate(filterable, sortable)]
        pub nickname: Option<String>,

        #[crudcrate(filterable, sortable)]
        pub quantity: i32,

        // Bool with a default supplied via on_create so the field can be omitted on create.
        #[crudcrate(filterable, on_create = true)]
        pub active: bool,

        #[crudcrate(exclude(create, update), on_create = Utc::now(), sortable)]
        pub created_at: DateTime<Utc>,

        #[crudcrate(
            exclude(create, update),
            on_create = Utc::now(),
            on_update = Utc::now(),
            sortable
        )]
        pub updated_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

use gadget::{Entity as GadgetEntity, FtrGadget, FtrGadgetCreate, FtrGadgetUpdate};

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".to_string());
    let db = Database::connect(&url).await?;
    let backend = db.get_database_backend();
    let schema = Schema::new(backend);

    // Persistent backends (Postgres/MySQL) keep tables across tests within a binary;
    // drop first so every test starts from a clean schema and empty data. On
    // sqlite::memory: each connection is a fresh database, so the drops are no-ops.
    db.execute(&Table::drop().table(GadgetEntity).if_exists().to_owned())
        .await?;
    db.execute(&schema.create_table_from_entity(GadgetEntity))
        .await?;
    Ok(db)
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/gadgets", FtrGadget::router(db).into())
}

async fn post_create(
    db: &DatabaseConnection,
    payload: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let resp = app(db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/gadgets")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

async fn get_one_http(db: &DatabaseConnection, id: &str) -> (StatusCode, serde_json::Value) {
    let resp = app(db)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/gadgets/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

async fn patch_update(
    db: &DatabaseConnection,
    id: &str,
    payload: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let resp = app(db)
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/gadgets/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

// ============================================================================
// Test 1 — nullable column: absent on create serialises as null; present is stored.
// ============================================================================

#[tokio::test]
async fn create_without_nullable_field_yields_null() {
    let db = setup_test_db().await.unwrap();

    let (status, created) = post_create(&db, json!({ "name": "Alpha", "quantity": 3 })).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["name"], "Alpha");
    assert!(
        created.get("nickname").is_some(),
        "nickname key must be present"
    );
    assert!(
        created["nickname"].is_null(),
        "omitted nullable column serialises as JSON null"
    );

    // Confirm the persisted record round-trips as null through get_one.
    let id = created["id"].as_str().unwrap();
    let (status, fetched) = get_one_http(&db, id).await;
    assert_eq!(status, StatusCode::OK);
    assert!(fetched["nickname"].is_null());
}

#[tokio::test]
async fn create_with_nullable_field_stores_value() {
    let db = setup_test_db().await.unwrap();

    let (status, created) = post_create(
        &db,
        json!({ "name": "Beta", "quantity": 1, "nickname": "Bee" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(created["nickname"], "Bee");

    let id = created["id"].as_str().unwrap();
    let (_, fetched) = get_one_http(&db, id).await;
    assert_eq!(fetched["nickname"], "Bee");
}

#[tokio::test]
async fn create_with_explicit_null_nickname_is_null() {
    // The Create model serialises Option<String> with serde default; explicit JSON null
    // deserialises to None and is stored as NULL.
    let db = setup_test_db().await.unwrap();

    let (status, created) = post_create(
        &db,
        json!({ "name": "Gamma", "quantity": 0, "nickname": serde_json::Value::Null }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(created["nickname"].is_null());
}

// ============================================================================
// Test 2 — three-way Option semantics in the Update model.
// ============================================================================

#[tokio::test]
async fn update_setting_nullable_to_new_value_updates_it() {
    let db = setup_test_db().await.unwrap();
    let (_, created) = post_create(
        &db,
        json!({ "name": "Delta", "quantity": 5, "nickname": "Old" }),
    )
    .await;
    let id = created["id"].as_str().unwrap();

    let (status, updated) = patch_update(&db, id, json!({ "nickname": "New" })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["nickname"], "New");
}

#[tokio::test]
async fn update_omitting_nullable_leaves_it_unchanged() {
    // Some(None)/None three-way distinction: omitting the key maps to NotSet, so the
    // stored value survives an update that only touches another field.
    let db = setup_test_db().await.unwrap();
    let (_, created) = post_create(
        &db,
        json!({ "name": "Epsilon", "quantity": 7, "nickname": "Keep" }),
    )
    .await;
    let id = created["id"].as_str().unwrap();

    let (status, updated) = patch_update(&db, id, json!({ "name": "Epsilon-renamed" })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["name"], "Epsilon-renamed");
    assert_eq!(
        updated["nickname"], "Keep",
        "omitting nickname (None -> NotSet) leaves the existing value intact"
    );
}

#[tokio::test]
async fn update_setting_nullable_to_json_null_clears_it() {
    // Actual behaviour: for an Option<T> column, JSON null deserialises (via double_option)
    // to Some(None), which merge.rs maps to ActiveValue::Set(None) — i.e. the value is CLEARED,
    // not rejected. This documents the actual three-way semantics for nullable columns.
    let db = setup_test_db().await.unwrap();
    let (_, created) = post_create(
        &db,
        json!({ "name": "Zeta", "quantity": 2, "nickname": "WillClear" }),
    )
    .await;
    let id = created["id"].as_str().unwrap();

    let (status, updated) =
        patch_update(&db, id, json!({ "nickname": serde_json::Value::Null })).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        updated["nickname"].is_null(),
        "JSON null on a nullable column clears it (Some(None) -> Set(None))"
    );

    // Persisted clear survives a re-fetch.
    let (_, fetched) = get_one_http(&db, id).await;
    assert!(fetched["nickname"].is_null());
}

#[tokio::test]
async fn update_model_field_shapes_are_double_option_and_required_inner() {
    // Drive the generated Update struct directly to prove field shapes:
    //   - nickname is Option<Option<String>>  (nullable column)
    //   - name / quantity / active are Option<Option<T>> too (all included update fields use
    //     the double_option wrapper regardless of column nullability).
    let db = setup_test_db().await.unwrap();
    let created = FtrGadget::create(
        &db,
        FtrGadgetCreate {
            name: "Eta".to_string(),
            nickname: Some("orig".to_string()),
            quantity: 9,
            active: Some(true),
        },
    )
    .await
    .unwrap();

    // Omit everything -> all NotSet -> unchanged record.
    let unchanged = FtrGadget::update(
        &db,
        created.id,
        FtrGadgetUpdate {
            name: None,
            nickname: None,
            quantity: None,
            active: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(unchanged.name, "Eta");
    assert_eq!(unchanged.nickname.as_deref(), Some("orig"));
    assert_eq!(unchanged.quantity, 9);

    // Some(Some(_)) on nickname updates; Some(None) on a later call clears.
    let set_new = FtrGadget::update(
        &db,
        created.id,
        FtrGadgetUpdate {
            name: None,
            nickname: Some(Some("changed".to_string())),
            quantity: None,
            active: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(set_new.nickname.as_deref(), Some("changed"));

    let cleared = FtrGadget::update(
        &db,
        created.id,
        FtrGadgetUpdate {
            name: None,
            nickname: Some(None),
            quantity: None,
            active: None,
        },
    )
    .await
    .unwrap();
    assert!(
        cleared.nickname.is_none(),
        "Some(None) clears the Option column"
    );
}

// ============================================================================
// Test 3 — timestamp on_create / on_update behaviour.
// ============================================================================

#[tokio::test]
async fn created_at_stable_updated_at_advances() {
    let db = setup_test_db().await.unwrap();
    let (_, created) = post_create(&db, json!({ "name": "Theta", "quantity": 4 })).await;
    let id = created["id"].as_str().unwrap();

    let created_at_initial = created["created_at"].as_str().unwrap().to_string();
    let updated_at_initial = created["updated_at"].as_str().unwrap().to_string();
    assert!(
        !created_at_initial.is_empty(),
        "created_at populated on create"
    );

    let parsed_created: DateTime<Utc> = created_at_initial.parse().unwrap();
    let parsed_updated: DateTime<Utc> = updated_at_initial.parse().unwrap();
    // Both timestamps are set from on_create expressions at insert time.
    assert!((parsed_created - parsed_updated).num_seconds().abs() <= 1);

    // Ensure the update lands in a later whole second than the create. MySQL's
    // default TIMESTAMP/DATETIME columns have one-second resolution (no fractional
    // seconds), so a sub-second gap would round to the same value; sleeping past a
    // full second guarantees updated_at advances on every backend.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    let (status, updated) = patch_update(&db, id, json!({ "name": "Theta-2" })).await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(
        updated["created_at"].as_str().unwrap(),
        created_at_initial,
        "created_at is excluded from update and stays put"
    );

    let updated_at_after: DateTime<Utc> = updated["updated_at"].as_str().unwrap().parse().unwrap();
    assert!(
        updated_at_after > parsed_updated,
        "on_update advances updated_at (before={parsed_updated}, after={updated_at_after})"
    );
}

#[tokio::test]
async fn timestamps_present_in_both_list_and_detail() {
    let db = setup_test_db().await.unwrap();
    post_create(&db, json!({ "name": "Iota", "quantity": 1 })).await;

    let resp = app(&db)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/gadgets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(items.len(), 1);
    // Neither timestamp uses exclude(list), so both appear in the List model.
    assert!(items[0].get("created_at").is_some());
    assert!(items[0].get("updated_at").is_some());
    assert!(items[0].get("nickname").is_some());
}

// ============================================================================
// Test 4 — integer and bool round-trip through create / get / update.
// ============================================================================

#[tokio::test]
async fn integer_round_trips_through_create_get_update() {
    let db = setup_test_db().await.unwrap();
    let (_, created) = post_create(&db, json!({ "name": "Kappa", "quantity": 42 })).await;
    assert_eq!(created["quantity"], 42);
    let id = created["id"].as_str().unwrap();

    let (_, fetched) = get_one_http(&db, id).await;
    assert_eq!(fetched["quantity"], 42);

    let (status, updated) = patch_update(&db, id, json!({ "quantity": -7 })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["quantity"], -7);
}

#[tokio::test]
async fn bool_default_applies_when_omitted_and_round_trips_when_set() {
    let db = setup_test_db().await.unwrap();

    // Omitted -> on_create = true default applies.
    let (_, defaulted) = post_create(&db, json!({ "name": "Lambda", "quantity": 0 })).await;
    assert_eq!(
        defaulted["active"], true,
        "on_create=true supplies the bool default"
    );

    // Explicit false on create is honoured.
    let (_, explicit) =
        post_create(&db, json!({ "name": "Mu", "quantity": 0, "active": false })).await;
    assert_eq!(explicit["active"], false);

    // Toggle via update round-trips.
    let id = explicit["id"].as_str().unwrap();
    let (status, toggled) = patch_update(&db, id, json!({ "active": true })).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(toggled["active"], true);

    let (_, refetched) = get_one_http(&db, id).await;
    assert_eq!(refetched["active"], true);
}

#[tokio::test]
async fn direct_trait_create_supplies_defaults_and_persists_types() {
    // Exercise the generated CreateModel struct fields directly: required String + i32 are
    // plain, the nullable Option is Option<String>, the on_create bool is Option<bool>.
    let db = setup_test_db().await.unwrap();

    let created = FtrGadget::create(
        &db,
        FtrGadgetCreate {
            name: "Nu".to_string(),
            nickname: None,
            quantity: 100,
            active: None, // -> on_create default true
        },
    )
    .await
    .unwrap();

    assert_eq!(created.name, "Nu");
    assert!(created.nickname.is_none());
    assert_eq!(created.quantity, 100);
    assert!(created.active, "active defaults to true via on_create");

    let fetched = FtrGadget::get_one(&db, created.id).await.unwrap();
    assert_eq!(fetched.quantity, 100);
    assert!(fetched.active);
}
