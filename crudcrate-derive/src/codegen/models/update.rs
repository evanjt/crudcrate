use crate::attribute_parser::get_crudcrate_bool;
use crate::codegen::models::shared::{
    generate_field_with_optional_default, resolve_field_type_with_target_models,
};
use crate::codegen::models::should_include_in_model;
use quote::quote;

/// Generates the field declarations for an update struct
pub(crate) fn generate_update_struct_fields(
    included_fields: &[&syn::Field],
) -> Vec<proc_macro2::TokenStream> {
    included_fields
        .iter()
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;

            if get_crudcrate_bool(field, "non_db_attr").unwrap_or(false) {
                // Resolve type with target models (update model)
                let final_ty = resolve_field_type_with_target_models(ty, field, |_, update, _| update.clone());
                generate_field_with_optional_default(ident.as_ref(), &final_ty, field)
            } else {
                // Extract inner type from Option<T> - inline replacement for extract_inner_type_for_update
                let inner_ty = if let syn::Type::Path(type_path) = ty
                    && let Some(last_seg) = type_path.path.segments.last()
                    && last_seg.ident == "Option"
                    && let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments
                    && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
                {
                    inner.clone()
                } else {
                    ty.clone()
                };
                quote! {
                    #[serde(
                        default,
                        skip_serializing_if = "Option::is_none",
                        with = "crudcrate::serde_with::rust::double_option"
                    )]
                    pub #ident: Option<Option<#inner_ty>>
                }
            }
        })
        .collect()
}

/// Filters fields that should be included in update model
pub(crate) fn filter_update_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<&syn::Field> {
    fields
        .iter()
        .filter(|field| should_include_in_model(field, "update_model"))
        .collect()
}
