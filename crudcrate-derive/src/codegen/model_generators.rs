//! Model generation for CRUD operations
//!
//! This module contains functions for generating the various model structs:
//! - Create models (for POST requests)
//! - Update models (for PUT requests)
//! - List models (for optimized list responses)
//! - Response models (for single item responses)

use crate::attribute_parser::{
    field_has_crudcrate_flag, get_crudcrate_bool, get_crudcrate_expr, get_join_config,
};
use crate::field_analyzer::{
    extract_inner_type_for_update, field_is_optional, resolve_target_models,
    resolve_target_models_with_list,
};
use crate::structs::{CRUDResourceMeta, EntityFieldAnalysis};
use heck::ToPascalCase;
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Type, punctuated::Punctuated, token::Comma};
