//! Minimal `CRUDResource` implementations for unit tests that need a concrete
//! resource type: fulltext dispatch and enum column handling are generic over
//! `T: CRUDResource`, so expression-level tests cannot exercise them without one.

// The resources are used as type parameters only, never constructed.
#![allow(dead_code)]

use crate::traits::{CRUDResource, MergeIntoActiveModel};

pub mod entity {
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "ft_things")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub name: String,
        pub status: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub struct Create;

impl From<Create> for entity::ActiveModel {
    fn from(_: Create) -> Self {
        entity::ActiveModel {
            ..Default::default()
        }
    }
}

pub struct Update;

impl MergeIntoActiveModel<entity::ActiveModel> for Update {
    fn merge_into_activemodel(
        self,
        existing: entity::ActiveModel,
    ) -> Result<entity::ActiveModel, crate::ApiError> {
        Ok(existing)
    }
}

macro_rules! test_resource {
    ($name:ident, $list:ident, { $($overrides:tt)* }) => {
        pub struct $name;

        pub struct $list;

        impl From<$name> for $list {
            fn from(_: $name) -> Self {
                $list
            }
        }

        impl From<entity::Model> for $name {
            fn from(_: entity::Model) -> Self {
                $name
            }
        }

        impl CRUDResource for $name {
            type EntityType = entity::Entity;
            type ColumnType = entity::Column;
            type ActiveModelType = entity::ActiveModel;
            type CreateModel = Create;
            type UpdateModel = Update;
            type ListModel = $list;

            const ID_COLUMN: Self::ColumnType = entity::Column::Id;
            const RESOURCE_NAME_SINGULAR: &str = "ft_thing";
            const RESOURCE_NAME_PLURAL: &str = "ft_things";
            const TABLE_NAME: &'static str = "ft_things";

            $($overrides)*
        }
    };
}

test_resource!(FulltextResource, FulltextList, {
    fn fulltext_searchable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![("name", entity::Column::Name)]
    }
});

test_resource!(EnumSearchResource, EnumSearchList, {
    fn is_enum_field(field_name: &str) -> bool {
        field_name == "status"
    }
});
