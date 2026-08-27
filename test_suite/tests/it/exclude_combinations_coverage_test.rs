// Exclude combinations coverage test.
//
// Exercises crudcrate-derive model codegen (create.rs / update.rs / list.rs /
// response.rs / api_struct.rs) by giving a single entity one field per exclusion
// kind and asserting the shape of the generated Create / Update / List / Response
// structs and the get_one vs get_all HTTP output.
//
// Fields:
//   - create_secret : exclude(create) -> absent from ExcItemCreate
//   - update_locked : exclude(update) -> absent from ExcItemUpdate
//   - detail_hidden : exclude(one)    -> absent from get_one, present in get_all/list
//   - list_hidden   : exclude(list)   -> absent from get_all/list, present in get_one
//   - everywhere    : plain field     -> present in every model and response

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr};
use serde_json::json;
use tower::ServiceExt;
use uuid::Uuid;

pub mod exc_item {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "exc_items")]
    #[crudcrate(generate_router, api_struct = "ExcItem")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        // Present in Create, Update, get_one, get_all/list.
        #[crudcrate(filterable, sortable)]
        pub everywhere: String,

        // exclude(create): not a field on ExcItemCreate. Server-assigned via on_create.
        #[crudcrate(exclude(create), on_create = "server-assigned".to_string())]
        pub create_secret: String,

        // exclude(update): not a field on ExcItemUpdate. Cannot be changed via PUT.
        #[crudcrate(exclude(update))]
        pub update_locked: String,

        // exclude(one): absent from get_one response, present in get_all/list.
        #[crudcrate(exclude(one))]
        pub detail_hidden: String,

        // exclude(list): absent from get_all/list response, present in get_one.
        #[crudcrate(exclude(list))]
        pub list_hidden: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

use exc_item::{ExcItem, ExcItemCreate, ExcItemUpdate};

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(exc_item::Entity).await
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new().nest("/items", ExcItem::router(db).into())
}

/// POST a row, returning the parsed JSON of the created resource.
async fn create_item(db: &DatabaseConnection, body: serde_json::Value) -> serde_json::Value {
    let resp = app(db)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/items")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED, "create should succeed");
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).expect("created body should be JSON object")
}

#[tokio::test]
async fn exclude_create_field_is_absent_from_create_model_and_defaults_on_post() {
    let db = setup_test_db().await.unwrap();

    // Compile-time evidence: ExcItemCreate can be built WITHOUT a `create_secret`
    // field. If exclude(create) failed, this struct literal would not compile.
    let create = ExcItemCreate {
        everywhere: "ev".to_string(),
        update_locked: "lock".to_string(),
        detail_hidden: "hidden-one".to_string(),
        list_hidden: "hidden-list".to_string(),
    };
    assert_eq!(create.everywhere, "ev");

    // Over HTTP, a `create_secret` in the body is ignored (not a Create field).
    let created = create_item(
        &db,
        json!({
            "everywhere": "ev",
            "create_secret": "should-be-ignored",
            "update_locked": "lock",
            "detail_hidden": "hidden-one",
            "list_hidden": "hidden-list"
        }),
    )
    .await;

    let id = created["id"].as_str().expect("id should be a string");
    assert!(!id.is_empty());

    // The POST response is the get_one shape: create_secret takes its on_create
    // value (the body value was ignored) and the exclude(one) field is absent.
    assert_eq!(created["create_secret"], "server-assigned");
    assert_eq!(created["everywhere"], "ev");
    assert!(created.get("detail_hidden").is_none());

    // Round-trip GET on the created id to confirm persistence.
    let resp = app(&db)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/items/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let fetched: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(fetched["id"], created["id"]);
    assert_eq!(fetched["everywhere"], "ev");
}

