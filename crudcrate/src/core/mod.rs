//! # Auto-Generated CRUD Operations
//!
//! This module provides the core functionality for automatically generating complete CRUD (Create, Read, Update, Delete) operations from Sea-ORM entities.
//!
//! ## Main Components
//!
//! - **[`CRUDResource`](@/traits/trait.CRUDResource.html)**: Central trait that defines CRUD operations, filtering, and sorting capabilities
//! - **[`MergeIntoActiveModel`](@/traits/trait.MergeIntoActiveModel.html)**: Helper trait for merging API structs into ActiveModel
//!
//! ## Generated Operations
//!
//! When you use `#[derive(EntityToModels)]`, the following operations are automatically generated:
//!
//! ### HTTP Endpoints
//! - `GET /resource` - List all items with filtering and pagination
//! - `GET /resource/{id}` - Get specific item by ID
//! - `POST /resource` - Create new item
//! - `PUT /resource/{id}` - Update existing item
//! - `DELETE /resource/{id}` - Delete specific item
//! - `DELETE /resource` - Bulk delete with filtering
//!
//! ### Generated Structs
//! - **API Struct** (e.g., `Todo`): For HTTP responses
//! - **Create Struct** (e.g., `TodoCreate`): For POST requests, excludes auto-generated fields
//! - **Update Struct** (e.g., `TodoUpdate`): For PUT requests, excludes primary keys
//! - **List Struct** (e.g., `TodoList`): Optimized for list responses
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use crudcrate::core::CRUDResource;
//! use sea_orm::DatabaseConnection;
//!
//! // The CRUDResource trait is automatically implemented
//! // when you use #[derive(EntityToModels)]
//!
//! // Manual usage (usually not needed - use generated router instead):
//! let todo = Todo::get_one(&db, id).await?;
//! let todos = Todo::get_all(&db, Some(filter_options)).await?;
//! let created = Todo::create(&db, todo_create).await?;
//! let updated = Todo::update(&db, id, todo_update).await?;
//! let deleted = Todo::delete(&db, id).await?;
//! ```

pub mod traits;
pub mod crud_operations;

// Re-export commonly used items
pub use traits::{CRUDResource, MergeIntoActiveModel};