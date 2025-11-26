//! # `CrudCrate` - Transform Sea-ORM entities into complete REST APIs with zero boilerplate
//!
//! **`CrudCrate`** is a Rust ecosystem that eliminates the repetitive work of building CRUD APIs by automatically generating complete REST endpoints from Sea-ORM entities through powerful procedural macros.
//!
//! ## Quick Start
//!
//! Add the main derive macro to your Sea-ORM entity:
//!
//! ```rust,ignore,ignore
//! use chrono::{DateTime, Utc};
//! use crudcrate::EntityToModels;
//! use sea_orm::entity::prelude::*;
//! use uuid::Uuid;
//!
//! #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
//! #[sea_orm(table_name = "todos")]
//! #[crudcrate(api_struct = "Todo", generate_router)]
//! pub struct Model {
//!     #[sea_orm(primary_key, auto_increment = false)]
//!     #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
//!     pub id: Uuid,
//!
//!     #[crudcrate(filterable, sortable, fulltext)]
//!     pub title: String,
//!
//!     #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
//!     pub created_at: DateTime<Utc>,
//! }
//! ```
//!
//! This single `#[derive(EntityToModels)]` generates everything automatically:
//! - `Todo` struct (for API responses)
//! - `TodoCreate` struct (for POST requests, excludes `id`)
//! - `TodoUpdate` struct (for PUT requests, excludes `id`)
//! - Complete `CRUDResource` implementation
//! - Router function with all CRUD endpoints
//!
//! Then use it in your application:
//!
//! ```rust,ignore,ignore
//! use axum::Router;
//! use sea_orm::DatabaseConnection;
//!
//! let app = Router::new()
//!     .merge(Todo::router(&db));
//!
//! // Available endpoints:
//! // GET    /todos        - List all todos with filtering/sorting
//! // GET    /todos/{id}   - Get specific todo
//! // POST   /todos        - Create new todo
//! // PUT    /todos/{id}   - Update todo
//! // DELETE /todos/{id}   - Delete todo
//! ```
//!
//! ## Core Features
//!
//! ### 🚀 Auto-Generated CRUD Operations
//! Transform Sea-ORM entities into complete REST APIs with zero boilerplate. Write one `#[derive(EntityToModels)]` and get 6 HTTP endpoints automatically.
//!
//! ### 🏗️ Smart Model Generation
//! Automatically creates Create/Update/List structs from your database model. No more writing 90% identical structs - one entity becomes 4 specialized models.
//!
//! ### 🔍 Advanced Filtering & Search
//! Query parameter → SQL condition translation with fulltext search. Rich filtering APIs without writing SQL - supports comparisons, lists, text search.
//!
//! ### 🔗 Relationship Loading
//! Populate related data in API responses automatically. Include nested data (Customer → Vehicles) without N+1 queries or manual joins.
//!
//! ### ⚡ Multi-Database Optimization
//! Database-specific query optimizations and index recommendations. Production-ready performance across SQLite/PostgreSQL/MySQL without config.
//!
//! ### 🛠️ Development Experience
//! Rich attribute system, `OpenAPI` docs, debug output, IDE support. Fast development cycle with great tooling and clear generated APIs.
//!
//! ## Module Organization
//!
//! The library is organized into feature groups for better maintainability:
//!
//! - **[`core`](@/core/index.html)**: Core CRUD operations and traits
//! - **[`filtering`](@/filtering/index.html)**: Query parameter parsing and filtering
//! - **[`relationships`](@/relationships/index.html)**: Join loading and relationship handling
//! - **[`database`](@/database/index.html)**: Database optimization and index analysis
//! - **[`dev_experience`](@/dev_experience/index.html)**: Debug features and developer tools
//!
//! ## Examples
//!
//! ### Basic CRUD with Filtering
//!
//! ```rust,ignore
//! # use crudcrate::EntityToModels;
//! # use sea_orm::entity::prelude::*;
//! #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
//! #[sea_orm(table_name = "customers")]
//! #[crudcrate(api_struct = "Customer", generate_router)]
//! pub struct Model {
//!     #[sea_orm(primary_key, auto_increment = false)]
//!     #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
//!     pub id: Uuid,
//!
//!     #[crudcrate(filterable, sortable)]
//!     pub name: String,
//!
//!     #[crudcrate(filterable)]
//!     pub email: String,
//! }
//! ```
//!
//! ### Relationship Loading
//!
//! ```rust,ignore
//! # use crudcrate::EntityToModels;
//! # use sea_orm::entity::prelude::*;
//! #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
//! #[sea_orm(table_name = "customers")]
//! #[crudcrate(api_struct = "Customer", generate_router)]
//! pub struct Model {
//!     #[sea_orm(primary_key, auto_increment = false)]
//!     #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
//!     pub id: Uuid,
//!
//!     #[crudcrate(filterable, sortable)]
//!     pub name: String,
//!
//!     #[sea_orm(ignore)]
//!     #[crudcrate(non_db_attr = true, exclude(create, update), join(one, all))]
//!     pub vehicles: Vec<Vehicle>,
//! }
//! ```
//!
//! ## Advanced Attributes
//!
//! `CrudCrate` provides comprehensive attribute customization:
//!
//! ### Primary Keys
//! ```rust,ignore
//! #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
//! pub id: Uuid,
//! ```
//!
//! ### Searchable Fields
//! ```rust,ignore
//! #[crudcrate(sortable, filterable, fulltext)]
//! pub title: String,
//! ```
//!
//! ### Auto-Managed Timestamps
//! ```rust,ignore
//! // Created timestamp
//! #[crudcrate(sortable, exclude(create, update), on_create = Utc::now())]
//! pub created_at: DateTime<Utc>,
//!
//! // Updated timestamp
//! #[crudcrate(sortable, exclude(create, update), on_create = Utc::now(), on_update = Utc::now())]
//! pub updated_at: DateTime<Utc>,
//! ```
//!
//! ### Relationship Loading
//! ```rust,ignore
//! #[sea_orm(ignore)]
//! #[crudcrate(non_db_attr = true, join(one, all))]
//! pub vehicles: Vec<Vehicle>,
//! ```
//!
//! ## Filtering & Querying
//!
//! Once your API is running, you get powerful filtering capabilities automatically:
//!
//! ### Basic Filtering
//! ```bash
//! # Simple equality
//! GET /todos?completed=true
//!
//! # String contains
//! GET /todos?title_like=example
//!
//! # Numeric comparisons
//! GET /todos?priority_gte=5
//! ```
//!
//! ### Advanced Queries
//! ```bash
//! # Multiple filters
//! GET /todos?completed=false&priority_gte=3&title_like=urgent
//!
//! # Sorting
//! GET /todos?sort=created_at_desc,priority_asc
//!
//! # Fulltext search
//! GET /todos?q=search terms
//! ```
//!
//! ## Feature Flags
//!
//! - **`derive`**: Enables procedural macros (default)
//! - **`debug`**: Shows generated code during compilation
//! - **`sqlite`**: `SQLite` database support (default)
//! - **`postgresql`**: `PostgreSQL` database support
//! - **`mysql`**: `MySQL` database support
//! - **`spring-rs`**: Spring-rs framework integration
//!
//! ## Database Support
//!
//! `CrudCrate` works with multiple databases, providing optimizations for each:
//!
//! - **`SQLite`**: Default, fastest for development and testing
//! - **`PostgreSQL`**: Production-ready with advanced fulltext search via tsvector/GIN indexes
//! - **`MySQL`**: FULLTEXT index support for search optimization
//!
//! ## License
//!
//! Licensed under MIT License. See [LICENSE](https://github.com/evanjt/crudcrate/blob/main/LICENSE) for details.

// Core Feature Groups
pub mod core;
pub mod database;
pub mod errors;
pub mod filtering;
pub mod operations;
pub mod relationships;
pub mod validation;

// Legacy modules for backward compatibility (re-export from new structure)
pub mod filter {
    pub use crate::filtering::conditions::*;
}
pub mod models {
    pub use crate::filtering::query_parser::*;
}
pub mod pagination {
    pub use crate::filtering::pagination::*;
}
pub mod routes {
    // Legacy module for backward compatibility - CRUD handlers are now generated automatically
}
pub mod sort {
    pub use crate::filtering::sort::*;
}
pub mod traits {
    pub use crate::core::traits::*;
}

// Export procedural macros
pub use crudcrate_derive::*;

// Export commonly used items from feature groups
pub use core::{CRUDResource, MergeIntoActiveModel};
pub use errors::ApiError;
pub use filtering::{
    FilterOptions, apply_filters, calculate_content_range, parse_pagination, parse_range,
    parse_sorting,
};
pub use operations::{CRUDOperations, DefaultCRUDOperations};

pub use serde_with;