#[tokio::test]
async fn get_one_omits_exclude_one_field_and_keeps_exclude_list_field() {
    let db = setup_test_db().await.unwrap();

    let created = create_item(
        &db,
        json!({
            "everywhere": "row-a",
            "update_locked": "lock-a",
            "detail_hidden": "secret-detail",
            "list_hidden": "list-only-value"
        }),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    let resp = app(&db)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/items/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let one: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let obj = one.as_object().expect("get_one returns an object");

    // exclude(one): absent from the detail response.
    assert!(
        !obj.contains_key("detail_hidden"),
        "exclude(one) field must be absent from get_one; keys: {:?}",
        obj.keys().collect::<Vec<_>>()
    );

    // exclude(list): PRESENT in the detail response (only hidden from list).
    assert!(
        obj.contains_key("list_hidden"),
        "exclude(list) field must be present in get_one"
    );
    assert_eq!(one["list_hidden"], "list-only-value");

    // Plain and other non-(one) fields remain present.
    assert_eq!(one["everywhere"], "row-a");
    assert!(obj.contains_key("create_secret"));
    assert!(obj.contains_key("update_locked"));
    assert!(obj.contains_key("id"));
}

#[tokio::test]
async fn get_all_omits_exclude_list_field_and_keeps_exclude_one_field() {
    let db = setup_test_db().await.unwrap();

    create_item(
        &db,
        json!({
            "everywhere": "row-list",
            "update_locked": "lock-l",
            "detail_hidden": "shown-in-list",
            "list_hidden": "hidden-in-list"
        }),
    )
    .await;

    let resp = app(&db)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/items")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let list: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(list.len(), 1);
    let item = list[0].as_object().expect("list item is an object");

    // exclude(list): absent from list responses.
    assert!(
        !item.contains_key("list_hidden"),
        "exclude(list) field must be absent from get_all; keys: {:?}",
        item.keys().collect::<Vec<_>>()
    );

    // exclude(one): PRESENT in list responses (only hidden from get_one).
    assert!(
        item.contains_key("detail_hidden"),
        "exclude(one) field must be present in get_all"
    );
    assert_eq!(list[0]["detail_hidden"], "shown-in-list");

    // Plain and other non-(list) fields remain present.
    assert_eq!(list[0]["everywhere"], "row-list");
    assert!(item.contains_key("create_secret"));
    assert!(item.contains_key("update_locked"));
    assert!(item.contains_key("id"));
}

#[tokio::test]
async fn exclude_update_field_is_absent_from_update_model_and_unchanged_via_put() {
    let db = setup_test_db().await.unwrap();

    let created = create_item(
        &db,
        json!({
            "everywhere": "orig",
            "update_locked": "frozen-value",
            "detail_hidden": "d",
            "list_hidden": "l"
        }),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // Compile-time evidence: ExcItemUpdate can be built WITHOUT an `update_locked`
    // field. Update fields are Option<Option<T>> for the double-option semantics.
    let _update = ExcItemUpdate {
        everywhere: Some(Some("changed".to_string())),
        create_secret: Some(Some("cs".to_string())),
        detail_hidden: Some(Some("d2".to_string())),
        list_hidden: Some(Some("l2".to_string())),
    };

    // Over HTTP, sending update_locked is ignored: it is not part of the Update model.
    let resp = app(&db)
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/items/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "everywhere": "changed",
                        "update_locked": "attempted-change",
                        "detail_hidden": "d2",
                        "list_hidden": "l2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    // The editable field changed; update_locked stayed frozen.
    assert_eq!(updated["everywhere"], "changed");
    assert_eq!(
        updated["update_locked"], "frozen-value",
        "exclude(update) field must not change via PUT"
    );
    assert_eq!(updated["id"], created["id"]);
}

#[tokio::test]
async fn list_and_detail_disagree_on_excluded_fields_for_same_row() {
    let db = setup_test_db().await.unwrap();

    let created = create_item(
        &db,
        json!({
            "everywhere": "consistency",
            "update_locked": "lk",
            "detail_hidden": "only-in-list",
            "list_hidden": "only-in-detail"
        }),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();
    let app_ref = app(&db);

    let one_resp = app_ref
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/items/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let one_bytes = to_bytes(one_resp.into_body(), usize::MAX).await.unwrap();
    let one: serde_json::Value = serde_json::from_slice(&one_bytes).unwrap();

    let all_resp = app_ref
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/items")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let all_bytes = to_bytes(all_resp.into_body(), usize::MAX).await.unwrap();
    let all: Vec<serde_json::Value> = serde_json::from_slice(&all_bytes).unwrap();
    let listed = all
        .iter()
        .find(|v| v["id"] == created["id"])
        .expect("row present in list");

    // detail_hidden: only in the list view.
    assert!(one.as_object().unwrap().get("detail_hidden").is_none());
    assert_eq!(listed["detail_hidden"], "only-in-list");

    // list_hidden: only in the detail view.
    assert_eq!(one["list_hidden"], "only-in-detail");
    assert!(listed.as_object().unwrap().get("list_hidden").is_none());

    // everywhere: identical in both shapes.
    assert_eq!(one["everywhere"], "consistency");
    assert_eq!(listed["everywhere"], "consistency");
}

#[tokio::test]
async fn direct_trait_get_one_and_get_all_round_trip_through_full_api_struct() {
    let db = setup_test_db().await.unwrap();

    // Build the full create model directly (exclude(create) field is absent here too)
    // and drive the trait method rather than HTTP.
    let create = ExcItemCreate {
        everywhere: "trait-row".to_string(),
        update_locked: "lk".to_string(),
        detail_hidden: "dh".to_string(),
        list_hidden: "lh".to_string(),
    };
    let created = ExcItem::create(&db, create)
        .await
        .expect("create via trait");

    // The full API struct retains every (non-join) column, including the
    // exclude(create) field populated by its on_create value.
    assert_eq!(created.everywhere, "trait-row");
    assert_eq!(created.create_secret, "server-assigned");
    assert_eq!(created.update_locked, "lk");
    assert_eq!(created.detail_hidden, "dh");
    assert_eq!(created.list_hidden, "lh");

    let fetched = ExcItem::get_one(&db, created.id)
        .await
        .expect("get_one via trait");
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.list_hidden, "lh");

    let all = ExcItem::get_all(
        &db,
        &sea_orm::Condition::all(),
        <ExcItem as CRUDResource>::ID_COLUMN,
        sea_orm::Order::Asc,
        0,
        100,
    )
    .await
    .expect("get_all via trait");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, created.id);
    assert_eq!(all[0].everywhere, "trait-row");
}
