//! Response model field generation for HTTP API responses.
//!
//! This module generates the field definitions and assignments for Response structs
//! that are returned from GET endpoints. Join fields are included to enable
//! relationship data in API responses.

use crate::attribute_parser::get_crudcrate_bool;
use crate::codegen::joins::config::get_join_config;
use quote::{quote, ToTokens};

/// Generate field assignment expressions for converting API struct to Response.
///
/// Join fields are included so relationship data appears in HTTP responses.
pub(crate) fn generate_response_from_assignments(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| {
            // Filter by one_model attribute (allows excluding fields from single-item responses)
            get_crudcrate_bool(field, "one_model").unwrap_or(true)
        })
        .map(|field| {
            let ident = &field.ident;
            quote! {
                #ident: model.#ident
            }
        })
        .collect()
}

/// Generate field definitions for Response struct.
///
/// Join fields receive `#[schema(no_recursion)]` to prevent infinite OpenAPI schema
/// recursion while still allowing serialization in responses.
pub(crate) fn generate_response_struct_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    api_struct_name: &syn::Ident,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| {
            // Filter by one_model attribute
            get_crudcrate_bool(field, "one_model").unwrap_or(true)
        })
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;

            // Copy non-crudcrate attributes to the response field
            let attrs: Vec<_> = field
                .attrs
                .iter()
                .filter(|attr| !attr.path().is_ident("crudcrate") && !attr.path().is_ident("sea_orm"))
                .collect();

            // Check if this is a self-referencing or join field
            let field_type_string = ty.to_token_stream().to_string();
            let is_self_referencing = field_type_string.contains(&api_struct_name.to_string());
            let is_join_field = get_join_config(field).is_some();

            // Add schema(no_recursion) for self-referencing or join fields to prevent
            // infinite recursion in OpenAPI schema generation
            let schema_attr = if is_self_referencing || is_join_field {
                Some(quote! {
                    #[schema(no_recursion)]
                })
            } else {
                None
            };

            let final_ty = quote! { #ty };

            quote! {
                #schema_attr
                #(#attrs)*
                pub #ident: #final_ty
            }
        })
        .collect()
}
