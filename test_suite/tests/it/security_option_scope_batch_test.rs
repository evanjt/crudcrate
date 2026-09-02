//! In a scoped list query, an `Option<Child>` join at depth > 1 must propagate scope
//! into the child's own nested loads, exactly as a `Vec<Child>` join does. The batch
//! loader previously recursed through `get_one` (unscoped) for the Option branch, so
//! a private grandchild of an Option child leaked into a scoped response.
//!
//! Fixture: Gizmo -> Option<Gadget> (depth 2) -> Vec<Widget>. Gadget is public so the
//! scoped fetch returns it, and one of its Widgets is private and must be filtered out.

use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, DbBackend, Order, Schema, Set,
};
use uuid::Uuid;

use crudcrate::traits::CRUDResource;

mod gizmo {
    use crudcrate::EntityToModels;
    use sea_orm::entity::prelude::*;
    use uuid::Uuid;

    use super::gadget::Gadget;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "gizmos")]
    #[crudcrate(api_struct = "Gizmo", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,
        pub name: String,
        pub gadget_id: Uuid,
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all, depth = 2))]
        pub gadget: Option<Gadget>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::gadget::Entity",
            from = "Column::GadgetId",
            to = "super::gadget::Column::Id"
        )]
        Gadget,
    }

    impl Related<super::gadget::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Gadget.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

mod gadget {
    use crudcrate::EntityToModels;
    use sea_orm::entity::prelude::*;
    use uuid::Uuid;

    use super::widget::Widget;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "gadgets")]
    #[crudcrate(api_struct = "Gadget", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,
        pub name: String,
        #[crudcrate(filterable, exclude(scoped, create), on_create = false)]
        pub is_private: bool,
        #[sea_orm(ignore)]
        #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all, depth = 1))]
        pub widgets: Vec<Widget>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "super::widget::Entity")]
        Widgets,
    }

    impl Related<super::widget::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Widgets.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

mod widget {
    use crudcrate::EntityToModels;
    use sea_orm::entity::prelude::*;
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "widgets")]
    #[crudcrate(api_struct = "Widget", derive_partial_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,
        pub gadget_id: Uuid,
        pub name: String,
        #[crudcrate(filterable, exclude(scoped, create), on_create = false)]
        pub is_private: bool,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "super::gadget::Entity",
            from = "Column::GadgetId",
            to = "super::gadget::Column::Id"
        )]
        Gadget,
    }

    impl Related<super::gadget::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Gadget.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

async fn setup() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(DbBackend::Sqlite);
    for stmt in [
        schema.create_table_from_entity(gadget::Entity),
        schema.create_table_from_entity(widget::Entity),
        schema.create_table_from_entity(gizmo::Entity),
    ] {
        db.execute(&stmt).await.unwrap();
    }
    db
}

/// A scoped list of gizmos must not surface the private widget of the gadget.
#[tokio::test]
async fn scoped_option_join_filters_private_grandchildren() {
    let db = setup().await;

    let gadget_id = Uuid::new_v4();
    gadget::ActiveModel {
        id: Set(gadget_id),
        name: Set("public-gadget".to_string()),
        is_private: Set(false),
    }
    .insert(&db)
    .await
    .unwrap();

    gizmo::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set("gizmo".to_string()),
        gadget_id: Set(gadget_id),
    }
    .insert(&db)
    .await
    .unwrap();

    for (name, private) in [("public-widget", false), ("private-widget", true)] {
        widget::ActiveModel {
            id: Set(Uuid::new_v4()),
            gadget_id: Set(gadget_id),
            name: Set(name.to_string()),
            is_private: Set(private),
        }
        .insert(&db)
        .await
        .unwrap();
    }

    let condition = sea_orm::Condition::all();
    let gizmos = gizmo::Gizmo::get_all_scoped(
        &db,
        &condition,
        gizmo::Gizmo::default_index_column(),
        Order::Asc,
        0,
        10,
    )
    .await
    .expect("scoped list");

    assert_eq!(gizmos.len(), 1, "one gizmo");
    let gadget = gizmos[0].gadget.as_ref().expect("gadget loaded");
    assert!(
        gadget.widgets.iter().all(|w| !w.is_private),
        "scoped list must not load the private widget; got {:?}",
        gadget.widgets.iter().map(|w| &w.name).collect::<Vec<_>>()
    );
    assert_eq!(gadget.widgets.len(), 1, "only the public widget is loaded");
}
