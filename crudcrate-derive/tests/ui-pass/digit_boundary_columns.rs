//! Field names with a digit boundary must produce the Column variant SeaORM
//! generates (`Is2faEnabled`), not a convert_case spelling (`Is2FaEnabled`).
//! `exclude(list)` on an Option field drives the select-only column list and
//! `exclude(scoped)` drives the scoped response model, the two emitters that
//! build Column idents from field names.
use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "accounts")]
#[crudcrate(api_struct = "Account")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key)]
    pub id: Uuid,
    #[crudcrate(filterable, sortable)]
    pub name: String,
    #[crudcrate(exclude(list))]
    pub is_2fa_enabled: Option<bool>,
    #[crudcrate(filterable, exclude(scoped))]
    pub has_v2_api: bool,
    pub s3_key: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

fn main() {
    let _ = Column::Is2faEnabled;
    let _ = Column::HasV2Api;
    let _ = Column::S3Key;
}
