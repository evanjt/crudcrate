//! Emits the List and Response models for an entity, plus their scoped variants.

use crate::codegen::models::scoped::{generate_scope_filterable_impls, generate_scoped_models};
use crate::fields;
use crate::ir::EntityFieldAnalysis;
use quote::{format_ident, quote};
use syn::DeriveInput;

/// Generates both List and Response models from entity definition
///
/// `struct_level_joins` are synthetic fields from struct-level `join(...)` attributes.
///
/// Returns a tuple of (`list_model_tokens`, `response_model_tokens`)
pub(crate) fn generate_list_and_response_models(
    input: &DeriveInput,
    api_struct_name: &syn::Ident,
    struct_name: &syn::Ident,
    field_analysis: &EntityFieldAnalysis,
    struct_level_joins: &[syn::Field],
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    // Generate List model
    let list_name = format_ident!("{}List", api_struct_name);
    let raw_fields = match fields::extract_named_fields(input) {
        Ok(f) => f,
        Err(_e) => {
            return (quote::quote! {}, quote::quote! {});
        }
    };

    // Combine real fields with synthetic join fields
    let mut all_fields = raw_fields.clone();
    for field in struct_level_joins {
        all_fields.push(field.clone());
    }

    let list_struct_fields =
        crate::codegen::models::list::generate_list_struct_fields(&all_fields, api_struct_name);
    let list_from_assignments =
        crate::codegen::models::list::generate_list_from_assignments(&all_fields);
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
    let response_struct_fields = crate::codegen::models::response::generate_response_struct_fields(
        &all_fields,
        api_struct_name,
    );
    let response_from_assignments =
        crate::codegen::models::response::generate_response_from_assignments(&all_fields);

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

    let scoped_models =
        generate_scoped_models(&all_fields, api_struct_name, &list_name, &response_name);
    let scope_filterable_impls =
        generate_scope_filterable_impls(&all_fields, api_struct_name, &list_name);

    let combined_list = quote! { #list_model #scoped_models #scope_filterable_impls };
    (combined_list, response_model)
}
