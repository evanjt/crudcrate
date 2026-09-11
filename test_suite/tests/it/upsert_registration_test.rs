//! Registration: find-or-insert on an alternate unique key, reporting what it did.
//!
//! Scenario: a source system re-sends its own content every cycle. The first pass stores the row,
//! a pass carrying the same content changes nothing and says so, and a pass carrying different
//! content updates the row it already registered rather than storing a second one.

use crudcrate::{EntityToModels, UpsertStatus, upsert};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait, Set, TransactionTrait,
};
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

        #[crudcrate(filterable, exclude(create, update))]
        pub source_system: String,
        #[crudcrate(filterable, exclude(create, update))]
        pub source_key: String,
        pub slope: f64,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// The common shape of a registered table: the source sends content, the entity keeps the
/// timestamps. Neither timestamp is content, so neither decides whether content changed.
pub mod stamped_curve {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "stamped_curves")]
    #[crudcrate(api_struct = "StampedCurve", upsert_key(source_system, source_key))]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable)]
        pub source_system: String,
        #[crudcrate(filterable)]
        pub source_key: String,
        pub slope: f64,

        #[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
        pub created_at: chrono::DateTime<chrono::Utc>,
        #[crudcrate(
            exclude(create, update),
            on_create = chrono::Utc::now(),
            on_update = chrono::Utc::now()
        )]
        pub updated_at: chrono::DateTime<chrono::Utc>,
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
use stamped_curve::{StampedCurve, StampedCurveCreate};
use unregistered_widget::{UnregisteredWidget, UnregisteredWidgetCreate};

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(
        registered_curve::Entity,
        unregistered_widget::Entity,
        stamped_curve::Entity
    )
    .await
}

fn stamped(slope: f64) -> StampedCurveCreate {
    StampedCurveCreate {
        source_system: "cnet".to_string(),
        source_key: "DOC:plate-7".to_string(),
        slope,
    }
}

fn sent(slope: f64) -> registered_curve::ActiveModel {
    let mut active: registered_curve::ActiveModel = RegisteredCurveCreate { slope }.into();
    active.source_system = Set("cnet".to_string());
    active.source_key = Set("DOC:plate-7".to_string());
    active
}

#[tokio::test]
async fn a_source_registering_its_own_content_twice_stores_one_row() {
    let db = setup_test_db().await.expect("db");

    let (first, status) = upsert::<RegisteredCurve, _>(&db, sent(1.5))
        .await
        .expect("first");
    assert_eq!(status, UpsertStatus::Created);

    let (again, status) = upsert::<RegisteredCurve, _>(&db, sent(1.5))
        .await
        .expect("again");
    assert_eq!(status, UpsertStatus::Unchanged);
    assert_eq!(again.id, first.id);

    let (edited, status) = upsert::<RegisteredCurve, _>(&db, sent(2.5))
        .await
        .expect("edited");
    assert_eq!(status, UpsertStatus::Updated);
    assert_eq!(edited.id, first.id);
    assert!((edited.slope - 2.5).abs() < f64::EPSILON);

    assert_eq!(
        registered_curve::Entity::find()
            .all(&db)
            .await
            .expect("rows")
            .len(),
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
        upsert::<UnregisteredWidget, _>(&db, sent.into())
            .await
            .is_err(),
        "with no declared key there is nothing to find the row by, so it refuses"
    );
}

#[tokio::test]
async fn a_timestamped_row_resent_unchanged_keeps_its_timestamps() {
    let db = setup_test_db().await.expect("db");

    let (first, status) = upsert::<StampedCurve, _>(&db, stamped(1.5).into())
        .await
        .expect("first");
    assert_eq!(status, UpsertStatus::Created);

    let (again, status) = upsert::<StampedCurve, _>(&db, stamped(1.5).into())
        .await
        .expect("again");
    assert_eq!(
        status,
        UpsertStatus::Unchanged,
        "a timestamp is not content, so re-sending the same content changed nothing"
    );
    assert_eq!(again.created_at, first.created_at);
    assert_eq!(
        again.updated_at, first.updated_at,
        "nothing was written, so nothing advanced"
    );
}

