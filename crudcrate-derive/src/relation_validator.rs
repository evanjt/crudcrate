//! Simplified join validation - provides warnings for potentially problematic configurations
//!
//! Philosophy: Trust Sea-ORM to handle relationships correctly. Only warn about performance issues.

use crate::codegen::joins::get_join_config;
use crate::codegen::type_resolution::{extract_option_inner_type_ref, extract_vec_inner_type_ref};
use crate::traits::crudresource::structs::EntityFieldAnalysis;
use quote::quote;

// Maximum allowed join depth (enforced at runtime, warned at compile-time)
const MAX_ALLOWED_DEPTH: u8 = 5;

/// Generate simple validation warnings for join configurations
/// Only warns about potential performance issues, trusts Sea-ORM for correctness
pub fn generate_cyclic_dependency_check(
    analysis: &EntityFieldAnalysis,
    entity_name: &str,
) -> proc_macro2::TokenStream {
    let mut warnings: Vec<proc_macro2::TokenStream> = Vec::new();

    // Check join_on_one fields
    for field in &analysis.join_on_one_fields {
        let result = get_join_config(field);
        if let Some(ref join_config) = result.config {
            check_join_depth(field, join_config, entity_name, &mut warnings);
        }
    }

    // Check join_on_all fields
    for field in &analysis.join_on_all_fields {
        let result = get_join_config(field);
        if let Some(ref join_config) = result.config {
            check_join_depth(field, join_config, entity_name, &mut warnings);
        }
    }

    quote! {
        #( #warnings )*
    }
}

/// Check if join depth is potentially problematic for performance
fn check_join_depth(
    field: &syn::Field,
    join_config: &crate::codegen::joins::JoinConfig,
    entity_name: &str,
    warnings: &mut Vec<proc_macro2::TokenStream>,
) {
    let field_name = field.ident.as_ref().map_or_else(|| "unknown".to_string(), std::string::ToString::to_string);

    // Get the actual depth from join_config (None defaults to MAX_ALLOWED_DEPTH at runtime)
    if let Some(depth) = join_config.depth {
        // Warn if depth exceeds maximum (will be capped at runtime)
        if depth > MAX_ALLOWED_DEPTH {
            let warning_msg = format!(
                "Join field '{field_name}' in '{entity_name}' has depth {depth}, but MAX_JOIN_DEPTH={MAX_ALLOWED_DEPTH}. \
                 Depth will be capped to {MAX_ALLOWED_DEPTH} at runtime. Consider using depth={MAX_ALLOWED_DEPTH} or less."
            );
            warnings.push(quote! {
                compile_error!(#warning_msg);
            });
        }
    } else {
        // None = unlimited, which defaults to MAX_ALLOWED_DEPTH at runtime
        // Extract target type for self-reference check
        if let Ok(target_type) = extract_target_type(&field.ty) {
            // Check if this is a self-referencing join (e.g., Category -> Category)
            if target_type.contains(entity_name) {
                let warning_msg = format!(
                    "Self-referencing join field '{field_name}' in '{entity_name}' has no explicit depth limit. \
                     This will default to depth={MAX_ALLOWED_DEPTH} at runtime. For self-referencing joins, consider using a lower depth: #[crudcrate(join(..., depth = 2))]"
                );
                warnings.push(quote! {
                    compile_error!(#warning_msg);
                });
            }
        }
    }
}

/// Extract target type from field (handles Vec<T> and Option<T>)
fn extract_target_type(field_type: &syn::Type) -> Result<String, ()> {
    let inner = extract_vec_inner_type_ref(field_type);
    let inner = extract_option_inner_type_ref(inner);

    if let syn::Type::Path(path) = inner {
        let segments: Vec<String> = path.path.segments.iter().map(|s| s.ident.to_string()).collect();
        Ok(segments.join("::"))
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_extract_target_type_simple() {
        let ty: syn::Type = parse_quote!(User);
        assert_eq!(extract_target_type(&ty), Ok("User".to_string()));
    }

    #[test]
    fn test_extract_target_type_vec() {
        let ty: syn::Type = parse_quote!(Vec<Vehicle>);
        assert_eq!(extract_target_type(&ty), Ok("Vehicle".to_string()));
    }

    #[test]
    fn test_extract_target_type_option() {
        let ty: syn::Type = parse_quote!(Option<Customer>);
        assert_eq!(extract_target_type(&ty), Ok("Customer".to_string()));
    }

    #[test]
    fn test_extract_target_type_vec_option() {
        let ty: syn::Type = parse_quote!(Vec<Option<Item>>);
        assert_eq!(extract_target_type(&ty), Ok("Item".to_string()));
    }

    #[test]
    fn test_extract_target_type_path() {
        let ty: syn::Type = parse_quote!(super::vehicle::Vehicle);
        assert_eq!(extract_target_type(&ty), Ok("super::vehicle::Vehicle".to_string()));
    }

    #[test]
    fn test_extract_target_type_reference_fails() {
        let ty: syn::Type = parse_quote!(&str);
        assert_eq!(extract_target_type(&ty), Err(()));
    }

    #[test]
    fn test_generate_cyclic_dependency_check_no_joins() {
        let analysis = EntityFieldAnalysis {
            db_fields: vec![],
            non_db_fields: vec![],
            primary_key_field: None,
            sortable_fields: vec![],
            filterable_fields: vec![],
            fulltext_fields: vec![],
            join_on_one_fields: vec![],
            join_on_all_fields: vec![],
            join_filter_sort_configs: vec![],
        };
        let result = generate_cyclic_dependency_check(&analysis, "TestEntity");
        assert!(result.is_empty());
    }

    #[test]
    fn test_max_allowed_depth_constant() {
        assert_eq!(MAX_ALLOWED_DEPTH, 5);
    }
}
