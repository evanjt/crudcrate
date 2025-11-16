//! Field analysis utilities for entity parsing and validation
//!
//! This module contains functions for:
//! - Extracting and parsing entity fields
//! - Analyzing field attributes and configurations
//! - Validating field relationships and join configurations

use crate::attribute_parser;
use crate::codegen::join_strategies::get_join_config;
use crate::traits::crudresource::structs::EntityFieldAnalysis;
use heck::ToPascalCase;
use proc_macro::TokenStream;
use quote::format_ident;
use syn::{Data, DeriveInput, Fields, Lit, Meta, parse::Parser, punctuated::Punctuated, token::Comma};

/// Extract named fields from a struct, panicking if not a struct with named fields
pub(crate) fn extract_named_fields(
    input: &DeriveInput,
) -> syn::punctuated::Punctuated<syn::Field, syn::token::Comma> {
    if let Data::Struct(data) = &input.data {
        if let Fields::Named(named) = &data.fields {
            named.named.clone()
        } else {
            panic!("ToCreateModel only supports structs with named fields");
        }
    } else {
        panic!("ToCreateModel can only be derived for structs");
    }
}

/// Extract entity fields with proper error handling
pub(crate) fn extract_entity_fields(
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

/// Parse entity-level attributes (api_struct, active_model)
pub(crate) fn parse_entity_attributes(input: &DeriveInput, struct_name: &syn::Ident) -> (syn::Ident, String) {
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

/// Check if a field has the #[sea_orm(ignore)] attribute
pub(crate) fn has_sea_orm_ignore(field: &syn::Field) -> bool {
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

/// Analyze entity fields and categorize them by attributes
pub(crate) fn analyze_entity_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> EntityFieldAnalysis<'_> {
    let mut analysis = EntityFieldAnalysis {
        db_fields: Vec::new(),
        non_db_fields: Vec::new(),
        primary_key_field: None,
        sortable_fields: Vec::new(),
        filterable_fields: Vec::new(),
        fulltext_fields: Vec::new(),
        join_on_one_fields: Vec::new(),
        join_on_all_fields: Vec::new(),
    };

    for field in fields {
        let is_non_db = attribute_parser::get_crudcrate_bool(field, "non_db_attr").unwrap_or(false);

        // Check for join attributes regardless of db/non_db status
        if let Some(join_config) = get_join_config(field) {
            if join_config.on_one {
                analysis.join_on_one_fields.push(field);
            }
            if join_config.on_all {
                analysis.join_on_all_fields.push(field);
            }
        }

        if is_non_db {
            analysis.non_db_fields.push(field);
        } else {
            analysis.db_fields.push(field);

            if attribute_parser::field_has_crudcrate_flag(field, "primary_key") {
                analysis.primary_key_field = Some(field);
            }
            if attribute_parser::field_has_crudcrate_flag(field, "sortable") {
                analysis.sortable_fields.push(field);
            }
            if attribute_parser::field_has_crudcrate_flag(field, "filterable") {
                analysis.filterable_fields.push(field);
            }
            if attribute_parser::field_has_crudcrate_flag(field, "fulltext") {
                analysis.fulltext_fields.push(field);
            }
        }
    }

    analysis
}

/// Validate field analysis for consistency
pub(crate) fn validate_field_analysis(analysis: &EntityFieldAnalysis) -> Result<(), TokenStream> {
    // Check for multiple primary keys
    if analysis.primary_key_field.is_some()
        && analysis
            .db_fields
            .iter()
            .filter(|field| attribute_parser::field_has_crudcrate_flag(field, "primary_key"))
            .count()
            > 1
    {
        return Err(syn::Error::new_spanned(
            analysis.primary_key_field.unwrap(),
            "Only one field can be marked with 'primary_key' attribute",
        )
        .to_compile_error()
        .into());
    }

    // Validate that non_db_attr fields have #[sea_orm(ignore)]
    for field in &analysis.non_db_fields {
        if !has_sea_orm_ignore(field) {
            let field_name = field
                .ident
                .as_ref()
                .map_or_else(|| "unknown".to_string(), std::string::ToString::to_string);
            return Err(syn::Error::new_spanned(
                field,
                format!(
                    "Field '{field_name}' has #[crudcrate(non_db_attr)] but is missing #[sea_orm(ignore)].\n\
                     Non-database fields must be marked with both attributes.\n\
                     Add #[sea_orm(ignore)] above the #[crudcrate(...)] attribute."
                ),
            )
            .to_compile_error()
            .into());
        }
    }

    Ok(())
}
