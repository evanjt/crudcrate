use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, traits::CRUDResource};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, EntityToModels, DeriveEntityModel)]
#[sea_orm(table_name = "customers")]
#[crudcrate(api_struct = "Customer", generate_router)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,
    #[crudcrate(filterable, sortable)]
    pub name: String,
    #[crudcrate(filterable)]
    pub email: String,
    #[crudcrate(sortable, exclude(one, all, create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,
    #[crudcrate(sortable, exclude(one, all, create, update), on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,

    // depth = 2: recurse one level past the vehicles so each vehicle's own joins
    // (parts, maintenance_records) are loaded. Any depth > 1 behaves identically here;
    // the magnitude beyond 1 has no effect — a join recurses into the child's own joins
    // whenever depth > 1, and stops (loads that level flat) at depth = 1.
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all, depth = 2))]
    pub vehicles: Vec<super::vehicle::Vehicle>,
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
