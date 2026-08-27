//! Derive complete REST APIs from Sea-ORM entities.
//!
//! `crudcrate` generates CRUD endpoints, request/response models, filtering, sorting,
//! pagination, batch operations, relationship loading, and `OpenAPI` schemas from a single
//! `#[derive(EntityToModels)]` on your Sea-ORM model. It targets [Axum](https://github.com/tokio-rs/axum)
//! and uses [utoipa](https://docs.rs/utoipa) for schema generation.
//!
//! For tutorials, walkthroughs, and guides see **<https://crudcrate.evanjt.com>**.
//!
//! # Quick start
//!
//! ```rust,ignore
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
//! This generates:
//!
//! - `Todo`, `TodoCreate`, `TodoUpdate`, `TodoList` structs
//! - A [`CRUDResource`] implementation with default get/create/update/delete logic
//! - `Todo::router(&db)` returning an Axum [`Router`](axum::Router) with all endpoints
//!
//! Mount it:
//!
//! ```rust,ignore
//! let app = Router::new().nest("/todos", Todo::router(&db));
//! ```
//!
//! # Generated endpoints
//!
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | `/{resource}` | List with filtering, sorting, fulltext search, pagination |
//! | GET | `/{resource}/{id}` | Single item with optional relationship loading |
//! | POST | `/{resource}` | Create |
//! | PUT | `/{resource}/{id}` | Partial update |
//! | DELETE | `/{resource}/{id}` | Delete |
//! | POST | `/{resource}/batch` | Batch create |
//! | PATCH | `/{resource}/batch` | Batch update |
//! | DELETE | `/{resource}/batch` | Batch delete |
//!
//! # Filtering and search
//!
//! Fields marked `filterable` accept query parameters with operator suffixes:
//!
//! ```text
//! GET /todos?filter={"completed":false,"priority_gte":3}
//! GET /todos?q=urgent review           # fulltext search
//! GET /todos?sort=["created_at","DESC"]
//! GET /todos?range=[0,24]              # pagination (React Admin compatible)
//! ```
//!
//! See the [`filtering`] module for the full operator reference.
//!
//! # Relationship loading
//!
//! Non-database fields annotated with `join(...)` are populated automatically:
//!
//! ```rust,ignore
//! #[sea_orm(ignore)]
//! #[crudcrate(non_db_attr, join(one, all, depth = 2))]
//! pub vehicles: Vec<Vehicle>,
//! ```
//!
//! At depth 1, list endpoints use batch loading (2 queries, not N+1).
//! Recursive loading supports up to depth 5. Self-referencing fields are
//! constrained to depth 1 at compile time.
//!
//! # Hooks
//!
//! Override any phase of any operation with attribute-based hooks:
//!
//! ```rust,ignore
//! #[crudcrate(
//!     generate_router,
//!     create::one::pre = validate_input,
//!     read::one::transform = enrich_with_metadata,
//!     delete::one::post = cleanup_s3_assets,
//! )]
//! ```
//!
//! Hook phases run in order: **pre** → **body** → **transform** → **post**.
//!
//! See the [`EntityToModels`] derive macro docs for the full attribute reference,
//! and the [`operations`] module for the [`CRUDOperations`] trait (an alternative
//! to per-attribute hooks).
//!
//! # Modules
//!
//! - [`core`]: [`CRUDResource`] trait, default CRUD implementations
//! - [`filtering`]: Query parameter parsing, filter conditions, pagination, sorting, fulltext search
//! - [`operations`]: [`CRUDOperations`] trait for struct-based customization
//! - [`errors`]: [`ApiError`] type with automatic HTTP status codes and internal logging
//! - [`validation`]: Input validation helpers
//!
//! # Feature flags
//!
//! | Flag | Default | Description |
//! |------|---------|-------------|
//! | `derive` | yes | Enables procedural macros (`EntityToModels`, etc.) |
//! | `sqlite` | yes | SQLite support via sqlx |
//! | `postgresql` | no | PostgreSQL support (enables GIN/tsvector fulltext) |
//! | `mysql` | no | MySQL support (enables FULLTEXT indexes) |

pub mod batch;
pub mod core;
#[doc(hidden)]
pub mod database;
pub mod errors;
pub mod filtering;
#[doc(hidden)]
pub mod impls_check;
pub mod operations;
pub mod profile;
#[doc(hidden)]
pub mod relationships;
pub mod scope;
pub mod validation;

// Compatibility aliases for paths older releases exposed. `filter`, `models`,
// `pagination` and `sort` are no longer named by crudcrate's own macros.
// `traits` still is (`crudcrate::traits::CRUDResource` in the handler macros
// and in derive output) and is not going away.
#[doc(hidden)]
pub mod filter {
    pub use crate::filtering::conditions::*;
}
#[doc(hidden)]
pub mod models {
    pub use crate::filtering::query_parser::*;
}
#[doc(hidden)]
pub mod pagination {
    pub use crate::filtering::pagination::*;
}
#[doc(hidden)]
pub mod sort {
    pub use crate::filtering::sort::*;
}
#[doc(hidden)]
pub mod traits {
    pub use crate::core::traits::*;
}

pub use crudcrate_derive::*;

pub use batch::{BatchFailure, BatchResult};
#[allow(deprecated)]
pub use core::{CRUDResource, MergeIntoActiveModel, PrimaryKeyType, UuidIdResult};
pub use errors::ApiError;
pub use filtering::{
    BatchOptions, FilterOperator, FilterOptions, JoinedColumnDef, JoinedFilter, ParsedFilters,
    SortConfig, apply_filters, apply_filters_with_joins, build_comparison_expr,
    calculate_content_range, parse_dot_notation, parse_pagination, parse_range, parse_sorting,
    parse_sorting_with_joins, table_column_ref,
};
#[allow(deprecated)]
pub use operations::{CRUDOperations, DefaultCRUDOperations};
pub use profile::SecurityProfile;
pub use scope::{ScopeCondition, ScopeFilterable};

/// Re-export of the `sea-orm` version crudcrate builds against, so applications
/// can use `crudcrate::sea_orm::...` instead of pinning a matching version
/// themselves.
pub use sea_orm;

#[doc(hidden)]
pub use serde_with;

/// Re-exported so derive-generated join code can call `crudcrate::tracing::warn!`
/// without requiring downstream crates to declare a direct `tracing` dependency.
#[doc(hidden)]
pub use tracing;
