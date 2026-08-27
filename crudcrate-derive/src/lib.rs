// Codegen functions are dominated by single quote! blocks that do not divide
// into meaningful helpers.
#![allow(clippy::too_many_lines)]

//! Procedural macros for generating CRUD operations from Sea-ORM entities.
//!
//! **Main macro**: `#[derive(EntityToModels)]` - see [`entity_to_models`]
//!
//! # Available Attributes
//!
//! ## Struct-Level Attributes
//!
//! Use on the struct with `#[crudcrate(...)]`:
//!
//! | Attribute | Type | Description |
//! |-----------|------|-------------|
//! | `generate_router` | flag | Generate Axum router function |
//! | `api_struct = "Name"` | string | Override generated struct name |
//! | `name_singular = "item"` | string | Singular resource name for errors/headers |
//! | `name_plural = "items"` | string | Plural resource name for routes |
//! | `description = "..."` | string | `OpenAPI` description |
//! | `fulltext_language = "english"` | string | `PostgreSQL` fulltext language |
//! | `batch_limit = 100` | integer | Max items for batch create/update/delete |
//! | `max_page_size = 1000` | integer | Max items per page for pagination |
//! | `operations = MyOps` | path | Custom `CRUDOperations` implementation |
//! | `derive_partial_eq` | flag | Derive `PartialEq` on generated structs |
//! | `derive_eq` | flag | Derive `Eq` on generated structs |
//!
//! ### Hook Attributes
//!
//! Format: `{operation}::{cardinality}::{phase} = function_name`
//!
//! | Operation | Cardinality | Phase | Description |
//! |-----------|-------------|-------|-------------|
//! | `create` | `one`, `many` | `pre`, `body`, `transform`, `post` | Create hooks |
//! | `read` | `one`, `many` | `pre`, `body`, `transform`, `post` | Read hooks |
//! | `update` | `one`, `many` | `pre`, `body`, `transform`, `post` | Update hooks |
//! | `delete` | `one`, `many` | `pre`, `body`, `transform`, `post` | Delete hooks |
//!
//! Example: `#[crudcrate(create::one::pre = validate_input)]`
//!
//! ## Field-Level Attributes
//!
//! Use on fields with `#[crudcrate(...)]`:
//!
//! | Attribute | Type | Description |
//! |-----------|------|-------------|
//! | `primary_key` | flag | Mark as primary key field |
//! | `filterable` | flag | Enable filtering on this field |
//! | `sortable` | flag | Enable sorting on this field |
//! | `fulltext` | flag | Include in fulltext search |
//! | `exclude(create)` | list | Exclude from create model |
//! | `exclude(update)` | list | Exclude from update model |
//! | `exclude(one)` | list | Exclude from `get_one` response |
//! | `exclude(list)` | list | Exclude from `get_all` response |
//! | `on_create = expr` | expr | Auto-generate value on create |
//! | `on_update = expr` | expr | Auto-generate value on update |
//! | `non_db_attr` | flag | Mark as non-database field (for joins) |
//! | `join(one)` | config | Load in `get_one` only |
//! | `join(all)` | config | Load in `get_all` only |
//! | `join(one, all)` | config | Load in both endpoints |
//! | `join(one, all, depth = N)` | config | With max recursion depth (1-5) |
//! | `join_filterable("col1", "col2")` | list | Enable filtering on join columns |
//! | `join_sortable("col1", "col2")` | list | Enable sorting on join columns |
//!
//! # Examples
//!
//! ```ignore
//! #[derive(EntityToModels)]
//! #[crudcrate(
//!     generate_router,
//!     batch_limit = 500,
//!     create::one::pre = validate_task,
//! )]
//! #[sea_orm(table_name = "tasks")]
//! pub struct Model {
//!     #[sea_orm(primary_key, auto_increment = false)]
//!     #[crudcrate(primary_key, exclude(create, update), on_create = Uuid::new_v4())]
//!     pub id: Uuid,
//!
//!     #[crudcrate(filterable, sortable, fulltext)]
//!     pub title: String,
//!
//!     #[crudcrate(exclude(create, update), on_create = chrono::Utc::now())]
//!     pub created_at: DateTime<Utc>,
//!
//!     #[sea_orm(ignore)]
//!     #[crudcrate(non_db_attr, join(one, all, depth = 2))]
//!     pub comments: Vec<Comment>,
//! }
//! ```
//!
//! **Module guide**: `fields/` (field processing) | `codegen/` (models, handlers, joins, routes)

#[cfg(test)]
mod expand_snapshots;

mod attrs;
mod codegen;
mod expand;
mod fields;
mod ir;
mod macro_implementation;
mod relation_validator;
mod syn_type;

use proc_macro::TokenStream;

/// Generates `<Name>Create` struct with fields not excluded by `exclude(create)`.
/// Fields with `on_create` become `Option<T>` to allow user override.
/// Implements `From<NameCreate>` for `ActiveModel` with automatic value generation.
#[proc_macro_derive(ToCreateModel, attributes(crudcrate, active_model))]
pub fn to_create_model(input: TokenStream) -> TokenStream {
    expand::simple_models::to_create_model_impl(input.into()).into()
}

/// Generates `<Name>Update` struct with fields not excluded by `exclude(update)`.
/// All fields are `Option<Option<T>>` to support partial updates and explicit null.
/// Implements `MergeIntoActiveModel` trait with `on_update` expression handling.
#[proc_macro_derive(ToUpdateModel, attributes(crudcrate, active_model))]
pub fn to_update_model(input: TokenStream) -> TokenStream {
    expand::simple_models::to_update_model_impl(input.into()).into()
}

/// Generates `<Name>List` struct with fields not excluded by `exclude(list)`.
/// Optimizes API payloads by excluding heavy fields (joins, large text) from list endpoints.
/// Implements `From<Name>` and `From<Model>` conversions.
#[proc_macro_derive(ToListModel, attributes(crudcrate))]
pub fn to_list_model(input: TokenStream) -> TokenStream {
    expand::simple_models::to_list_model_impl(input.into()).into()
}

/// Generates complete CRUD API structures from Sea-ORM entities.
///
/// Creates API struct, List/Response models, and `CRUDResource` implementation.
/// Supports custom functions, joins, filtering, sorting, and fulltext search.
///
/// Key attributes: `api_struct`, `generate_router`, `exclude()`, `join()`, `on_create/update`.
/// See crate documentation for full attribute reference and examples.
/// # Panics
///
/// This function will panic in the following cases:
/// - When deprecated syntax is used (e.g., `create_model = false` instead of `exclude(create)`)
/// - When there are cyclic join dependencies without explicit depth specification
/// - When required Sea-ORM relation enums are missing for join fields
#[proc_macro_derive(EntityToModels, attributes(crudcrate))]
pub fn entity_to_models(input: TokenStream) -> TokenStream {
    expand::entity::entity_to_models_impl(input.into()).into()
}
