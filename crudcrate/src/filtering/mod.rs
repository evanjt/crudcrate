//! Filtering, sorting, pagination, and fulltext search.
//!
//! This module translates query parameters into Sea-ORM conditions. When you mark fields
//! with `#[crudcrate(filterable)]`, `#[crudcrate(sortable)]`, or `#[crudcrate(fulltext)]`,
//! the generated handlers use these utilities to parse incoming requests.
//!
//! # Query parameter reference
//!
//! ## Filtering
//!
//! Filters are passed as a JSON object in the `filter` query parameter. Operator suffixes
//! on field names control the comparison:
//!
//! | Suffix | SQL equivalent | Example |
//! |--------|---------------|---------|
//! | *(none)* | `= value` | `{"completed": false}` |
//! | `_neq` | `!= value` | `{"status_neq": "archived"}` |
//! | `_gt` | `> value` | `{"priority_gt": 3}` |
//! | `_gte` | `>= value` | `{"priority_gte": 3}` |
//! | `_lt` | `< value` | `{"priority_lt": 10}` |
//! | `_lte` | `<= value` | `{"priority_lte": 10}` |
//! | `_like` | `LIKE %value%` | `{"name_like": "john"}` |
//!
//! Multiple values for the same field (comma-separated) produce an `IN` clause.
//!
//! ## Fulltext search
//!
//! The `q` parameter searches across all fields marked `fulltext`:
//!
//! ```text
//! GET /todos?q=urgent review
//! ```
//!
//! On PostgreSQL this uses `to_tsvector`/`to_tsquery` with GIN indexes.
//! On MySQL it uses `MATCH ... AGAINST`. On SQLite it falls back to `LIKE`.
//!
//! ## Sorting
//!
//! ```text
//! GET /todos?sort=["created_at","DESC"]
//! GET /todos?sort=created_at&order=DESC
//! ```
//!
//! ## Pagination
//!
//! ```text
//! GET /todos?range=[0,24]              # React Admin style
//! GET /todos?page=1&per_page=25        # page-based
//! ```
//!
//! Responses include `Content-Range` and `X-Total-Count` headers.
//!
//! # Key types
//!
//! - [`FilterOptions`] — parsed query parameters for a list request
//! - [`BatchOptions`] — options for batch endpoints (e.g. `?partial=true`)
//! - [`apply_filters`] — builds a Sea-ORM `Condition` from filter params
//! - [`parse_sorting`] — resolves sort parameters to column + direction
//! - [`parse_pagination`] — extracts offset/limit from query params

pub mod conditions;
pub mod joined;
pub mod pagination;
pub mod query_parser;
pub mod search;
pub mod sort;

// Re-export commonly used items
pub use conditions::{apply_filters, apply_filters_with_joins, parse_pagination, parse_range};
pub use joined::{
    FilterOperator, JoinedColumnDef, JoinedFilter, ParsedFilters, SortConfig, parse_dot_notation,
};
pub use pagination::calculate_content_range;
pub use query_parser::{BatchOptions, FilterOptions};
pub use search::build_fulltext_condition;
pub use sort::{parse_sorting, parse_sorting_with_joins};
