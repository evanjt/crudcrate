//! Merge code generation for Update model implementation
//!
//! Handles generation of:
//! - Included field merge logic (fields in Update model)
//! - Excluded field merge logic (fields with `on_update` but excluded from Update model)

use crate::attribute_parser;
use crate::codegen::models::shared::generate_active_value_assignment;
use crate::fields;
use quote::quote;

/// Generates merge code for fields included in the Update model
/// Handles Option<Option<T>> for optional fields and Option<T> for required fields
pub(crate) fn generate_included_merge_code(
    included_fields: &[&syn::Field],
) -> Vec<proc_macro2::TokenStream> {
    included_fields
        .iter()
        .filter(|field| {
            !attribute_parser::get_crudcrate_bool(field, "non_db_attr").unwrap_or(false)
        })
        .map(|field| {
            let ident = &field.ident;
            let is_optional = fields::field_is_optional(field);

            if is_optional {
                quote! {
                    model.#ident = match self.#ident {
                        Some(Some(value)) => sea_orm::ActiveValue::Set(Some(value.into())),
                        Some(None)      => sea_orm::ActiveValue::Set(None),
                        None            => sea_orm::ActiveValue::NotSet,
                    };
                }
            } else {
                quote! {
                    model.#ident = match self.#ident {
                        Some(Some(value)) => sea_orm::ActiveValue::Set(value.into()),
                        Some(None) => {
                            return Err(crudcrate::ApiError::bad_request(format!(
                                "Field '{}' is required and cannot be set to null",
                                stringify!(#ident)
                            )));
                        },
                        None => sea_orm::ActiveValue::NotSet,
                    };
                }
            }
        })
        .collect()
}

/// Generates merge code for fields excluded from Update model but with `on_update`
/// These fields are automatically updated with the `on_update` expression
pub(crate) fn generate_excluded_merge_code(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| {
            attribute_parser::get_crudcrate_bool(field, "update_model") == Some(false)
                && !attribute_parser::get_crudcrate_bool(field, "non_db_attr").unwrap_or(false)
        })
        .filter_map(|field| {
            attribute_parser::get_crudcrate_expr(field, "on_update").map(|expr| {
                let ident = field.ident.as_ref().unwrap();
                let is_optional = fields::field_is_optional(field);
                generate_active_value_assignment(ident, &expr, is_optional)
            })
        })
        .collect()
}
