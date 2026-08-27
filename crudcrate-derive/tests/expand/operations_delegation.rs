//! operations = MyOps routes the generated CRUDResource through CRUDOperations
use async_trait::async_trait;
use crudcrate::{ApiError, CRUDOperations, EntityToModels};
use sea_orm::{DatabaseConnection, entity::prelude::*};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "products")]
#[crudcrate(api_struct = "Product", generate_router, operations = ProductOperations)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(filterable, sortable)]
    pub name: String,
    pub price: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

pub struct ProductOperations;

#[async_trait]
impl CRUDOperations for ProductOperations {
    type Resource = Product;
    async fn before_create(&self, _db: &DatabaseConnection, data: &ProductCreate) -> Result<(), ApiError> {
        if data.price <= 0 {
            return Err(ApiError::bad_request("Price must be positive"));
        }
        Ok(())
    }
}

fn main() {}
