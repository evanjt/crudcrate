use chrono::{DateTime, Utc};
use crudcrate::{traits::CRUDResource, EntityToModels};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

use super::vehicle::Vehicle;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "customers")]
#[crudcrate(api_struct = "Customer", generate_router, debug_output)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(filterable, sortable)]
    pub name: String,
    #[crudcrate(filterable)]
    pub email: String,
    #[crudcrate(sortable, exclude(one), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
    #[crudcrate(sortable, exclude(one), on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr = true, exclude(create, update), join(all, depth = 2))]
    pub vehicles: Vec<Vehicle>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::vehicle::Entity")]
    Vehicles,
}

impl Related<super::vehicle::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vehicles.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
