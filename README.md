# crudcrate

This crate aims to reduce the amount of excess code required to generate structures
for CRUD APIs working with sea-orm and axum. 

For example, the structs below, and their mutating functions with an ActiveModel:

```rust
use super::db::{ActiveModel, Model};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use sea_orm::{FromQueryResult, NotSet, Set};

#[derive(ToSchema, Serialize, Deserialize)]
pub struct Project {
    color: String,
    last_updated: NaiveDateTime,
    description: Option<String>,
    id: Uuid,
    name: String,
}

#[derive(ToSchema, Serialize, Deserialize, FromQueryResult)]
pub struct ProjectCreate {
    pub color: Option<String>,
    pub description: Option<String>,
    pub name: String,
}

#[derive(ToSchema, Serialize, Deserialize, FromQueryResult)]
pub struct ProjectUpdate {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub color: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub description: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub name: Option<Option<String>>,
}

impl ProjectUpdate {
    pub fn merge_into_activemodel(self, mut model: ActiveModel) -> ActiveModel {
        model.color = match self.color {
            Some(Some(color)) => Set(color),
            None => NotSet,
            _ => NotSet,
        };
        model.description = match self.description {
            Some(description) => Set(description),
            None => NotSet,
        };
        model.name = match self.name {
            Some(Some(name)) => Set(name),
            None => NotSet,
            _ => NotSet,
        };
        model
    }
}
```

Can be reduced to:

```rust
use crudcrate::{ToCreateModel, ToUpdateModel};
use sea_orm::FromQueryResult;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(ToSchema, Serialize, Deserialize, FromQueryResult, ToUpdateModel, ToCreateModel)]
#[active_model = "super::db::ActiveModel"]
pub struct Project {
    color: String,
    #[crudcrate(update = false, create = false)]
    last_updated: NaiveDateTime,
    description: Option<String>,
    #[crudcrate(update = false, create = false)]
    id: Uuid,
    name: String,
}
```

Using the `ToUpdateModel` and `ToCreateModel` create the models 
`ProjectUpdate` and `ProjectCreate` respectively. Using the `update` and
`create` attributes allow the flexibility of their inclusion in the 
generated structs.