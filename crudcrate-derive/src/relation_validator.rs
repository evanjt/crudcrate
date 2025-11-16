//! Simplified join validation - provides warnings for potentially problematic configurations
//!
//! Philosophy: Trust Sea-ORM to handle relationships correctly. Only warn about performance issues.

use crate::codegen::joins::get_join_config;
use crate::codegen::type_resolution::{extract_option_inner_type_ref, extract_vec_inner_type_ref};
use crate::traits::crudresource::structs::EntityFieldAnalysis;
use quote::quote;

// Performance warning threshold
const PERFORMANCE_WARNING_DEPTH: u8 = 5;

/// Generate simple validation warnings for join configurations
/// Only warns about potential performance issues, trusts Sea-ORM for correctness
pub fn generate_cyclic_dependency_check(
    analysis: &EntityFieldAnalysis,
    entity_name: &str,
) -> proc_macro2::TokenStream {
    let mut warnings: Vec<proc_macro2::TokenStream> = Vec::new();

    // Check join_on_one fields
    for field in &analysis.join_on_one_fields {
        if let Some(join_config) = get_join_config(field) {
            check_join_depth(field, &join_config, entity_name, &mut warnings);
        }
    }

    // Check join_on_all fields
    for field in &analysis.join_on_all_fields {
        if let Some(join_config) = get_join_config(field) {
            check_join_depth(field, &join_config, entity_name, &mut warnings);
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
    let field_name = field.ident.as_ref().map(|i| i.to_string()).unwrap_or_else(|| "unknown".to_string());

    // Get the actual depth from join_config
    let depth = join_config.depth.unwrap_or(3);

    // Warn if depth is excessive (performance concern)
    if depth > PERFORMANCE_WARNING_DEPTH {
        let warning_msg = format!(
            "Join field '{field_name}' in '{entity_name}' has recursion depth {} which may impact performance. Consider reducing to {} or less.",
            depth, PERFORMANCE_WARNING_DEPTH
        );
        warnings.push(quote! {
            compile_error!(#warning_msg);
        });
    }

    // Warn about unlimited recursion
    if join_config.depth.is_none() {
        // Extract target type for self-reference check
        if let Ok(target_type) = extract_target_type(&field.ty) {
            // Check if this is a self-referencing join (e.g., Category -> Category)
            if target_type.contains(entity_name) {
                let warning_msg = format!(
                    "Self-referencing join field '{field_name}' in '{entity_name}' has no depth limit. This will use default depth (3) and may cause performance issues. Consider adding explicit depth: join(..., depth = 2)"
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

/// Generate join relation validation (mostly informational comments)
pub fn generate_join_relation_validation(_analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    // Removed complex validation - Sea-ORM handles this at runtime
    // Just return empty token stream
    quote! {}
}
