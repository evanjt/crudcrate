use crate::attribute_parser::{
    field_has_crudcrate_flag, get_crudcrate_bool, get_crudcrate_expr, get_join_config,
};
use crate::field_analyzer::{field_is_optional, resolve_target_models_with_list};
use crate::structs::EntityFieldAnalysis;
use quote::{ToTokens, quote};

pub(crate) fn generate_list_struct_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| {
            let include_in_list = get_crudcrate_bool(field, "list_model").unwrap_or(true);
            // Only exclude join(one) fields from List models - keep join(all) fields since they're meant for list responses
            let is_join_one_only = if let Some(join_config) = get_join_config(field) {
                !join_config.on_all // Exclude if NOT loading in get_all (on_all = false)
            } else {
                false
            };
            include_in_list && !is_join_one_only
        })
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;

            // Check if this field uses target models
            let final_ty = if field_has_crudcrate_flag(field, "use_target_models") {
                if let Some((_, _, list_model)) = resolve_target_models_with_list(ty) {
                    // Replace the type with the target's List model
                    if let syn::Type::Path(type_path) = ty {
                        if let Some(last_seg) = type_path.path.segments.last() {
                            if last_seg.ident == "Vec" {
                                // Vec<Treatment> -> Vec<TreatmentList>
                                quote! { Vec<#list_model> }
                            } else {
                                // Treatment -> TreatmentList
                                quote! { #list_model }
                            }
                        } else {
                            quote! { #ty }
                        }
                    } else {
                        quote! { #ty }
                    }
                } else {
                    quote! { #ty }
                }
            } else if get_join_config(field).is_some() {
                // This ensures List models have proper API struct types, not DB Model types
                crate::resolve_join_field_type_preserving_container(ty)
            } else {
                quote! { #ty }
            };

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
        .filter(|field| {
            let include_in_list = get_crudcrate_bool(field, "list_model").unwrap_or(true);
            // Only exclude join(one) fields from List models - keep join(all) fields since they're meant for list responses
            let is_join_one_only = if let Some(join_config) = get_join_config(field) {
                !join_config.on_all // Exclude if NOT loading in get_all (on_all = false)
            } else {
                false
            };
            include_in_list && !is_join_one_only
        })
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;

            // Check if this field uses target models
            if field_has_crudcrate_flag(field, "use_target_models") {
                if let Some((_, _, _)) = resolve_target_models_with_list(ty) {
                    // For Vec<T>, convert each item using From trait
                    if let syn::Type::Path(type_path) = ty
                        && let Some(last_seg) = type_path.path.segments.last()
                        && last_seg.ident == "Vec"
                    {
                        return quote! {
                            #ident: model.#ident.into_iter().map(Into::into).collect()
                        };
                    }
                    // For single item, use direct conversion
                    quote! {
                        #ident: model.#ident.into()
                    }
                } else {
                    quote! {
                        #ident: model.#ident
                    }
                }
            } else {
                quote! {
                    #ident: model.#ident
                }
            }
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
            if field_has_crudcrate_flag(field, "use_target_models") {
                let field_type = &field.ty;
                if let Some((_, _, list_type)) = resolve_target_models_with_list(field_type) {
                    // For Vec<T>, convert each item using From trait to ListModel
                    if let syn::Type::Path(type_path) = field_type
                        && let Some(last_seg) = type_path.path.segments.last()
                        && last_seg.ident == "Vec"
                    {
                        assignments.push(quote! {
                                    #field_name: model.#field_name.into_iter().map(|item| #list_type::from(item)).collect()
                                });
                        continue;
                    }
                    // For single item, use direct conversion to ListModel
                    assignments.push(quote! {
                        #field_name: #list_type::from(model.#field_name)
                    });
                    continue;
                }
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
