use crate::attribute_parser::get_crudcrate_bool;
use crate::codegen::join_strategies::get_join_config;
use quote::quote;

pub(crate) fn generate_response_from_assignments(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| get_crudcrate_bool(field, "one_model").unwrap_or(true))
        .map(|field| {
            let ident = &field.ident;
            quote! {
                #ident: model.#ident
            }
        })
        .collect()
}
pub(crate) fn generate_response_struct_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .filter(|field| get_crudcrate_bool(field, "one_model").unwrap_or(true))
        .map(|field| {
            let ident = &field.ident;
            let ty = &field.ty;

            // Similar logic to List model for join fields
            let final_ty = if get_join_config(field).is_some() {
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
