//! `ToCreateModel`, `ToUpdateModel` and `ToListModel` expansion.

use quote::{format_ident, quote};
use syn::DeriveInput;

use crate::expand::screening::screen_tokens;
use crate::{attrs, codegen, fields};

/// `#[serde(deny_unknown_fields)]` for the generated input models when the struct
/// opts in with `#[crudcrate(deny_unknown_fields)]`, otherwise nothing.
pub(crate) fn strict_payload_attr(attrs: &[syn::Attribute]) -> proc_macro2::TokenStream {
    if attrs::parse_crud_resource_meta(attrs).deny_unknown_fields {
        quote! { #[serde(deny_unknown_fields)] }
    } else {
        quote! {}
    }
}

pub(crate) fn extract_active_model_type(
    input: &DeriveInput,
    name: &syn::Ident,
) -> Result<proc_macro2::TokenStream, proc_macro2::TokenStream> {
    for attr in &input.attrs {
        if attr.path().is_ident("active_model")
            && let Some(s) = attrs::get_string_from_attr(attr)
        {
            return match syn::parse_str::<syn::Type>(&s) {
                Ok(ty) => Ok(quote! { #ty }),
                Err(_) => Err(syn::Error::new_spanned(
                    attr,
                    format!("Invalid active_model type: '{s}'. Expected a valid Rust type path."),
                )
                .to_compile_error()),
            };
        }
    }
    let ident = format_ident!("{}ActiveModel", name);
    Ok(quote! { #ident })
}

pub(crate) fn to_create_model_impl(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let input = match syn::parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(e) => return e.to_compile_error(),
    };
    if let Some(tokens) = screen_tokens(&input) {
        return tokens;
    }
    let name = &input.ident;
    let create_name = format_ident!("{}Create", name);

    let active_model_type = match extract_active_model_type(&input, name) {
        Ok(ty) => ty,
        Err(e) => return e,
    };
    let fields = match fields::extract_named_fields(&input) {
        Ok(f) => f,
        Err(e) => return e,
    };
    let create_struct_fields = codegen::models::create::generate_create_struct_fields(&fields);
    let conv_lines = codegen::models::create::generate_create_conversion_lines(&fields);
    let strict_payload = strict_payload_attr(&input.attrs);

    // Always include ToSchema for Create models
    // Circular dependencies are handled by schema(no_recursion) on join fields in the main model
    let create_derives =
        quote! { Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let expanded = quote! {
        #[derive(#create_derives)]
        #strict_payload
        pub struct #create_name {
            #(#create_struct_fields),*
        }

        impl From<#create_name> for #active_model_type {
            fn from(create: #create_name) -> Self {
                #active_model_type {
                    #(#conv_lines),*
                }
            }
        }
    };

    expanded
}

pub(crate) fn to_update_model_impl(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let input = match syn::parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(e) => return e.to_compile_error(),
    };
    if let Some(tokens) = screen_tokens(&input) {
        return tokens;
    }
    let name = &input.ident;
    let update_name = format_ident!("{}Update", name);

    let active_model_type = match extract_active_model_type(&input, name) {
        Ok(ty) => ty,
        Err(e) => return e,
    };
    let fields = match fields::extract_named_fields(&input) {
        Ok(f) => f,
        Err(e) => return e,
    };
    let included_fields = crate::codegen::models::update::filter_update_fields(&fields);
    let update_struct_fields =
        crate::codegen::models::update::generate_update_struct_fields(&included_fields);
    let included_merge = codegen::models::merge::generate_included_merge_code(&included_fields);
    let excluded_merge = codegen::models::merge::generate_excluded_merge_code(&fields);
    let strict_payload = strict_payload_attr(&input.attrs);

    // Always include ToSchema for Update models
    // Circular dependencies are handled by schema(no_recursion) on join fields in the main model
    let update_derives =
        quote! { Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let expanded = quote! {
        #[derive(#update_derives)]
        #strict_payload
        pub struct #update_name {
            #(#update_struct_fields),*
        }

        impl #update_name {
            pub fn merge_fields(self, mut model: #active_model_type) -> Result<#active_model_type, crudcrate::ApiError> {
                #(#included_merge)*
                #(#excluded_merge)*
                Ok(model)
            }
        }

        impl crudcrate::traits::MergeIntoActiveModel<#active_model_type> for #update_name {
            fn merge_into_activemodel(self, model: #active_model_type) -> Result<#active_model_type, crudcrate::ApiError> {
                Self::merge_fields(self, model)
            }
        }
    };

    expanded
}

pub(crate) fn to_list_model_impl(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let input = match syn::parse2::<DeriveInput>(input) {
        Ok(input) => input,
        Err(e) => return e.to_compile_error(),
    };
    if let Some(tokens) = screen_tokens(&input) {
        return tokens;
    }
    let name = &input.ident;
    let list_name = format_ident!("{}List", name);

    let fields = match fields::extract_named_fields(&input) {
        Ok(f) => f,
        Err(e) => return e,
    };
    let list_struct_fields =
        crate::codegen::models::list::generate_list_struct_fields(&fields, name);
    let list_from_assignments =
        crate::codegen::models::list::generate_list_from_assignments(&fields);

    // Always include ToSchema for List models
    // Circular dependencies are handled by schema(no_recursion) on join fields in the main model
    let list_derives = quote! { Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema };

    let expanded = quote! {
        #[derive(#list_derives)]
        pub struct #list_name {
            #(#list_struct_fields),*
        }

        impl From<#name> for #list_name {
            fn from(model: #name) -> Self {
                Self {
                    #(#list_from_assignments),*
                }
            }
        }
    };

    expanded
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_strict_payload_attr_follows_opt_in() {
        let opted: DeriveInput = parse_quote! {
            #[crudcrate(deny_unknown_fields)]
            pub struct Model {
                pub id: i32,
            }
        };
        assert!(
            strict_payload_attr(&opted.attrs)
                .to_string()
                .contains("deny_unknown_fields")
        );

        let default: DeriveInput = parse_quote! {
            pub struct Model {
                pub id: i32,
            }
        };
        assert!(strict_payload_attr(&default.attrs).is_empty());
    }
}
