//! Field extraction utilities
//!
//! Functions for extracting and parsing fields from struct definitions,
//! including entity-level attribute parsing.

use crate::attribute_parser;
use heck::ToPascalCase;
use proc_macro::TokenStream;
use quote::format_ident;
use syn::{Data, DeriveInput, Fields, Lit, Meta, parse::Parser, punctuated::Punctuated, token::Comma};

/// Extract named fields from a struct, returning proper compile error if not valid
pub fn extract_named_fields(
    input: &DeriveInput,
) -> Result<syn::punctuated::Punctuated<syn::Field, syn::token::Comma>, TokenStream> {
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => Ok(named.named.clone()),
            _ => Err(syn::Error::new_spanned(
                input,
                "This derive macro only supports structs with named fields",
            )
            .to_compile_error()
            .into()),
        },
        _ => Err(syn::Error::new_spanned(
            input,
            "This derive macro only supports structs"
        )
        .to_compile_error()
        .into()),
    }
}

/// Extract entity fields with proper error handling
pub fn extract_entity_fields(
    input: &DeriveInput,
) -> Result<&syn::punctuated::Punctuated<syn::Field, syn::token::Comma>, TokenStream> {
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => Ok(&fields.named),
            _ => Err(syn::Error::new_spanned(
                input,
                "EntityToModels only supports structs with named fields",
            )
            .to_compile_error()
            .into()),
        },
        _ => Err(
            syn::Error::new_spanned(input, "EntityToModels only supports structs")
                .to_compile_error()
                .into(),
        ),
    }
}

/// Parse entity-level attributes (`api_struct`, `active_model`)
pub fn parse_entity_attributes(input: &DeriveInput, struct_name: &syn::Ident) -> (syn::Ident, String) {
    let mut api_struct_name = None;
    let mut active_model_path = None;

    for attr in &input.attrs {
        if attr.path().is_ident("crudcrate")
            && let Meta::List(meta_list) = &attr.meta
            && let Ok(metas) =
                Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
        {
            for meta in &metas {
                if let Meta::NameValue(nv) = meta {
                    if nv.path.is_ident("api_struct") {
                        if let syn::Expr::Lit(expr_lit) = &nv.value
                            && let Lit::Str(s) = &expr_lit.lit
                        {
                            api_struct_name = Some(format_ident!("{}", s.value()));
                        }
                    } else if nv.path.is_ident("active_model")
                        && let syn::Expr::Lit(expr_lit) = &nv.value
                        && let Lit::Str(s) = &expr_lit.lit
                    {
                        active_model_path = Some(s.value());
                    }
                }
            }
        }
    }

    let table_name = attribute_parser::extract_table_name(&input.attrs)
        .unwrap_or_else(|| struct_name.to_string());
    let api_struct_name =
        api_struct_name.unwrap_or_else(|| format_ident!("{}", table_name.to_pascal_case()));
    let active_model_path = active_model_path.unwrap_or_else(|| "ActiveModel".to_string());

    (api_struct_name, active_model_path)
}

/// Check if a field has the #[`sea_orm(ignore)`] attribute
pub fn has_sea_orm_ignore(field: &syn::Field) -> bool {
    for attr in &field.attrs {
        if attr.path().is_ident("sea_orm")
            && let Meta::List(meta_list) = &attr.meta
            && let Ok(metas) =
                Punctuated::<Meta, Comma>::parse_terminated.parse2(meta_list.tokens.clone())
        {
            for meta in metas {
                if let Meta::Path(path) = meta
                    && path.is_ident("ignore")
                {
                    return true;
                }
            }
        }
    }
    false
}
