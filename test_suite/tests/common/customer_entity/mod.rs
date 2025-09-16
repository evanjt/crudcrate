use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, traits::CRUDResource};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

use super::vehicle_entity::Vehicle;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "customers")]
#[crudcrate(api_struct = "Customer", generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, create_model = false, update_model = false, on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    #[crudcrate(filterable)]
    pub email: String,

    #[crudcrate(sortable, create_model = false, update_model = false, on_create = Utc::now())]
    pub created_at: DateTime<Utc>,

    #[crudcrate(sortable, create_model = false, update_model = false, on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,

    // Join field for recursive loading
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, join(one, all, depth = 2))]
    pub vehicles: Vec<Vehicle>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::vehicle_entity::Entity")]
    Vehicles,
}

impl Related<super::vehicle_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vehicles.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Type alias for easier importing
pub type CustomerEntity = Entity;