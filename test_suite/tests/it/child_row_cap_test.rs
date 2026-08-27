//! `max_child_rows` caps the child rows one join field may load per request
//! and answers 413 when a parent has more.

use axum::http::StatusCode;
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr};
use serde_json::json;
use test_suite::http;
use uuid::Uuid;

pub mod parent {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "crc_parents")]
    #[crudcrate(
        generate_router,
        api_struct = "CrcParent",
        derive_partial_eq,
        max_child_rows = 2
    )]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all, depth = 1))]
        pub children: Vec<super::child::CrcChild>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::child::Entity")]
        Children,
    }

    impl Related<super::child::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Children.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod child {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "crc_children")]
    #[crudcrate(generate_router, api_struct = "CrcChild", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable)]
        pub crc_parent_id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub label: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::parent::Entity",
            from = "Column::CrcParentId",
            to = "super::parent::Column::Id"
        )]
        Parent,
    }

    impl Related<super::parent::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Parent.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(parent::Entity, child::Entity).await
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new()
        .nest("/parents", parent::CrcParent::router(db).into())
        .nest("/children", child::CrcChild::router(db).into())
}

async fn create_parent_with_children(app: &axum::Router, name: &str, children: usize) -> String {
    let (status, body) = http::post(app, "/parents", &json!({"name": name})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let parent_id = body["id"].as_str().unwrap().to_string();
    for n in 0..children {
        let (status, body) = http::post(
            app,
            "/children",
            &json!({"crc_parent_id": parent_id, "label": format!("{name}-{n}")}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    parent_id
}

#[test]
fn attribute_sets_the_profile_field() {
    assert_eq!(
        parent::CrcParent::security_profile().max_child_rows_per_relation,
        Some(2)
    );
    assert_eq!(
        child::CrcChild::security_profile().max_child_rows_per_relation,
        None
    );
}

#[tokio::test]
async fn children_within_cap_are_loaded() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);
    let id = create_parent_with_children(&app, "small", 2).await;

    let (status, body) = http::get(&app, &format!("/parents/{id}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["children"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn get_one_over_cap_returns_413() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);
    let id = create_parent_with_children(&app, "big", 3).await;

    let (status, body) = http::get(&app, &format!("/parents/{id}")).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{body}");
    assert!(body.to_string().contains("children"), "{body}");
}

#[tokio::test]
async fn get_all_over_cap_returns_413() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);
    create_parent_with_children(&app, "a", 2).await;
    create_parent_with_children(&app, "b", 1).await;

    let (status, body) = http::get(&app, "/parents").await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{body}");
}

#[tokio::test]
async fn get_all_within_cap_loads_children() {
    let db = setup_test_db().await.unwrap();
    let app = app(&db);
    create_parent_with_children(&app, "a", 1).await;
    create_parent_with_children(&app, "b", 1).await;

    let (status, body) = http::get(&app, "/parents").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let total: usize = body
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["children"].as_array().unwrap().len())
        .sum();
    assert_eq!(total, 2);
}
