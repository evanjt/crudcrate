//! Shared utilities for model generation to eliminate code duplication

use crate::attribute_parser::{field_has_crudcrate_flag, get_crudcrate_expr};
use crate::field_analyzer::{resolve_target_models, resolve_target_models_with_list};
use quote::quote;

/// Resolves the final type for a field, handling use_target_models transformations
///
/// # Arguments
/// * `ty` - The original field type
/// * `field` - The field to check for attributes
/// * `model_selector` - Function to select which model type (0=create, 1=update, 2=list)
pub(crate) fn resolve_field_type_with_target_models(
    ty: &syn::Type,
    field: &syn::Field,
    model_selector: impl Fn(&proc_macro2::TokenStream, &proc_macro2::TokenStream, &proc_macro2::TokenStream) -> proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if !field_has_crudcrate_flag(field, "use_target_models") {
        return quote! { #ty };
    }

    // Try to resolve target models
    let target_model = if let Some((create, update)) = resolve_target_models(ty) {
        // For create/update (2 models)
        model_selector(&quote! { #create }, &quote! { #update }, &quote! { #ty })
    } else if let Some((create, update, list)) = resolve_target_models_with_list(ty) {
        // For list (3 models)
        model_selector(&quote! { #create }, &quote! { #update }, &quote! { #list })
    } else {
        return quote! { #ty };
    };

    // Check if original type is Vec<T>
    if is_vec_type(ty) {
        quote! { Vec<#target_model> }
    } else {
        target_model
    }
}

/// Checks if a type is Vec<T>
fn is_vec_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if let Some(last_seg) = type_path.path.segments.last() {
            return last_seg.ident == "Vec";
        }
    }
    false
}

/// Generates a field with optional default serde attribute
pub(crate) fn generate_field_with_optional_default(
    ident: &Option<syn::Ident>,
    ty: proc_macro2::TokenStream,
    field: &syn::Field,
) -> proc_macro2::TokenStream {
    if get_crudcrate_expr(field, "default").is_some() {
        quote! {
            #[serde(default)]
            pub #ident: #ty
        }
    } else {
        quote! {
            pub #ident: #ty
        }
    }
}

/// Generates conversion logic for use_target_models fields in From implementations
/// Returns None if field doesn't use target models
pub(crate) fn generate_target_model_conversion(
    field: &syn::Field,
    ident: &Option<syn::Ident>,
) -> Option<proc_macro2::TokenStream> {
    if !field_has_crudcrate_flag(field, "use_target_models") {
        return None;
    }

    let ty = &field.ty;

    // Check if we can resolve target models
    let has_targets = resolve_target_models(ty).is_some()
        || resolve_target_models_with_list(ty).is_some();

    if !has_targets {
        return None;
    }

    // For Vec<T>, convert each item
    if is_vec_type(ty) {
        Some(quote! {
            #ident: model.#ident.into_iter().map(Into::into).collect()
        })
    } else {
        // For single item, use direct conversion
        Some(quote! {
            #ident: model.#ident.into()
        })
    }
}