/// MySQL stores `TIMESTAMP` to the second, so the gap has to exceed one.
#[tokio::test]
async fn a_registration_that_changes_content_advances_updated_at_only() {
    let db = setup_test_db().await.expect("db");

    let (first, _) = upsert::<StampedCurve, _>(&db, stamped(1.5).into())
        .await
        .expect("first");
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    let (edited, status) = upsert::<StampedCurve, _>(&db, stamped(2.5).into())
        .await
        .expect("edited");
    assert_eq!(status, UpsertStatus::Updated);
    assert_eq!(
        edited.created_at, first.created_at,
        "a registration must not rewrite the row's created_at"
    );
    assert!(
        edited.updated_at > first.updated_at,
        "the row changed, so the field the entity maintains on update advanced"
    );
}

#[tokio::test]
async fn test_registration_refuses_unset_key() {
    let db = setup_test_db().await.expect("db");
    let mut active = sent(1.5);
    active.source_key = sea_orm::ActiveValue::NotSet;
    let error = upsert::<RegisteredCurve, _>(&db, active)
        .await
        .expect_err("missing key");
    assert!(error.to_string().contains("part of its key"));
    assert!(
        registered_curve::Entity::find()
            .all(&db)
            .await
            .expect("rows")
            .is_empty()
    );
}

pub mod raced_curve {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "raced_curves")]
    #[crudcrate(api_struct = "RacedCurve", upsert_key(source_system, source_key))]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, exclude(create, update))]
        pub source_system: String,
        #[crudcrate(filterable, exclude(create, update))]
        pub source_key: String,
        pub slope: f64,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

use raced_curve::{RacedCurve, RacedCurveCreate};

fn raced(slope: f64) -> raced_curve::ActiveModel {
    let mut active: raced_curve::ActiveModel = RacedCurveCreate { slope }.into();
    active.source_system = Set("cnet".to_string());
    active.source_key = Set("DOC:plate-9".to_string());
    active
}

/// Scenario: two registrations of one key arrive together, and the first commits between the
/// second's read and its insert, which is the interleaving READ COMMITTED allows.
///
/// Expected behaviour: the second reports what the first stored rather than failing the unique
/// index the key is held by. SQLite serialises writers instead, so there is no such interleaving
/// to drive there.
#[tokio::test]
async fn a_key_registered_between_the_read_and_the_insert_is_merged_not_refused() {
    let db = test_suite::reset_db!(raced_curve::Entity)
        .await
        .expect("db");
    if db.get_database_backend() == DbBackend::Sqlite {
        return;
    }
    db.execute_unprepared(
        "CREATE UNIQUE INDEX raced_curves_key ON raced_curves (source_system, source_key)",
    )
    .await
    .expect("the key is held by a unique index, as a registered table's is");

    let other = test_suite::connect().await.expect("second connection");
    let holder = other.begin().await.expect("holding transaction");
    let first = raced_curve::Entity::insert(raced(1.5))
        .exec_with_returning(&holder)
        .await
        .expect("the registration that wins the race");

    let registering = tokio::spawn(async move { upsert::<RacedCurve, _>(&db, raced(2.5)).await });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    holder.commit().await.expect("the winner commits");

    let (merged, status) = registering
        .await
        .expect("the losing registration returns")
        .expect("it is not an error");
    assert_eq!(status, UpsertStatus::Updated);
    assert_eq!(merged.id, first.id, "one key is one row");
    assert!((merged.slope - 2.5).abs() < f64::EPSILON);

    let db = test_suite::connect().await.expect("reading connection");
    assert_eq!(
        raced_curve::Entity::find()
            .all(&db)
            .await
            .expect("rows")
            .len(),
        1
    );
}
