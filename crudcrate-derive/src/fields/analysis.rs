//! Field analysis and validation
//!
//! Functions for analyzing fields, categorizing them by attributes,
//! and validating field configurations.

use crate::attribute_parser;
use crate::codegen::joins::get_join_config;
use crate::fields::extraction::has_sea_orm_ignore;
use crate::traits::crudresource::structs::{EntityFieldAnalysis, JoinFilterSortConfig};
use proc_macro::TokenStream;

/// Analyze entity fields and categorize them by attributes
pub fn analyze_entity_fields(
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
        join_filter_sort_configs: Vec::new(),
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

            // Extract join filter/sort configuration if present
            if !join_config.filterable_columns.is_empty() || !join_config.sortable_columns.is_empty() {
                let field_name = field
                    .ident
                    .as_ref()
                    .map_or_else(|| "unknown".to_string(), std::string::ToString::to_string);

                analysis.join_filter_sort_configs.push(JoinFilterSortConfig {
                    field_name,
                    entity_path: join_config.path.clone(),
                    filterable_columns: join_config.filterable_columns.clone(),
                    sortable_columns: join_config.sortable_columns.clone(),
                });
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
pub fn validate_field_analysis(analysis: &EntityFieldAnalysis) -> Result<(), TokenStream> {
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
