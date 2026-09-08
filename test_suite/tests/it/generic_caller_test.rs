//! Scenario: plumbing generic over the resource spawns or boxes an operation's future.
//!
//! Expected behaviour: it compiles and runs. The trait futures are `Send`, so a caller does not
//! have to name the resource to hand one to `tokio::spawn`.

use crudcrate::traits::CRUDResource;
use crudcrate::{ApiError, CRUDOperations, EntityToModels};
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, DbErr};
use tokio::task::JoinHandle;
use uuid::Uuid;

pub mod note {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "spawned_notes")]
    #[crudcrate(api_struct = "Note", operations = NoteOps)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        pub body: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}

    pub struct NoteOps;

    impl CRUDOperations for NoteOps {
        type Resource = Note;
    }
}

use note::{Note, NoteCreate, NoteOps};

fn spawn_create<R: CRUDResource + 'static>(
    db: DatabaseConnection,
    data: R::CreateModel,
) -> JoinHandle<Result<R, ApiError>>
where
    R::CreateModel: 'static,
{
    tokio::spawn(async move { R::create(&db, data).await })
}

fn spawn_delete<O: CRUDOperations + 'static>(
    ops: O,
    db: DatabaseConnection,
    id: crudcrate::PrimaryKeyType<O::Resource>,
) -> JoinHandle<Result<crudcrate::PrimaryKeyType<O::Resource>, ApiError>> {
    tokio::spawn(async move { ops.delete(&db, id).await })
}

fn boxed_get_one<'a, R: CRUDResource + 'a>(
    db: &'a DatabaseConnection,
    id: crudcrate::PrimaryKeyType<R>,
) -> std::pin::Pin<Box<dyn Future<Output = Result<R, ApiError>> + Send + 'a>> {
    Box::pin(R::get_one(db, id))
}

async fn setup_test_db() -> Result<DatabaseConnection, DbErr> {
    test_suite::reset_db!(note::Entity).await
}

#[tokio::test]
async fn an_operation_future_can_be_spawned_from_generic_code() {
    let db = setup_test_db().await.unwrap();

    let created = spawn_create::<Note>(
        db.clone(),
        NoteCreate {
            body: "spawned".to_string(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let fetched = boxed_get_one::<Note>(&db, created.id).await.unwrap();
    assert_eq!(fetched.body, "spawned");

    let deleted = spawn_delete(NoteOps, db.clone(), created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(deleted, created.id);
    assert_eq!(note::Entity::find().count(&db).await.unwrap(), 0);
}
