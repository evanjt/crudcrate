use crate::attribute_parser::get_crudcrate_bool;
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

            let final_ty = quote! { #ty };

            quote! {
                pub #ident: #final_ty
            }
        })
        .collect()
}
