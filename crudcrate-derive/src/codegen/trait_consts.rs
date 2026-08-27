//! Emitters for the `CRUDResource` associated consts and type aliases.

use quote::{format_ident, quote};

use crate::attrs::get_crudcrate_bool;
use crate::codegen::models::should_include_in_model;
use crate::ir::CRUDResourceMeta;
use crate::syn_type::{column_ident, extract_option_inner_type_ref, ident_to_string, is_text_type};

pub(crate) fn generate_crud_type_aliases(
    api_struct_name: &syn::Ident,
    _crud_meta: &CRUDResourceMeta,
    active_model_path: &str,
) -> (
    syn::Ident,
    syn::Ident,
    syn::Ident,
    syn::Type,
    syn::Type,
    syn::Type,
) {
    let create_model_name = format_ident!("{}Create", api_struct_name);
    let update_model_name = format_ident!("{}Update", api_struct_name);
    let list_model_name = format_ident!("{}List", api_struct_name);

    // Sea-ORM always uses Entity and Column - these are not configurable
    let entity_type: syn::Type = syn::parse_quote!(Entity);
    let column_type: syn::Type = syn::parse_quote!(Column);

    let active_model_type: syn::Type =
        syn::parse_str(active_model_path).unwrap_or_else(|_| syn::parse_quote!(ActiveModel));

    (
        create_model_name,
        update_model_name,
        list_model_name,
        entity_type,
        column_type,
        active_model_type,
    )
}

pub(crate) fn generate_id_column(
    primary_key_field: Option<&syn::Field>,
) -> proc_macro2::TokenStream {
    if let Some(pk_field) = primary_key_field {
        let field_name = &pk_field.ident.as_ref().unwrap();
        let column_name = column_ident(&ident_to_string(field_name));
        quote! { Self::ColumnType::#column_name }
    } else {
        quote! { Self::ColumnType::Id }
    }
}

pub(crate) fn generate_field_entries(fields: &[&syn::Field]) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_str = ident_to_string(field_name);
            let column_name = column_ident(&field_str);
            quote! { (#field_str, Self::ColumnType::#column_name) }
        })
        .collect()
}

pub(crate) fn generate_like_filterable_entries(
    fields: &[&syn::Field],
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter_map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_str = ident_to_string(field_name);

            // Check if this field should use LIKE queries based on its type
            if is_text_type(&field.ty) {
                Some(quote! { #field_str })
            } else {
                None
            }
        })
        .collect()
}

/// Generate string entries for columns excluded from scoped (public) requests.
///
/// Collects field names that have `exclude(scoped)`; these are stripped from
/// filterable/sortable lists when a `ScopeCondition` is active, preventing
/// schema probing by unauthenticated users.
pub(crate) fn generate_scoped_excluded_entries(
    fields: &[&syn::Field],
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| !should_include_in_model(field, "scoped_model"))
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_str = ident_to_string(field_name);
            quote! { #field_str }
        })
        .collect()
}

/// Generate enum field checker using compile-time trait detection.
/// Automatically detects fields whose type implements `sea_orm::ActiveEnum`
/// using the inherent impl trick; no explicit annotation needed.
/// The `#[crudcrate(enum_field)]` attribute is still supported as an explicit override.
pub(crate) fn generate_enum_field_checker(all_fields: &[&syn::Field]) -> proc_macro2::TokenStream {
    let field_checks: Vec<proc_macro2::TokenStream> = all_fields
        .iter()
        .filter_map(|field| {
            if let Some(field_name) = &field.ident {
                let field_name_str = ident_to_string(field_name);

                // Backward compat: explicit enum_field still works but is no longer required.
                // Deprecated in 0.7.2: enum fields are now auto-detected.
                let explicit = get_crudcrate_bool(field, "enum_field").unwrap_or(false);
                if explicit {
                    return Some(quote! { #field_name_str => true, });
                }

                // Auto-detect: unwrap Option<T> to get the inner type, then check
                // at compile time whether it implements sea_orm::ActiveEnum.
                // Uses the "inherent impl trick": inherent methods on a generic wrapper
                // shadow trait methods, so if T: ActiveEnum the inherent const wins.
                let inner_ty = extract_option_inner_type_ref(&field.ty);

                Some(quote! {
                    #field_name_str => {
                        trait __Fallback { const V: bool = false; }
                        impl<T> __Fallback for __Probe<T> {}
                        struct __Probe<T>(::core::marker::PhantomData<T>);
                        #[allow(dead_code)]
                        impl<T: ::sea_orm::ActiveEnum> __Probe<T> {
                            const V: bool = true;
                        }
                        <__Probe<#inner_ty>>::V
                    },
                })
            } else {
                None
            }
        })
        .collect();

    quote! {
        match field_name {
            #(#field_checks)*
            _ => false,
        }
    }
}
