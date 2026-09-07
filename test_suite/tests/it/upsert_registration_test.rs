//! Registration: find-or-insert on an alternate unique key, reporting what it did.
//!
//! Scenario: a source system re-sends its own content every cycle. The first pass stores the row,
//! a pass carrying the same content changes nothing and says so, and a pass carrying different
//! content updates the row it already registered rather than storing a second one.

use crudcrate::{EntityToModels, UpsertStatus, upsert};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait};
use uuid::Uuid;

pub mod registered_curve {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "registered_curves")]
    #[crudcrate(api_struct = "RegisteredCurve", upsert_key(source_system, source_key))]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable)]
        pub source_system: String,
        #[crudcrate(filterable)]
        pub source_key: String,
        pub slope: f64,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod unregistered_widget {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "unregistered_widgets")]
    #[crudcrate(api_struct = "UnregisteredWidget")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

use registered_curve::{RegisteredCurve, RegisteredCurveCreate};
use unregistered_widget::{UnregisteredWidget, UnregisteredWidgetCreate};

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(registered_curve::Entity, unregistered_widget::Entity).await
}

fn sent(slope: f64) -> RegisteredCurveCreate {
    RegisteredCurveCreate {
        source_system: "cnet".to_string(),
        source_key: "DOC:plate-7".to_string(),
        slope,
    }
}

#[tokio::test]
async fn a_source_registering_its_own_content_twice_stores_one_row() {
    let db = setup_test_db().await.expect("db");

    let (first, status) = upsert::<RegisteredCurve>(&db, sent(1.5)).await.expect("first");
    assert_eq!(status, UpsertStatus::Created);

    let (again, status) = upsert::<RegisteredCurve>(&db, sent(1.5)).await.expect("again");
    assert_eq!(status, UpsertStatus::Unchanged);
    assert_eq!(again.id, first.id);

    let (edited, status) = upsert::<RegisteredCurve>(&db, sent(2.5)).await.expect("edited");
    assert_eq!(status, UpsertStatus::Updated);
    assert_eq!(edited.id, first.id);
    assert!((edited.slope - 2.5).abs() < f64::EPSILON);

    assert_eq!(
        registered_curve::Entity::find().all(&db).await.expect("rows").len(),
        1,
        "three passes over one key are one row"
    );
}

#[tokio::test]
async fn a_resource_declaring_no_key_is_refused_rather_than_guessed() {
    let db = setup_test_db().await.expect("db");
    let sent = UnregisteredWidgetCreate {
        name: "nothing registers this".to_string(),
    };
    assert!(
        upsert::<UnregisteredWidget>(&db, sent).await.is_err(),
        "with no declared key there is nothing to find the row by, so it refuses"
    );
}
