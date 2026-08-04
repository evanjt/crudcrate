//! A relation wrapper field (`HasMany<..>`) on a struct that is NOT processed
//! by `#[sea_orm::model]` must produce a guided compile error, not a silent
//! no-op: without the attribute macro nothing strips the wrapper fields, so
//! the derive would either mis-treat them as columns or (worse) skip the
//! struct entirely, surfacing as "cannot find type" errors far away.

use crudcrate::EntityToModels;
use sea_orm::entity::prelude::*;
use uuid::Uuid;

pub mod book {
    use super::*;

    #[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "drwm_books")]
    #[crudcrate(api_struct = "Book")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        pub title: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub mod author {
    use super::*;

    // Missing #[sea_orm::model] above the derives.
    #[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "drwm_authors")]
    #[crudcrate(api_struct = "Author")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,

        pub name: String,

        #[sea_orm(has_many)]
        pub books: HasMany<super::book::Entity>,
    }

    impl ActiveModelBehavior for ActiveModel {}
}

fn main() {}
