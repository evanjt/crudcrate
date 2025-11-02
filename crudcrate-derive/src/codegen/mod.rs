//! Code generation modules for CRUD operations
//!
//! This module contains the code generation logic split into focused modules:
//! - model_generators: Generate Create/Update/List/Response models
//! - recursive_loading: Handle join loading and recursion logic
//! - trait_implementations: Generate CRUDResource trait implementations
//! - type_resolution: Complex type extraction and resolution

pub mod model_generators;

// Re-export for backward compatibility
pub use model_generators::*;
pub mod handler;
pub mod join;
pub mod model;
pub mod router;
pub mod type_resolution;
