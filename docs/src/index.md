# CRUDCrate

**Transform Sea-ORM entities into Axum-based REST APIs with a single derive
macro.**

CRUDCrate eliminates unnecessary boilerplate in creating REST APIs. Its focus
is to autogenerate the repetitive code needed for basic CRUD operations but
leave open the opportunity to customise and extend wherever needed.

With a Sea-ORM entity definition, adding `#[derive(EntityToModels)]` to your
model and then `#[crudcrate(generate_router)]` is all that is needed for the
default behaviour that includes:

* CRUD endpoints
* Utoipa OpenAPI documentation
* Request/response models
* Error handling
* Filtering
* Sorting
* Pagination
* Relationship loading
<!-- 
## Quick Example

```rust
use axum::{Router, Extension};
use sea_orm::DatabaseConnection;
use crudcrate::EntityToModels;

// Your Sea-ORM entity
#[derive(Clone, Debug, DeriveEntityModel, EntityToModels)]
#[crudcrate(generate_router)]
#[sea_orm(table_name = "todos")]
pub struct Model {
    #[sea_orm(primary_key)]
    #[crudcrate(primary_key, exclude(create, update))]
    pub id: i32,

    #[crudcrate(filterable, sortable)]
    pub email: String,

    #[crudcrate(exclude(one, list))]  
    pub password_hash: String,

    #[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
    pub created_at: DateTime,
}

// In your main.rs
async fn main() {
    let db: DatabaseConnection = /* ... */;

    let app = Router::new()
        .merge(user::user_router())
        .layer(Extension(db));

    axum::serve(listener, app).await.unwrap();
}
```


## Installation

Add this to your `Cargo.toml`:

```bash
cargo add crudcrate
``` 
