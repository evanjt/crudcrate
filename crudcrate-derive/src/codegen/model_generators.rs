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

pub(crate) fn generate_create_struct_fields(
    fields: &Punctuated<syn::Field, Comma>,
) -> Vec<TokenStream> {
    fields
        .iter()
        .filter(|field| {
            // Exclude fields from create model if create_model = false
            let include_in_create = get_crudcrate_bool(field, "create_model").unwrap_or(true);

            // Exclude join fields entirely from Create models - they're populated by recursive loading
            let is_join_field = get_join_config(field).is_some();

            include_in_create && !is_join_field
        })
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            if get_crudcrate_bool(field, "non_db_attr").unwrap_or(false) {
                // Check if this field uses target models
                let has_use_target_models = field_has_crudcrate_flag(field, "use_target_models");
                let final_ty = if has_use_target_models {
                    if let Some((create_model, _)) = resolve_target_models(ty) {
                        // Replace the type with the target's Create model
                        if let syn::Type::Path(type_path) = ty {
                            if let Some(last_seg) = type_path.path.segments.last() {
                                if last_seg.ident == "Vec" {
                                    // Vec<Treatment> -> Vec<TreatmentCreate>
                                    quote! { Vec<#create_model> }
                                } else {
                                    // Treatment -> TreatmentCreate
                                    quote! { #create_model }
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
                } else {
                    quote! { #ty }
                };
                if get_crudcrate_expr(field, "default").is_some() {
                    quote! {
                        #[serde(default)]
                        pub #ident: #final_ty
                    }
                } else {
                    quote! {
                        pub #ident: #final_ty
                    }
                }
            } else if get_crudcrate_expr(field, "on_create").is_some() {
                quote! {
                    #[serde(default)]
                    pub #ident: Option<#ty>
                }
            } else {
                quote! {
                    pub #ident: #ty
                }
            }
        })
        .collect()
}

pub(crate) fn generate_create_conversion_lines(
    fields: &Punctuated<syn::Field, Comma>,
    entity_name: &str,
) -> Vec<TokenStream> {
    fields
        .iter()
        .filter(|field| {
            let include_in_create = get_crudcrate_bool(field, "create_model").unwrap_or(true);
            let is_join_field = get_join_config(field).is_some();
            include_in_create && !is_join_field
        })
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;

            if get_crudcrate_bool(field, "non_db_attr").unwrap_or(false) {
                // For non-db fields with use_target_models, handle the conversion
                let has_use_target_models = field_has_crudcrate_flag(field, "use_target_models");
                if has_use_target_models {
                    if let Some((create_model, _)) = resolve_target_models(ty) {
                        if let syn::Type::Path(type_path) = ty {
                            if let Some(last_seg) = type_path.path.segments.last() {
                                if last_seg.ident == "Vec" {
                                    // Handle Vec<TargetCreate> -> Vec<ActiveModel>
                                    quote! {
                                        #ident: sea_orm::ActiveValue::Set(
                                            create.#ident.into_iter()
                                                .map(|item| item.into())
                                                .collect::<Vec<_>>()
                                        ),
                                    }
                                } else {
                                    // Handle TargetCreate -> ActiveModel
                                    quote! {
                                        #ident: sea_orm::ActiveValue::Set(create.#ident.into()),
                                    }
                                }
                            } else {
                                quote! {
                                    #ident: sea_orm::ActiveValue::Set(create.#ident),
                                }
                            }
                        } else {
                            quote! {
                                #ident: sea_orm::ActiveValue::Set(create.#ident),
                            }
                        }
                    } else {
                        quote! {
                            #ident: sea_orm::ActiveValue::Set(create.#ident),
                        }
                    }
                } else if get_crudcrate_expr(field, "default").is_some() {
                    quote! {
                        #ident: sea_orm::ActiveValue::Set(create.#ident),
                    }
                } else {
                    quote! {
                        #ident: sea_orm::ActiveValue::Set(create.#ident),
                    }
                }
            } else if let Some(expr) = get_crudcrate_expr(field, "on_create") {
                quote! {
                    #ident: sea_orm::ActiveValue::Set(#expr),
                }
            } else {
                let entity_ident = format_ident!("{}", entity_name);
                let column_ident = format_ident!("{}Column", entity_name);
                quote! {
                    #ident: sea_orm::ActiveValue::Set(create.#ident),
                }
            }
        })
        .collect()
}

pub(crate) fn generate_update_struct_fields(
    fields: &Punctuated<syn::Field, Comma>,
) -> Vec<&syn::Field> {
    fields
        .iter()
        .filter(|field| {
            let include_in_update = get_crudcrate_bool(field, "update_model").unwrap_or(true);

            // Exclude join fields entirely from Update models - they're populated by recursive loading
            let is_join_field = get_join_config(field).is_some();

            include_in_update && !is_join_field
        })
        .collect()
}