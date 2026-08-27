//! Consolidated join loading code generation
//!
//! This module provides shared logic for generating join loading code for both
//! `get_one()` and `get_all()` methods, eliminating the duplication between
//! handlers/get.rs and joins/recursion.rs
//!
//! ## Security Limits
//!
//! **Regular Joins - `MAX_JOIN_DEPTH` = 5**: Cross-model join recursion is capped at depth 5 to prevent:
//! - Infinite recursion with circular references
//! - Exponential query growth (N+1 problem)
//! - Database connection pool exhaustion
//!
//! **Self-Referencing Joins - Depth = 1 Only**: Self-referencing fields (e.g., `Category { children: Vec<Category> }`)
//! are automatically limited to depth=1 to prevent exponential query growth. This means self-referencing fields
//! will load immediate children only, without recursive nesting. Depths > 1 are a compile error.
//!
//! **To use deeper joins**:
//! - Explicitly set `depth` parameter: `#[crudcrate(join(all, depth = 3))]`
//! - Regular joins (cross-model): Maximum 5 (values > 5 are capped to 5)
//! - Self-referencing: Always 1 (values > 1 are a compile error)
//! - Unspecified depth defaults to 5 for regular joins, 1 for self-referencing
//!
//! **Example**:
//! ```ignore
//! // Regular joins (different models)
//! #[crudcrate(join(all, depth = 1))]  // Shallow: load related entities only
//! pub users: Vec<User>
//!
//! #[crudcrate(join(all, depth = 3))]  // Medium: 3 levels deep
//! pub organization: Option<Organization>
//!
//! #[crudcrate(join(all))]  // Defaults to depth = 5 (maximum)
//! pub vehicles: Vec<Vehicle>
//!
//! // Self-referencing joins (same model) - always depth=1 only
//! #[crudcrate(join(all))]  // Loads immediate children only
//! pub children: Vec<Category>
//!
//! #[crudcrate(join(all, depth = 5))]  // Compile error: self-references only support depth = 1
//! pub subcategories: Vec<Category>
//! ```

pub(crate) mod batch;
pub(crate) mod filter_sort;
pub(crate) mod fk;
pub(crate) mod per_row;
