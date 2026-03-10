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
///
/// Returns an error if deprecated syntax (like `join_filterable`/`join_sortable`) is used.
pub fn analyze_entity_fields(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> Result<EntityFieldAnalysis<'_>, TokenStream> {
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

    let mut deprecation_errors: Vec<syn::Error> = Vec::new();

    for field in fields {
        let is_non_db = attribute_parser::get_crudcrate_bool(field, "non_db_attr").unwrap_or(false);

        // Check for join attributes regardless of db/non_db status
        let join_result = get_join_config(field);

        // Collect any deprecation errors (e.g., from old join_filterable/join_sortable syntax)
        deprecation_errors.extend(join_result.errors);

        if let Some(join_config) = join_result.config {
            if join_config.on_one {
                analysis.join_on_one_fields.push(field);
            }
            if join_config.on_all {
                analysis.join_on_all_fields.push(field);
            }

            // Extract join filter/sort configuration if present
            if !join_config.filterable_columns.is_empty()
                || !join_config.sortable_columns.is_empty()
            {
                let field_name = field
                    .ident
                    .as_ref()
                    .map_or_else(|| "unknown".to_string(), std::string::ToString::to_string);

                analysis
                    .join_filter_sort_configs
                    .push(JoinFilterSortConfig {
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

    // If there are deprecation errors, return them immediately
    if !deprecation_errors.is_empty() {
        // Combine all errors into one
        let mut combined = deprecation_errors.remove(0);
        for err in deprecation_errors {
            combined.combine(err);
        }
        return Err(combined.to_compile_error().into());
    }

    Ok(analysis)
}

/// Validate aggregate config against actual entity fields.
///
/// Checks that:
/// - `time_column` exists and is a DateTime type
/// - Each metric exists and is a numeric type
/// - Each group_by column exists
pub fn validate_aggregate_config(
    agg_config: &crate::traits::crudresource::structs::AggregateConfig,
    field_analysis: &EntityFieldAnalysis,
) -> Result<(), TokenStream> {
    use proc_macro2::Span;
    use quote::quote_spanned;

    // Build field name → type lookup from db_fields
    let field_map: std::collections::HashMap<String, &syn::Type> = field_analysis
        .db_fields
        .iter()
        .filter_map(|f| {
            let name = f.ident.as_ref()?.to_string();
            Some((name, &f.ty))
        })
        .collect();

    let available_fields: Vec<&str> = field_map.keys().map(String::as_str).collect();

    // Validate time_column exists and is DateTime
    let span = agg_config
        .time_column_span
        .unwrap_or_else(Span::call_site);
    if let Some(ty) = field_map.get(&agg_config.time_column) {
        if !is_datetime_type(ty) {
            let type_str = quote::quote!(#ty).to_string();
            return Err(quote_spanned! { span =>
                compile_error!(concat!(
                    "aggregate time_column '", #type_str, "' has type ", #type_str,
                    " — expected DateTime"
                ));
            }
            .into());
        }
    } else {
        let col = &agg_config.time_column;
        let available = available_fields.join(", ");
        let msg = format!(
            "aggregate time_column '{col}' not found. Available fields: {available}"
        );
        return Err(quote_spanned! { span =>
            compile_error!(#msg);
        }
        .into());
    }

    // Validate metrics exist and are numeric
    for (i, metric) in agg_config.metrics.iter().enumerate() {
        let span = agg_config
            .metrics_spans
            .get(i)
            .copied()
            .unwrap_or_else(Span::call_site);
        if let Some(ty) = field_map.get(metric.as_str()) {
            if !is_numeric_type(ty) {
                let type_str = quote::quote!(#ty).to_string();
                let msg = format!(
                    "aggregate metric '{metric}' has type {type_str} — must be numeric (f32, f64, i8-i64, u8-u64, Decimal)"
                );
                return Err(quote_spanned! { span =>
                    compile_error!(#msg);
                }
                .into());
            }
        } else {
            let available = available_fields.join(", ");
            let msg = format!(
                "aggregate metric '{metric}' not found. Available fields: {available}"
            );
            return Err(quote_spanned! { span =>
                compile_error!(#msg);
            }
            .into());
        }
    }

    // Validate group_by columns exist
    for (i, col) in agg_config.group_by.iter().enumerate() {
        let span = agg_config
            .group_by_spans
            .get(i)
            .copied()
            .unwrap_or_else(Span::call_site);
        if !field_map.contains_key(col.as_str()) {
            let available = available_fields.join(", ");
            let msg = format!(
                "aggregate group_by column '{col}' not found. Available fields: {available}"
            );
            return Err(quote_spanned! { span =>
                compile_error!(#msg);
            }
            .into());
        }
    }

    // Validate continuous aggregate config
    for (i, (interval, view_name)) in agg_config.continuous_aggregates.iter().enumerate() {
        let span = agg_config
            .continuous_aggregate_spans
            .get(i)
            .copied()
            .unwrap_or_else(Span::call_site);

        // Check interval exists in allowed list
        if !agg_config.intervals.iter().any(|a| a == interval) {
            let allowed = agg_config.intervals.join(", ");
            let msg = format!(
                "continuous_aggregates: interval '{interval}' not in allowed intervals [{allowed}]"
            );
            return Err(quote_spanned! { span =>
                compile_error!(#msg);
            }
            .into());
        }

        // Check view name is a valid SQL identifier
        if view_name.is_empty()
            || !view_name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            let msg = format!(
                "continuous_aggregates: view name '{view_name}' is not a valid SQL identifier (use only a-zA-Z0-9_)"
            );
            return Err(quote_spanned! { span =>
                compile_error!(#msg);
            }
            .into());
        }
    }

    Ok(())
}

/// Check if a type is a DateTime type (last segment contains "DateTime" or is "NaiveDateTime")
fn is_datetime_type(ty: &syn::Type) -> bool {
    let type_str = last_type_segment(ty);
    type_str.contains("DateTime") || type_str == "NaiveDateTime"
}

/// Check if a type is a numeric type
fn is_numeric_type(ty: &syn::Type) -> bool {
    let type_str = last_type_segment(ty);
    matches!(
        type_str.as_str(),
        "f32" | "f64" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "Decimal"
    )
}

/// Extract the last path segment from a type, unwrapping Option<T>
fn last_type_segment(ty: &syn::Type) -> String {
    match ty {
        syn::Type::Path(type_path) => {
            let last_seg = type_path.path.segments.last();
            if let Some(seg) = last_seg {
                let ident = seg.ident.to_string();
                // Unwrap Option<T> to check inner type
                if ident == "Option" {
                    if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            return last_type_segment(inner_ty);
                        }
                    }
                }
                ident
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
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
