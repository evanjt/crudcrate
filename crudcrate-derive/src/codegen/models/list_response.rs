//! List and Response model generation orchestration
//!
//! This module coordinates the generation of both List and Response models
//! from entity definitions, using the dedicated list and response generators.

use crate::fields;
use crate::traits::crudresource::structs::EntityFieldAnalysis;
use quote::{format_ident, quote};
use syn::DeriveInput;

/// Generates both List and Response models from entity definition
///
/// Returns a tuple of (`list_model_tokens`, `response_model_tokens`)
pub(crate) fn generate_list_and_response_models(
    input: &DeriveInput,
    api_struct_name: &syn::Ident,
    struct_name: &syn::Ident,
    field_analysis: &EntityFieldAnalysis,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    // Generate List model
    let list_name = format_ident!("{}List", api_struct_name);
    let raw_fields = match fields::extract_named_fields(input) {
        Ok(f) => f,
        Err(_e) => {
            // This shouldn't happen since we validated earlier at entry point
            // Return empty token stream - error already emitted
            return (quote::quote! {}, quote::quote! {});
        }
    };
    let list_struct_fields = crate::codegen::models::list::generate_list_struct_fields(&raw_fields);
    let list_from_assignments =
        crate::codegen::models::list::generate_list_from_assignments(&raw_fields);
    let list_from_model_assignments =
        crate::codegen::models::list::generate_list_from_model_assignments(field_analysis);

    let list_derives =
        quote! { Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let list_model = quote! {
        #[derive(#list_derives)]
        pub struct #list_name {
            #(#list_struct_fields),*
        }

        impl From<#api_struct_name> for #list_name {
            fn from(model: #api_struct_name) -> Self {
                Self {
                    #(#list_from_assignments),*
                }
            }
        }

        impl From<#struct_name> for #list_name {
            fn from(model: #struct_name) -> Self {
                Self {
                    #(#list_from_model_assignments),*
                }
            }
        }
    };

    // Generate Response model
    let response_name = format_ident!("{}Response", api_struct_name);
    let response_struct_fields =
        crate::codegen::models::response::generate_response_struct_fields(&raw_fields);
    let response_from_assignments =
        crate::codegen::models::response::generate_response_from_assignments(&raw_fields);

    let response_derives =
        quote! { Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let response_model = quote! {
        #[derive(#response_derives)]
        pub struct #response_name {
            #(#response_struct_fields),*
        }

        impl From<#api_struct_name> for #response_name {
            fn from(model: #api_struct_name) -> Self {
                Self {
                    #(#response_from_assignments),*
                }
            }
        }
    };

    (list_model, response_model)
}
