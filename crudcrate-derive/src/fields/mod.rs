//! Field extraction, analysis, and type utilities.

pub(crate) mod analysis;
pub(crate) mod extraction;

pub(crate) use analysis::{analyze_entity_fields, validate_field_analysis};
pub(crate) use extraction::{extract_entity_fields, extract_named_fields, parse_entity_attributes};
