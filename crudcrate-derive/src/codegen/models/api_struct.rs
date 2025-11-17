//! API struct generation - creates the main API model from entity fields
//!
//! This module handles generation of:
//! - API struct field definitions
//! - From<Model> conversion assignments
//! - DateTime timezone conversion
//! - Join field initialization

use crate::attribute_parser;
use crate::codegen::joins::get_join_config;
use crate::fields;
use crate::traits::crudresource::structs::EntityFieldAnalysis;
use quote::{quote, ToTokens};

/// Generates API struct fields and From<Model> conversion assignments
/// Returns (field_definitions, from_model_assignments)
pub(crate) fn generate_api_struct_content(
    analysis: &EntityFieldAnalysis,
) -> (
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
) {
    let mut api_struct_fields = Vec::new();
    let mut from_model_assignments = Vec::new();

    // Process database fields
    for field in &analysis.db_fields {
        let field_name = &field.ident;
        let field_type = &field.ty;

        // Filter out sea_orm attributes (not needed in API struct)
        let api_field_attrs: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| !attr.path().is_ident("sea_orm"))
            .collect();

        api_struct_fields.push(quote! {
            #(#api_field_attrs)*
            pub #field_name: #field_type
        });

        // Generate From<Model> assignment with DateTimeWithTimeZone conversion
        let assignment = if field_type
            .to_token_stream()
            .to_string()
            .contains("DateTimeWithTimeZone")
        {
            if fields::field_is_optional(field) {
                quote! {
                    #field_name: model.#field_name.map(|dt| dt.with_timezone(&chrono::Utc))
                }
            } else {
                quote! {
                    #field_name: model.#field_name.with_timezone(&chrono::Utc)
                }
            }
        } else {
            quote! {
                #field_name: model.#field_name
            }
        };

        from_model_assignments.push(assignment);
    }

    // Process non-database fields (joins, computed fields, etc.)
    for field in &analysis.non_db_fields {
        let field_name = &field.ident;
        let field_type = &field.ty;

        let default_expr = attribute_parser::get_crudcrate_expr(field, "default")
            .unwrap_or_else(|| syn::parse_quote!(Default::default()));

        // Preserve crudcrate attributes
        let crudcrate_attrs: Vec<_> = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("crudcrate"))
            .collect();

        // Add schema(no_recursion) for join fields (prevents utoipa circular dependencies)
        let schema_attrs = if get_join_config(field).is_some() {
            quote! { #[schema(no_recursion)] }
        } else {
            quote! {}
        };

        let final_field_type = quote! { #field_type };

        let field_definition = quote! {
            #schema_attrs
            #(#crudcrate_attrs)*
            pub #field_name: #final_field_type
        };

        api_struct_fields.push(field_definition);

        // Generate From<Model> assignment with proper defaults
        let assignment = if get_join_config(field).is_some() {
            // Join fields get empty values (loaded separately)
            let empty_value = if let Ok(syn::Type::Path(type_path)) =
                syn::parse2::<syn::Type>(quote! { #final_field_type })
            {
                if let Some(segment) = type_path.path.segments.last() {
                    if segment.ident == "Vec" {
                        quote! { vec![] }
                    } else if segment.ident == "Option" {
                        quote! { None }
                    } else {
                        quote! { Default::default() }
                    }
                } else {
                    quote! { Default::default() }
                }
            } else {
                quote! { Default::default() }
            };

            quote! {
                #field_name: #empty_value
            }
        } else {
            quote! {
                #field_name: #default_expr
            }
        };

        from_model_assignments.push(assignment);
    }

    (api_struct_fields, from_model_assignments)
}
