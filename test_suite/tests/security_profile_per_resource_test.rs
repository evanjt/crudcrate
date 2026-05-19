//! Per-resource `SecurityProfile` override via the
//! `#[crudcrate(security_profile = "...")]` attribute.
//!
//! Verifies that the derive macro reads the attribute value and emits a
//! `CRUDResource::security_profile()` override that returns the named preset,
//! so consumers can opt into a tighter profile without wiring an
//! `Extension<SecurityProfile>` layer.

use crudcrate::{CRUDResource, EntityToModels, SecurityProfile};

mod secure_attr {
    use super::*;
    use sea_orm::entity::prelude::*;
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "secure_resources")]
    #[crudcrate(api_struct = "SecureResource", security_profile = "secure")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,
        #[crudcrate(filterable)]
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

mod react_admin_attr {
    use super::*;
    use sea_orm::entity::prelude::*;
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "ra_resources")]
    #[crudcrate(api_struct = "RaResource", security_profile = "react_admin")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,
        #[crudcrate(filterable)]
        pub label: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

mod no_attr {
    use super::*;
    use sea_orm::entity::prelude::*;
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
    #[sea_orm(table_name = "default_resources")]
    #[crudcrate(api_struct = "DefaultResource")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
        pub id: Uuid,
        #[crudcrate(filterable)]
        pub name: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
}

#[test]
fn secure_attribute_returns_secure_profile() {
    assert_eq!(
        secure_attr::SecureResource::security_profile(),
        SecurityProfile::secure()
    );
}

#[test]
fn react_admin_attribute_returns_react_admin_profile() {
    assert_eq!(
        react_admin_attr::RaResource::security_profile(),
        SecurityProfile::react_admin()
    );
}

#[test]
fn no_attribute_returns_secure_profile() {
    // 0.9.0: the trait default flipped from `legacy()` to `secure()`. Resources
    // that don't override the attribute now ship hardened defaults.
    assert_eq!(
        no_attr::DefaultResource::security_profile(),
        SecurityProfile::secure()
    );
}
