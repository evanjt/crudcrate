use crate::attribute_parser::{get_crudcrate_bool, get_crudcrate_expr};
use crate::codegen::joins::get_join_config;
use crate::codegen::models::shared::{
    generate_target_model_conversion, resolve_field_type_with_target_models,
};
use crate::codegen::models::should_include_in_model;
use crate::fields::field_is_optional;
use crate::traits::crudresource::structs::EntityFieldAnalysis;
use quote::{ToTokens, quote};

pub(crate) fn generate_list_struct_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| should_include_in_model(field, "list_model"))
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;

            // Resolve type with target models (list model)
            let final_ty = resolve_field_type_with_target_models(ty, field, |_, _, list| list.clone());

            quote! {
                pub #ident: #final_ty
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

            // Try to generate target model conversion, fallback to direct assignment
            generate_target_model_conversion(field, ident).unwrap_or_else(|| {
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

        if get_crudcrate_bool(field, "list_model").unwrap_or(true) {
            // Field is included in ListModel - use actual data from Model
            if let Some(conversion) = generate_target_model_conversion(field, field_name) {
                assignments.push(conversion);
                continue;
            }

            // Handle DateTime conversion for Model -> ListModel
            let field_type = &field.ty;
            if field_type
                .to_token_stream()
                .to_string()
                .contains("DateTimeWithTimeZone")
            {
                if field_is_optional(field) {
                    assignments.push(quote! {
                        #field_name: model.#field_name.map(|dt| dt.with_timezone(&chrono::Utc))
                    });
                } else {
                    assignments.push(quote! {
                        #field_name: model.#field_name.with_timezone(&chrono::Utc)
                    });
                }
            } else {
                // Standard field - use directly from Model
                assignments.push(quote! {
                    #field_name: model.#field_name
                });
            }
        }
        // Fields with list_model = false are not included in ListModel struct, so skip them
    }

    // Handle non-DB fields - use defaults since they don't exist in Model
    for field in &analysis.non_db_fields {
        let field_name = &field.ident;

        let include_in_list = get_crudcrate_bool(field, "list_model").unwrap_or(true);
        // Only exclude join(one) fields from List models - keep join(all) fields since they're meant for list responses
        let is_join_one_only = if let Some(join_config) = get_join_config(field) {
            !join_config.on_all // Exclude if NOT loading in get_all (on_all = false)
        } else {
            false
        };

        if include_in_list && !is_join_one_only {
            // Check if this is a join(all) field
            let is_join_all = get_join_config(field).is_some_and(|c| c.on_all);

            if is_join_all {
                // Join(all) fields: Initialize with empty vec in From<Model> - they'll be populated by get_all() loading logic
                // The ListModel struct includes them with Vec<APIStruct> type, so we initialize with vec![]
                assignments.push(quote! {
                    #field_name: vec![]
                });
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
