use chrono::{DateTime, Utc};
use crudcrate::{EntityToModels, traits::CRUDResource};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "companies")]
#[crudcrate(api_struct = "Company", generate_router, derive_partial_eq)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(filterable, sortable)]
    pub name: String,

    #[crudcrate(filterable)]
    pub industry: String,

    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
    pub created_at: DateTime<Utc>,

    #[crudcrate(sortable, exclude(create, update), on_create = Utc::now(), on_update = Utc::now())]
    pub updated_at: DateTime<Utc>,

    // Branches of this company. depth = 2 means each branch is loaded along with
    // its own joins (departments, and their joins in turn).
    #[sea_orm(ignore)]
    #[crudcrate(non_db_attr, join(one, all, depth = 2))]
    pub branches: Vec<super::branch::Branch>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::branch::Entity")]
    Branches,
}

impl Related<super::branch::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Branches.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
