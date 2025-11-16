//! Consolidated join loading code generation
//!
//! This module provides shared logic for generating join loading code for both
//! get_one() and get_all() methods, eliminating the duplication between
//! handlers/get.rs and join_strategies/recursion.rs

use crate::codegen::join_strategies::get_join_config;
use crate::codegen::type_resolution::{
    extract_api_struct_type_for_recursive_call, extract_option_or_direct_inner_type,
    get_path_from_field_type, is_vec_type,
};
use crate::traits::crudresource::structs::EntityFieldAnalysis;
use quote::quote;

/// Generate join loading code for get_one() method
pub fn generate_get_one_join_loading(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    // Check if there are any join fields
    if analysis.join_on_one_fields.is_empty() && analysis.join_on_all_fields.is_empty() {
        return quote! { Ok(model.into()) };
    }

    // Deduplicate fields (some may have both join(one) and join(all))
    let mut seen_fields = std::collections::HashSet::new();
    let mut join_fields: Vec<&syn::Field> = Vec::new();

    for field in analysis.join_on_one_fields.iter().chain(analysis.join_on_all_fields.iter()) {
        if field.ident.as_ref().map_or(true, |name| seen_fields.insert(name.to_string())) {
            join_fields.push(field);
        }
    }

    generate_join_loading_impl(&join_fields, "get_one")
}

/// Generate join loading code for get_all() method
pub fn generate_get_all_join_loading(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    if analysis.join_on_all_fields.is_empty() {
        return quote! {};
    }
    let join_fields: Vec<&syn::Field> = analysis.join_on_all_fields.iter().copied().collect();
    generate_join_loading_impl(&join_fields, "get_all")
}

/// Shared implementation for generating join loading code
fn generate_join_loading_impl(
    join_fields: &[&syn::Field],
    context: &str,
) -> proc_macro2::TokenStream {
    let mut loading_statements = Vec::new();
    let mut field_assignments = Vec::new();

    for field in join_fields {
        let Some(field_name) = &field.ident else {
            continue;
        };

        let join_config = get_join_config(field).unwrap_or_default();
        let is_vec_field = is_vec_type(&field.ty);
        let depth_limited = join_config.depth == Some(1);

        // Get entity path (custom or derived from type)
        let entity_path = if let Some(custom_path) = &join_config.path {
            let path_tokens: proc_macro2::TokenStream = custom_path.parse().unwrap();
            quote! { #path_tokens::Entity }
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
                    let related_models = model.find_related(#entity_path).all(db).await.unwrap_or_default();
                    let #loaded_var: Vec<#api_struct_type> = related_models
                        .into_iter()
                        .map(Into::into)
                        .collect();
                });
                field_assignments.push(quote! { result.#field_name = #loaded_var; });
            } else {
                // Unlimited depth: Recursive loading via get_one()
                loading_statements.push(quote! {
                    let related_models = model.find_related(#entity_path).all(db).await.unwrap_or_default();
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
                        .await
                        .unwrap_or_default()
                        .map(Into::into);
                });
                field_assignments.push(quote! {
                    result.#field_name = #loaded_var.unwrap_or_default();
                });
            } else {
                // Unlimited depth: Recursive loading via get_one()
                loading_statements.push(quote! {
                    let #field_name = match model.find_related(#entity_path).one(db).await.unwrap_or_default() {
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
                    result.#field_name = #field_name.unwrap_or_default();
                });
            }
        }
    }

    // For get_all, we're in a loop context and need to return the result directly
    // For get_one, we return Ok(result) as the method return value
    if context == "get_all" {
        quote! {
            #( #loading_statements )*
            let mut result: Self = model.into();
            #( #field_assignments )*
            result
        }
    } else {
        quote! {
            #( #loading_statements )*
            let mut result: Self = model.into();
            #( #field_assignments )*
            Ok(result)
        }
    }
}
