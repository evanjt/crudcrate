//! Test that `derive_eq` without `derive_partial_eq` compiles.
//!
//! `Eq: PartialEq`, so deriving `Eq` alone would emit uncompilable code.
//! The derive forces `PartialEq` on whenever `Eq` is requested, so a model
//! that sets only `derive_eq` must still compile.

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, EntityToModels)]
#[sea_orm(table_name = "labels")]
#[crudcrate(api_struct = "Label", derive_eq)]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
    pub id: Uuid,

    #[crudcrate(sortable, filterable)]
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

fn main() {
    // Eq forces PartialEq, so the generated api struct supports both.
    let a = Label {
        id: Uuid::nil(),
        name: "one".to_string(),
    };
    let b = Label {
        id: Uuid::nil(),
        name: "one".to_string(),
    };
    assert_eq!(a, b);

    fn assert_eq_bound<T: Eq>(_: &T) {}
    assert_eq_bound(&a);
}
