# crudcrate

**`crudcrate`** provides a set of procedural macros that significantly reduce the boilerplate for creating CRUD APIs in Rust. It seamlessly integrates with **[SeaORM](https://crates.io/crates/sea-orm)** for database interactions and **[Axum](https://crates.io/crates/axum)** for building web services. The macros are defined in [`crudcrate-derive`](https://crates.io/crates/crudcrate-derive) and are re-exported here for ease of use.

The library is designed with flexibility in mind—developers can quickly scaffold endpoints, automatically generate OpenAPI documentation via **[Utoipa](https://crates.io/crates/utoipa)**, and even opt out of using the provided handlers if they require custom security or additional functionality (e.g., integrating with Keycloak).

---

## 📚 Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
  - [API Example](#api-example)
  - [ToCreateModel and ToUpdateModel](#tocreatemodel-and-toupdatemodel)
  - [CRUDResource Trait and OpenAPI Integration](#crudresource-trait-and-openapi-integration)
  - [CRUD Handlers](#crud-handlers)
- [Customization and Flexibility](#customization-and-flexibility)
- [License](#license)

---

## ✨ Features

- **Automatic Model Generation:** Use the `ToCreateModel` and `ToUpdateModel` macros to automatically generate Create and Update models from your database entity, and supporting functions to merge updates into sea-orm's ActiveModels.
- **CRUD Endpoints:** Generate fully functional CRUD endpoints with the `crud_handlers!` macro that autogenerate OpenAPI compatible docs, integrate the update and create models generated from `ToCreateModel` and `ToUpdateModel`.
- **OpenAPI Integration:** Automatically generate API documentation through Utoipa.
- **Sorting, Filtering & Pagination:** Built-in modules for handling sort, filter, and pagination that are compatible with tools like **[React-admin](https://marmelab.com/react-admin/)**.
- **Flexibility:** Opt in or out of generated handlers and models as needed, allowing custom security layers (e.g., Keycloak) and tailored API behavior.
- **Time Savings:** Drastically reduce repetitive code, enabling you to focus on your business logic while saving development time.

---

## 🚀 Installation

Add `crudcrate` to your `Cargo.toml`:

```toml
[dependencies]
crudcrate = "0.1"  # Replace with the latest version
```

---

## 📦 Usage

### API Example

A complete working example using a simple **Todo** API is available in our [crudcrate-example](https://github.com/evanjt/crudcrate-example) repository. This example demonstrates how to build a public-facing API with a single `todo` table, including all CRUD operations.


### ✅ ToCreateModel and ToUpdateModel

Normally, you might write verbose code to create models for database inserts and updates. With `crudcrate`, you can dramatically simplify this:

**Verbose (Manual) Approach:**

```rust
use super::db::{ActiveModel, Model};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use sea_orm::{FromQueryResult, NotSet, Set};

#[derive(ToSchema, Serialize, Deserialize)]
pub struct Todo {
    id: Uuid,
    title: String,
    completed: bool,
    last_updated: NaiveDateTime,
}

#[derive(ToSchema, Serialize, Deserialize, FromQueryResult)]
pub struct TodoCreate {
    pub title: String,
    pub completed: Option<bool>,
}

#[derive(ToSchema, Serialize, Deserialize, FromQueryResult)]
pub struct TodoUpdate {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub title: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "::serde_with::rust::double_option"
    )]
    pub completed: Option<Option<bool>>,
}

impl TodoUpdate {
    pub fn merge_into_activemodel(self, mut model: ActiveModel) -> ActiveModel {
        model.title = match self.title {
            Some(Some(title)) => Set(title),
            None => NotSet,
            _ => NotSet,
        };
        model.completed = match self.completed {
            Some(Some(completed)) => Set(completed),
            None => NotSet,
            _ => NotSet,
        };
        model
    }
}
```

**Simplified with `crudcrate`:**

```rust
use crudcrate::{ToCreateModel, ToUpdateModel};
use sea_orm::FromQueryResult;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(ToSchema, Serialize, Deserialize, FromQueryResult, ToUpdateModel, ToCreateModel)]
#[active_model = "super::db::ActiveModel"]
pub struct Todo {
    #[crudcrate(update = false, create = false)]
    id: Uuid,
    title: String,
    #[crudcrate(on_create = false)]
    completed: bool,
    #[crudcrate(update = false, create = false)]
    last_updated: NaiveDateTime,
}
```

This approach generates the `TodoCreate` and `TodoUpdate` structs automatically, handling default values and optional fields based on your annotations.

---

### 📖 CRUDResource Trait and OpenAPI Integration

The core of `crudcrate` is the `CRUDResource` trait, which standardizes common CRUD operations and automatically integrates with OpenAPI documentation through Utoipa. Below is an example implementation for a **Todo** resource.

```rust
use async_trait::async_trait;
use sea_orm::{
    entity::prelude::*, Condition, DatabaseConnection, EntityTrait, Order, PaginatorTrait,
};
use uuid::Uuid;

// Assume Todo is defined as shown in the ToCreateModel/ToUpdateModel example,
// and your db.rs file defines the Todo table (with Entity, Column, etc).

#[async_trait]
impl CRUDResource for Todo {
    type EntityType = super::db::Entity;
    type ColumnType = super::db::Column;
    type ActiveModelType = super::db::ActiveModel;
    type CreateModel = TodoCreate;
    type UpdateModel = TodoUpdate;

    const ID_COLUMN: Self::ColumnType = super::db::Column::Id;
    const RESOURCE_NAME_SINGULAR: &'static str = "todo";
    const RESOURCE_NAME_PLURAL: &'static str = "todos";
    const RESOURCE_DESCRIPTION: &'static str = "A simple todo item that includes a title and completion status.";

    fn sortable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![
            ("id", Self::ID_COLUMN),
            ("title", Self::ColumnType::Title),
            ("completed", Self::ColumnType::Completed),
            ("last_updated", Self::ColumnType::LastUpdated),
        ]
    }

    fn filterable_columns() -> Vec<(&'static str, Self::ColumnType)> {
        vec![
            ("title", Self::ColumnType::Title),
            ("completed", Self::ColumnType::Completed),
        ]
    }
}
```


**Key Points:**

- **Standardization:** By implementing `CRUDResource`, your resource automatically supports standardized methods for listing, fetching, creating, updating, and deleting records.
- **OpenAPI Documentation:** The trait’s constants (like `RESOURCE_NAME_SINGULAR`, `RESOURCE_NAME_PLURAL`, and `RESOURCE_DESCRIPTION`) are used by the generated OpenAPI documentation. This ensures that your API docs are consistent and descriptive.
- **Integration with React-Admin:** The trait also defines methods for sorting, filtering, and pagination. When used in conjunction with helper modules (e.g., `sort.rs`, `filter.rs`, `pagination.rs`), it aligns with the default behaviors of popular admin interfaces such as **[React-admin](https://marmelab.com/react-admin/)**.
- **Asynchronous and Flexible:** The use of `async_trait` ensures that all CRUD operations are asynchronous, and the trait constraints ensure smooth integration with SeaORM’s ActiveModels.

---

### ✅ CRUD Handlers

If you prefer a complete, ready-to-use API, the `crud_handlers!` macro generates all the necessary endpoint handlers for Axum, including support for:
- Fetching a single record
- Listing records with sorting, filtering, and pagination
- Creating a new record
- Updating an existing record
- Deleting one or many records

Example:

```rust
use super::models::{Todo, TodoCreate, TodoUpdate};
use crudcrate::{CRUDResource, crud_handlers};
use sea_orm::DatabaseConnection;
use utoipa_axum::{router::OpenApiRouter, routes};

// Generate handlers for Todo:
// - get_one_handler at GET /todo/{id}
// - get_all_handler at GET /todo
// - create_one_handler at POST /todo
// - update_one_handler at PUT /todo/{id}
// - delete_one_handler at DELETE /todo/{id}
// - delete_many_handler at DELETE /todo/batch

crud_handlers!(Todo, TodoUpdate, TodoCreate);

pub fn router(db: &DatabaseConnection) -> OpenApiRouter
where
    Todo: CRUDResource,
{
    OpenApiRouter::new()
        .routes(routes!(get_one_handler))
        .routes(routes!(get_all_handler))
        .routes(routes!(create_one_handler))
        .routes(routes!(update_one_handler))
        .routes(routes!(delete_one_handler))
        .routes(routes!(delete_many_handler))
        .with_state(db.clone())
}
```

---

## 🔧 Customization and Flexibility

- **Opt Out When Needed:** You are not forced to use the generated handlers. You can implement your own endpoint logic or integrate additional security layers (e.g., Keycloak) as needed.
- **Selective Model Generation:** Use attributes such as `#[crudcrate(create = false)]` or `#[crudcrate(update = false)]` on individual fields to control their inclusion in the generated Create and Update models.
- **Seamless OpenAPI Docs:** The CRUDResource trait’s constants and method implementations are designed to automatically generate and enrich OpenAPI documentation. This minimizes manual doc updates and ensures that your API docs always reflect the current state of your resource.
- **Improved Developer Velocity:** By handling common tasks (sorting, filtering, pagination, error handling) out of the box, `crudcrate` helps you ship features faster while reducing the risk of repetitive bugs and inconsistencies.

---

## 📜 License

This project is licensed under the MIT License. See [LICENSE](./LICENSE) for more details.
