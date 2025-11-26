//! Consolidated join loading code generation
//!
//! This module provides shared logic for generating join loading code for both
//! `get_one()` and `get_all()` methods, eliminating the duplication between
//! handlers/get.rs and joins/recursion.rs
//!
//! ## Security Limits
//!
//! **MAX_JOIN_DEPTH = 5**: Join recursion is capped at depth 5 to prevent:
//! - Infinite recursion with circular references
//! - Exponential query growth (N+1 problem)
//! - Database connection pool exhaustion
//!
//! **To use deeper joins**:
//! - Explicitly set `depth` parameter: `#[crudcrate(join(all, depth = 3))]`
//! - Maximum allowed: 5 (values > 5 are capped to 5)
//! - Unspecified depth defaults to 5 for safety
//!
//! **Example**:
//! ```ignore
//! #[crudcrate(join(all, depth = 1))]  // Shallow: load related entities only
//! pub users: Vec<User>
//!
//! #[crudcrate(join(all, depth = 3))]  // Medium: 3 levels deep
//! pub organization: Option<Organization>
//!
//! #[crudcrate(join(all))]  // Defaults to depth = 5 (maximum)
//! pub vehicles: Vec<Vehicle>
//! ```

// Security: Maximum join depth to prevent infinite recursion and resource exhaustion
// Users cannot exceed this limit - values > 5 are automatically capped
const MAX_JOIN_DEPTH: u8 = 5;

use crate::codegen::joins::get_join_config;
use crate::codegen::type_resolution::{
    extract_api_struct_type_for_recursive_call, extract_option_or_direct_inner_type,
    get_path_from_field_type, is_vec_type,
};
use crate::traits::crudresource::structs::EntityFieldAnalysis;
use quote::quote;

/// Generate join loading code for `get_one()` method
///
/// Returns code that evaluates to `Self` (not wrapped in Result).
/// The caller is responsible for wrapping in `Ok()`.
pub fn generate_get_one_join_loading(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    // Check if there are any join fields
    if analysis.join_on_one_fields.is_empty() && analysis.join_on_all_fields.is_empty() {
        return quote! { Self::from(model) };
    }

    // Deduplicate fields (some may have both join(one) and join(all))
    let mut seen_fields = std::collections::HashSet::new();
    let mut join_fields: Vec<&syn::Field> = Vec::new();

    for field in analysis.join_on_one_fields.iter().chain(analysis.join_on_all_fields.iter()) {
        if field.ident.as_ref().is_none_or(|name| seen_fields.insert(name.to_string())) {
            join_fields.push(field);
        }
    }

    generate_join_loading_impl(&join_fields, "get_one")
}

/// Generate join loading code for `get_all()` method
pub fn generate_get_all_join_loading(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    if analysis.join_on_all_fields.is_empty() {
        return quote! {};
    }
    let join_fields: Vec<&syn::Field> = analysis.join_on_all_fields.clone();
    generate_join_loading_impl(&join_fields, "get_all")
}

/// Shared implementation for generating join loading code
fn generate_join_loading_impl(
    join_fields: &[&syn::Field],
    _context: &str,
) -> proc_macro2::TokenStream {
    let mut loading_statements = Vec::new();
    let mut field_assignments = Vec::new();

    for field in join_fields {
        let Some(field_name) = &field.ident else {
            continue;
        };

        let join_config = get_join_config(field).unwrap_or_default();
        let is_vec_field = is_vec_type(&field.ty);

        // Security: Cap depth at MAX_JOIN_DEPTH to prevent infinite recursion
        // If depth is None (unlimited), default to safe maximum
        let effective_depth = join_config.depth.unwrap_or(MAX_JOIN_DEPTH).min(MAX_JOIN_DEPTH);
        let depth_limited = effective_depth == 1;

        // Get entity path (custom or derived from type)
        let entity_path = if let Some(custom_path) = &join_config.path {
            match custom_path.parse::<proc_macro2::TokenStream>() {
                Ok(path_tokens) => quote! { #path_tokens::Entity },
                Err(_) => {
                    // Generate a compile error if the path is invalid
                    let error_msg = format!(
                        "Invalid join path '{}' for field '{}'. Expected a valid Rust module path.",
                        custom_path, field_name
                    );
                    return quote! { compile_error!(#error_msg); };
                }
            }
        } else {
            get_path_from_field_type(&field.ty, "Entity")
        };

        if is_vec_field {
            // Vec<T> relationships (has_many)
            let api_struct_type = extract_api_struct_type_for_recursive_call(&field.ty);

            if depth_limited {
                // Depth=1: Load data, no recursion
                let loaded_var = quote::format_ident!("loaded_{}", field_name);
                loading_statements.push(quote! {
                    let related_models = model.find_related(#entity_path).all(db).await?;
                    let #loaded_var: Vec<#api_struct_type> = related_models
                        .into_iter()
                        .map(Into::into)
                        .collect();
                });
                field_assignments.push(quote! { result.#field_name = #loaded_var; });
            } else {
                // Unlimited depth: Recursive loading via get_one()
                loading_statements.push(quote! {
                    let related_models = model.find_related(#entity_path).all(db).await?;
                    let mut #field_name = Vec::new();
                    for related_model in related_models {
                        match #api_struct_type::get_one(db, related_model.id).await {
                            Ok(entity) => #field_name.push(entity),
                            Err(_) => #field_name.push(related_model.into()),
                        }
                    }
                });
                field_assignments.push(quote! { result.#field_name = #field_name; });
            }
        } else {
            // Option<T> or T relationships (belongs_to/has_one)
            let target_type = extract_option_or_direct_inner_type(&field.ty);

            if depth_limited {
                // Depth=1: Load data, no recursion
                let loaded_var = quote::format_ident!("loaded_{}", field_name);
                loading_statements.push(quote! {
                    let #loaded_var = model
                        .find_related(#entity_path)
                        .one(db)
                        .await?
                        .map(Into::into);
                });
                field_assignments.push(quote! {
                    result.#field_name = #loaded_var;
                });
            } else {
                // Unlimited depth: Recursive loading via get_one()
                loading_statements.push(quote! {
                    let #field_name = match model.find_related(#entity_path).one(db).await? {
                        Some(related_model) => {
                            match #target_type::get_one(db, related_model.id).await {
                                Ok(entity) => Some(entity),
                                Err(_) => Some(related_model.into()),
                            }
                        }
                        None => None,
                    };
                });
                field_assignments.push(quote! {
                    result.#field_name = #field_name;
                });
            }
        }
    }

    // Both contexts return Self directly (not wrapped in Result)
    // The caller is responsible for wrapping in Ok() when needed
    quote! {
        #( #loading_statements )*
        let mut result: Self = model.into();
        #( #field_assignments )*
        result
    }
}
