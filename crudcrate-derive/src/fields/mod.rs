//! Field utilities module
//!
//! This module provides comprehensive field processing capabilities for entity structs:
//!
//! ## Submodules
//!
//! - **extraction**: Extract fields from struct definitions and parse entity attributes
//! - **analysis**: Analyze and categorize fields by their attributes (db/non-db, joins, etc.)
//! - **type_utils**: Type introspection (Option detection, target model resolution)
//!
//! ## Usage
//!
//! ```ignore
//! // Extract fields from struct
//! let fields = extraction::extract_entity_fields(&input)?;
//!
//! // Analyze and categorize
//! let analysis = analysis::analyze_entity_fields(fields);
//!
//! // Validate configuration
//! analysis::validate_field_analysis(&analysis)?;
//!
//! // Check field types
//! if type_utils::field_is_optional(field) { ... }
//! ```

pub mod analysis;
pub mod extraction;
pub mod type_utils;

// Re-export commonly used functions for convenience
pub use analysis::{analyze_entity_fields, validate_field_analysis};
pub use extraction::{extract_entity_fields, extract_named_fields, has_sea_orm_ignore, parse_entity_attributes};
pub use type_utils::{field_is_optional, resolve_target_models, resolve_target_models_with_list};
