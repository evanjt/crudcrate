//! List model fields and conversions.

use crate::attrs::get_crudcrate_expr;
use crate::attrs::get_join_config;
use crate::codegen::models::shared::{
    generate_target_model_conversion, resolve_field_type_with_target_models, wire_attrs,
};
use crate::codegen::models::should_include_in_model;
use crate::ir::EntityFieldAnalysis;
use crate::syn_type::resolve_dtwtz;
use crate::syn_type::{is_option_type, is_vec_type, transform_type_to_list_variant};
use quote::quote;

pub(crate) fn generate_list_struct_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    api_struct_name: &syn::Ident,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| should_include_in_model(field, "list_model"))
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;

            // For join(all) fields, use the List variant of the inner type
            // so that exclude(list) on child fields is respected
            let is_join_all = get_join_config(field).is_some_and(|c| c.on_all);
            let final_ty = if is_join_all {
                transform_type_to_list_variant(ty, api_struct_name)
            } else {
                // Resolve type with target models (list model)
                resolve_field_type_with_target_models(ty, field, |_, _, list| list.clone())
            };

            let resolved_ty = resolve_dtwtz(&final_ty);
            let attrs = wire_attrs(field);
            quote! {
                #(#attrs)*
                pub #ident: #resolved_ty
            }
        })
        .collect()
}

pub(crate) fn generate_list_from_assignments(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| should_include_in_model(field, "list_model"))
        .map(|field| {
            let ident = &field.ident;

            // For join(all) fields, convert the inner API struct to its List
            // variant: Vec<Child> element-wise, Option<Child> via map.
            let is_join_all = get_join_config(field).is_some_and(|c| c.on_all);
            if is_join_all && is_vec_type(&field.ty) {
                return quote! {
                    #ident: model.#ident.into_iter().map(Into::into).collect()
                };
            }
            if is_join_all && is_option_type(&field.ty) {
                return quote! {
                    #ident: model.#ident.map(Into::into)
                };
            }

            // Try to generate target model conversion, fallback to direct assignment
            generate_target_model_conversion(field, ident.as_ref()).unwrap_or_else(|| {
                quote! {
                    #ident: model.#ident
                }
            })
        })
        .collect()
}

pub(crate) fn generate_list_from_model_assignments(
    analysis: &EntityFieldAnalysis,
) -> Vec<proc_macro2::TokenStream> {
    let mut assignments = Vec::new();

    // Handle DB fields that are included in ListModel
    for field in &analysis.db_fields {
        let field_name = &field.ident;

        if should_include_in_model(field, "list_model") {
            // Field is included in ListModel - use actual data from Model
            if let Some(conversion) = generate_target_model_conversion(field, field_name.as_ref()) {
                assignments.push(conversion);
                continue;
            }

            assignments.push(quote! {
                #field_name: model.#field_name
            });
        }
        // Fields with list_model = false are not included in ListModel struct, so skip them
    }

    // Handle non-DB fields - use defaults since they don't exist in Model
    for field in &analysis.non_db_fields {
        let field_name = &field.ident;

        if should_include_in_model(field, "list_model") {
            // Check if this is a join(all) field
            let is_join_all = get_join_config(field).is_some_and(|c| c.on_all);

            if is_join_all {
                // Join(all) fields are populated by the get_all() loading logic;
                // From<Model> only needs an empty placeholder. Vec<Child> uses an
                // empty vec, Option<Child> uses None.
                if is_option_type(&field.ty) {
                    assignments.push(quote! {
                        #field_name: None
                    });
                } else {
                    assignments.push(quote! {
                        #field_name: vec![]
                    });
                }
            } else {
                // Regular non-DB fields: use default or specified default
                let default_expr = get_crudcrate_expr(field, "default")
                    .unwrap_or_else(|| syn::parse_quote!(Default::default()));
                assignments.push(quote! {
                    #field_name: #default_expr
                });
            }
        }
        // Fields with list_model = false or join(one)-only fields are not included in ListModel struct, so skip them
    }

    assignments
}
