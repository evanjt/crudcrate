use crate::attribute_parser::{get_crudcrate_bool, get_crudcrate_expr};
use crate::codegen::models::shared::{
    generate_active_value_set, generate_field_with_optional_default,
    resolve_field_type_with_target_models,
};
use crate::codegen::models::should_include_in_model;
use crate::fields::field_is_optional;
use quote::quote;

/// Generates the conversion lines for a create model to active model conversion
pub(crate) fn generate_create_conversion_lines(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    let mut conv_lines = Vec::new();
    for field in fields {
        if get_crudcrate_bool(field, "non_db_attr").unwrap_or(false) {
            continue;
        }
        let ident = field.ident.as_ref().unwrap();
        let include = get_crudcrate_bool(field, "create_model").unwrap_or(true);
        let is_optional = field_is_optional(field);

        if include {
            if let Some(expr) = get_crudcrate_expr(field, "on_create") {
                if is_optional {
                    conv_lines.push(quote! {
                        #ident: sea_orm::ActiveValue::Set(match create.#ident {
                            Some(Some(inner)) => Some(inner.into()),
                            Some(None)         => None,
                            None               => Some((#expr).into()),
                        })
                    });
                } else {
                    conv_lines.push(quote! {
                        #ident: sea_orm::ActiveValue::Set(match create.#ident {
                            Some(val) => val.into(),
                            None      => (#expr).into(),
                        })
                    });
                }
            } else if is_optional {
                conv_lines.push(quote! {
                    #ident: sea_orm::ActiveValue::Set(create.#ident.map(|v| v.into()))
                });
            } else {
                conv_lines.push(quote! {
                    #ident: sea_orm::ActiveValue::Set(create.#ident.into())
                });
            }
        } else if let Some(expr) = get_crudcrate_expr(field, "on_create") {
            conv_lines.push(generate_active_value_set(ident, &expr, is_optional));
        } else {
            // Field is excluded from Create model and has no on_create - set to NotSet
            // This allows the field to be set manually later in custom create functions
            conv_lines.push(quote! {
                #ident: sea_orm::ActiveValue::NotSet
            });
        }
    }
    conv_lines
}

pub(crate) fn generate_create_struct_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| should_include_in_model(field, "create_model"))
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;
            if get_crudcrate_bool(field, "non_db_attr").unwrap_or(false) {
                // Resolve type with target models (create model)
                let final_ty = resolve_field_type_with_target_models(ty, field, |create, _, _| create.clone());
                generate_field_with_optional_default(ident, final_ty, field)
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
