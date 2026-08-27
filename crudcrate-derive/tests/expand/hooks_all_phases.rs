//! Every hook phase on every operation: pre, body (delete), transform, post
use chrono::{DateTime, Utc};
use crudcrate::{ApiError, EntityToModels};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

async fn pre_create(_db: &DatabaseConnection, _d: &ItemCreate) -> Result<(), ApiError> { Ok(()) }
async fn post_create(_db: &DatabaseConnection, _e: &Item) -> Result<(), ApiError> { Ok(()) }
async fn transform_create(_db: &DatabaseConnection, e: Item) -> Result<Item, ApiError> { Ok(e) }
async fn pre_read(_db: &DatabaseConnection, _id: Uuid) -> Result<(), ApiError> { Ok(()) }
async fn post_read(_db: &DatabaseConnection, _e: &Item) -> Result<(), ApiError> { Ok(()) }
async fn transform_read(_db: &DatabaseConnection, e: Item) -> Result<Item, ApiError> { Ok(e) }
async fn pre_update(_db: &DatabaseConnection, _id: Uuid, _d: &ItemUpdate) -> Result<(), ApiError> { Ok(()) }
async fn post_update(_db: &DatabaseConnection, _e: &Item) -> Result<(), ApiError> { Ok(()) }
async fn transform_update(_db: &DatabaseConnection, e: Item) -> Result<Item, ApiError> { Ok(e) }
async fn pre_delete(_db: &DatabaseConnection, _id: Uuid) -> Result<(), ApiError> { Ok(()) }
async fn body_delete(_db: &DatabaseConnection, id: Uuid) -> Result<Uuid, ApiError> { Ok(id) }
async fn post_delete(_db: &DatabaseConnection, _id: Uuid) -> Result<(), ApiError> { Ok(()) }
async fn transform_delete(_db: &DatabaseConnection, id: Uuid) -> Result<Uuid, ApiError> { Ok(id) }

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "items")]
#[crudcrate(
    api_struct = "Item",
    name_singular = "item",
    name_plural = "items",
    generate_router,
    create::one::pre = pre_create,
    create::one::post = post_create,
    create::one::transform = transform_create,
    read::one::pre = pre_read,
    read::one::post = post_read,
    read::one::transform = transform_read,
    update::one::pre = pre_update,
    update::one::post = post_update,
    update::one::transform = transform_update,
    delete::one::pre = pre_delete,
    delete::one::body = body_delete,
    delete::one::post = post_delete,
    delete::one::transform = transform_delete,
)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(filterable, sortable)]
    pub name: String,
    #[crudcrate(exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

fn main() {}
