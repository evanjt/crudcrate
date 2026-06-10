//! Regression guard: a `generate_router` entity must compile when the consumer
//! enables utoipa's `axum_extras` feature (as real axum services do).
//!
//! Before the fix, crudcrate's generated batch handlers exposed a `:ty` macro
//! fragment (`PrimaryKeyType<$resource>`) nested in `Path<>`/`Json<>` signatures
//! and relied on `axum_extras` param/body inference, which (a) the utoipa parser
//! could not descend into and (b) required the primary-key type to implement
//! `ToSchema`. With `axum_extras` enabled here, this file only compiles if both
//! are handled. See `crudcrate/src/core/crud_operations.rs`.

use crudcrate::{CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;

#[derive(
    Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize, EntityToModels,
)]
#[sea_orm(table_name = "axum_extras_things")]
#[crudcrate(api_struct = "AxumExtrasThing", generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(filterable, sortable)]
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

#[tokio::test]
async fn generate_router_compiles_with_axum_extras() {
    // Building the router exercises the generated #[utoipa::path] handlers
    // (get_one / delete_one / update_one / delete_many) under axum_extras.
    let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
    let _router = AxumExtrasThing::router(&db);
    assert_eq!(AxumExtrasThing::TABLE_NAME, "axum_extras_things");
}
