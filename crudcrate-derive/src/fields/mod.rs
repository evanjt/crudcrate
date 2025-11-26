//! Field extraction, analysis, and type utilities.

pub mod analysis;
pub mod extraction;
pub mod type_utils;

pub use analysis::{analyze_entity_fields, validate_field_analysis};
pub use extraction::{extract_entity_fields, extract_named_fields, parse_entity_attributes};
pub use type_utils::{field_is_optional, resolve_target_models, resolve_target_models_with_list};
