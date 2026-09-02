//! Coverage for the `get_all` (LIST) BATCH join-loading path on an
//! `Option<Child>` `belongs_to` field.
//!
//! For a `belongs_to` like `Widget.owner_id -> Owner` (field
//! `owner: Option<ObjOwner>` on Widget), the FK lives on the PARENT (Widget)
//! row, not the child. The batch loader in
//! `crudcrate-derive/src/codegen/joins/loading.rs` must resolve this direction
//! correctly so that a LIST of widgets populates each widget's `owner` from its
//! own `owner_id`. The `sea_orm` `find_related()` call resolves `belongs_to`
//! vs `has_one` from the relation definition, so the orphan widget
//! (`owner_id = None`) must
//! come back with `owner = null` while the two widgets pointing at the seeded
//! owner carry that owner.
//!
//! `owner` is declared `join(one, all, depth = 1)`, exercising both the
//! `get_all` batch loader and the `get_one` single-item loader; the two must
//! agree.

use axum::http::StatusCode;
use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr};
use test_suite::http;
use uuid::Uuid;

pub mod obj_owner {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "obj_owners")]
    #[crudcrate(generate_router, api_struct = "ObjOwner", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::obj_widget::Entity")]
        Widgets,
    }

    impl Related<super::obj_widget::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Widgets.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod obj_widget {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "obj_widgets")]
    #[crudcrate(generate_router, api_struct = "ObjWidget", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        // Nullable belongs_to FK: a widget may have no owner.
        #[crudcrate(filterable)]
        pub owner_id: Option<Uuid>,

        // belongs_to parent: FK (owner_id) is on THIS row, so the loader must
        // resolve via find_related, not by filtering Owner.id by widget ids.
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all, depth = 1))]
        pub owner: Option<super::obj_owner::ObjOwner>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::obj_owner::Entity",
            from = "Column::OwnerId",
            to = "super::obj_owner::Column::Id"
        )]
        Owner,
    }

    impl Related<super::obj_owner::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Owner.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(obj_owner::Entity, obj_widget::Entity).await
}

fn app(db: &DatabaseConnection) -> axum::Router {
    axum::Router::new()
        .nest("/owners", obj_owner::ObjOwner::router(db).into())
        .nest("/widgets", obj_widget::ObjWidget::router(db).into())
}

/// Seed one owner, two widgets pointing at it, and one orphan widget with no
/// owner. Returns `(owner_id, owned_widget_ids, orphan_widget_id)`.
async fn seed(db: &DatabaseConnection) -> (Uuid, Vec<Uuid>, Uuid) {
    let owner = obj_owner::ObjOwner::create(
        db,
        obj_owner::ObjOwnerCreate {
            name: "Acme".to_string(),
        },
    )
    .await
    .expect("create owner");

    let mut widget_ids = Vec::new();
    for n in 0..2 {
        let widget = obj_widget::ObjWidget::create(
            db,
            obj_widget::ObjWidgetCreate {
                name: format!("widget-{n}"),
                owner_id: Some(owner.id),
            },
        )
        .await
        .expect("create owned widget");
        widget_ids.push(widget.id);
    }

    let orphan = obj_widget::ObjWidget::create(
        db,
        obj_widget::ObjWidgetCreate {
            name: "orphan".to_string(),
            owner_id: None,
        },
    )
    .await
    .expect("create orphan widget");

    (owner.id, widget_ids, orphan.id)
}

/// LIST exercises the batch `get_all` loader. Each widget with an `owner_id`
/// must carry the matching owner; the orphan must come back with `owner = null`.
#[tokio::test]
async fn list_widgets_populates_belongs_to_owner_and_leaves_orphan_null() {
    let db = setup_test_db().await.expect("db setup");
    let (owner_id, widget_ids, orphan) = seed(&db).await;

    let app = app(&db);
    let (status, body) = http::get(&app, "/widgets").await;
    assert_eq!(status, StatusCode::OK);

    let widgets = body.as_array().expect("list response is an array");
    assert_eq!(widgets.len(), 3, "all three widgets returned");

    for id in &widget_ids {
        let row = widgets
            .iter()
            .find(|w| w["id"].as_str() == Some(&id.to_string()))
            .expect("owned widget present in list");
        let owner = &row["owner"];
        assert!(
            owner.is_object(),
            "owned widget's belongs_to owner must be populated in get_all, got {owner}"
        );
        assert_eq!(owner["id"].as_str(), Some(owner_id.to_string().as_str()));
        assert_eq!(owner["name"].as_str(), Some("Acme"));
    }

    let orphan_row = widgets
        .iter()
        .find(|w| w["id"].as_str() == Some(&orphan.to_string()))
        .expect("orphan widget present in list");
    assert!(
        orphan_row["owner"].is_null(),
        "orphan widget (owner_id = None) must have owner = null in get_all, got {}",
        orphan_row["owner"]
    );
}

/// `get_one` must agree with the LIST shape: owned widgets carry the owner,
/// the orphan has `owner = null`.
#[tokio::test]
async fn get_one_widget_agrees_with_list() {
    let db = setup_test_db().await.expect("db setup");
    let (owner_id, widget_ids, orphan) = seed(&db).await;

    let app = app(&db);

    let (status, widget) = http::get(&app, &format!("/widgets/{}", widget_ids[0])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        widget["owner"]["id"].as_str(),
        Some(owner_id.to_string().as_str()),
        "get_one must populate the belongs_to owner"
    );
    assert_eq!(widget["owner"]["name"].as_str(), Some("Acme"));

    let (status, orphan_widget) = http::get(&app, &format!("/widgets/{orphan}")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        orphan_widget["owner"].is_null(),
        "get_one orphan must have owner = null, got {}",
        orphan_widget["owner"]
    );
}

/// Direct trait call: `ObjWidget::get_all` returns api structs whose typed
/// `owner` field is `Some(..)`/`None` correctly per the widget's own FK.
#[tokio::test]
async fn get_all_trait_call_populates_typed_owner_field() {
    let db = setup_test_db().await.expect("db setup");
    let (owner_id, widget_ids, orphan) = seed(&db).await;

    let widgets = obj_widget::ObjWidget::get_all(
        &db,
        &sea_orm::Condition::all(),
        obj_widget::Column::Name,
        sea_orm::Order::Asc,
        0,
        100,
    )
    .await
    .expect("get_all");

    for id in &widget_ids {
        let w = widgets.iter().find(|w| w.id == *id).expect("owned present");
        let owner = w.owner.as_ref().expect("typed owner populated");
        assert_eq!(owner.id, owner_id);
        assert_eq!(owner.name, "Acme");
    }

    let orphan_w = widgets
        .iter()
        .find(|w| w.id == orphan)
        .expect("orphan present");
    assert!(
        orphan_w.owner.is_none(),
        "orphan widget's typed owner must be None"
    );
}
