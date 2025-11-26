//! Test that exclude(create, update, one, list) attributes compile correctly
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use chrono::{DateTime, Utc};

pub mod item {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, EntityToModels, Serialize, Deserialize, ToSchema)]
    #[sea_orm(table_name = "items")]
    #[crudcrate(api_struct = "Item", derive_partial_eq, derive_eq)]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        #[crudcrate(filterable, sortable)]
        pub name: String,

        // Excluded from list responses
        #[crudcrate(exclude(list))]
        pub detailed_description: String,

        // Excluded from single-item responses
        #[crudcrate(exclude(one))]
        pub list_only_field: String,

        // Auto-managed timestamp
        #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
        pub created_at: DateTime<Utc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

fn main() {
    use item::*;

    // Verify Create model doesn't have id or created_at (they're auto-generated)
    let _create = ItemCreate {
        name: "test".to_string(),
        detailed_description: "desc".to_string(),
        list_only_field: "list".to_string(),
    };

    // Verify the models exist and compile
    let _: fn() -> Item = || panic!();
    let _: fn() -> ItemCreate = || panic!();
    let _: fn() -> ItemUpdate = || panic!();
    let _: fn() -> ItemList = || panic!();
    let _: fn() -> ItemResponse = || panic!();
}
