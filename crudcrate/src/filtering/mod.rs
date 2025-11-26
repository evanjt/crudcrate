//! # Advanced Filtering & Search
//!
//! This module provides comprehensive query parameter to SQL condition translation with fulltext search support. It enables rich filtering APIs without writing SQL manually.
//!
//! ## Key Features
//!
//! - **Query Parameter Parsing**: Automatically converts URL parameters to SQL conditions
//! - **Fulltext Search**: Database-optimized search with `PostgreSQL` GIN indexes and `MySQL` FULLTEXT
//! - **Type Safety**: Automatic type validation and conversion
//! - **Multi-Database**: Optimized queries for `SQLite`, `PostgreSQL`, and `MySQL`
//!
//! ## Main Components
//!
//! - **[`FilterOptions`](@/query_parser/struct.FilterOptions.html)**: Main filtering configuration
//! - **[`apply_filters`](@/conditions/fn.apply_filters.html)**: Core filtering application
//! - **[`parse_sorting`](@/sort/fn.parse_sorting.html)**: Sorting parameter parsing
//! - **[`parse_pagination`](@/pagination/fn.parse_pagination.html)**: Pagination support
//!
//! ## Query Parameter Examples
//!
//! ### Basic Filtering
//! ```rust,ignore
//! // Simple equality
//! GET /todos?completed=true
//!
//! // String contains (LIKE query)
//! GET /todos?title_like=example
//!
//! // Numeric comparisons
//! GET /todos?priority_gte=5
//! GET /todos?priority_lte=10
//! GET /todos?priority_gt=3
//!
//! // List operations (IN query)
//! GET /todos?id=uuid1,uuid2,uuid3
//! ```
//!
//! ### Advanced Queries
//! ```rust,ignore
//! // Multiple filters combined with AND
//! GET /todos?completed=false&priority_gte=3&title_like=urgent
//!
//! // Date range filtering
//! GET /todos?created_at_gte=2024-01-01T00:00:00Z
//! GET /todos?created_at_lte=2024-12-31T23:59:59Z
//!
//! // Sorting (comma-separated)
//! GET /todos?sort=created_at_desc,priority_asc,title
//!
//! // Fulltext search (database-optimized)
//! GET /todos?q=search terms
//!
//! // Pagination with Range headers
//! GET /todos
//! Range: items=0-24
//! ```
//!
//! ## Database Optimizations
//!
//! ### `PostgreSQL`
//! - GIN indexes for fulltext search
//! - tsvector columns for optimized search
//! - JSON operations for complex filters
//!
//! ### `MySQL`
//! - FULLTEXT indexes for search optimization
//! - Spatial data support
//! - Optimized LIKE queries
//!
//! ### `SQLite`
//! - LIKE-based fallback for search
//! - Best for development and testing
//! - Fast in-memory operations
//!
//! ## Usage in Generated APIs
//!
//! When you use `#[derive(EntityToModels)]`, filtering is automatically integrated into your CRUD endpoints:
//!
//! ```rust,ignore
//! # use crudcrate::EntityToModels;
//! # use sea_orm::entity::prelude::*;
//! #[derive(Clone, Debug, PartialEq, DeriveEntityModel, EntityToModels)]
//! #[sea_orm(table_name = "todos")]
//! #[crudcrate(api_struct = "Todo", generate_router)]
//! pub struct Model {
//!     #[sea_orm(primary_key)]
//!     pub id: i32,
//!
//!     #[crudcrate(filterable, sortable, fulltext)]
//!     pub title: String,
//!
//!     #[crudcrate(filterable, sortable)]
//!     pub completed: bool,
//!
//!     #[crudcrate(filterable, sortable)]
//!     pub priority: i32,
//! }
//!
//! // This automatically enables:
//! // GET /todos?title_like=urgent&completed=false&priority_gte=3&sort=priority_desc
//! // GET /todos?q=search terms&completed=true
//! // GET /todos?priority_gte=5&sort=created_at_desc
//! ```

pub mod query_parser;
pub mod search;
pub mod conditions;
pub mod sort;
pub mod pagination;
pub mod joined;

// Re-export commonly used items
pub use query_parser::FilterOptions;
pub use conditions::{apply_filters, apply_filters_with_joins, parse_pagination, parse_range};
pub use search::build_fulltext_condition;
pub use sort::{parse_sorting, parse_sorting_with_joins};
pub use pagination::calculate_content_range;
pub use joined::{JoinedColumnDef, JoinedFilter, FilterOperator, ParsedFilters, SortConfig, parse_dot_notation};