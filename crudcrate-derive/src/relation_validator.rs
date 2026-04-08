//! Join validation - compile-time checks for problematic join configurations.
//!
//! Detects:
//! - Self-referencing joins without explicit depth (would default to MAX_DEPTH)
//! - Depth exceeding MAX_ALLOWED_DEPTH
//! - Bidirectional SeaORM relations that cause infinite recursion in `find_related()`

use crate::codegen::joins::get_join_config;
use crate::codegen::type_resolution::{
    extract_api_struct_type_for_recursive_call, get_path_from_field_type,
};
use crate::traits::crudresource::structs::EntityFieldAnalysis;
use quote::quote;

// Maximum allowed join depth (enforced at runtime, warned at compile-time)
const MAX_ALLOWED_DEPTH: u8 = 5;

/// Check for hard compile errors in join configurations.
/// Returns non-empty only for errors that MUST block compilation.
/// Advisory warnings are handled by `generate_bidirectional_checks` instead.
pub fn generate_cyclic_dependency_check(
    _analysis: &EntityFieldAnalysis,
    _entity_name: &str,
) -> proc_macro2::TokenStream {
    // All checks moved to generate_bidirectional_checks which is included
    // in the main output (not the early-return error path).
    quote! {}
}

/// Check if join depth is potentially problematic for performance
fn check_join_depth(
    field: &syn::Field,
    join_config: &crate::codegen::joins::JoinConfig,
    entity_name: &str,
    warnings: &mut Vec<proc_macro2::TokenStream>,
) {
    let field_name = field
        .ident
        .as_ref()
        .map_or_else(|| "unknown".to_string(), std::string::ToString::to_string);

    // These are advisory warnings, not hard errors — the runtime handles both cases safely.
    // Hard errors are reserved for bidirectional relations (check_bidirectional_relation)
    // which cause runtime stack overflow.
    //
    // We use #[deprecated] on a generated const to produce a warning with source location,
    // since stable Rust has no compile_warning!() macro.
    if let Some(depth) = join_config.depth {
        if depth > MAX_ALLOWED_DEPTH {
            let msg = format!(
                "crudcrate: join '{field_name}' has depth {depth}, but MAX_JOIN_DEPTH={MAX_ALLOWED_DEPTH}. \
                 Depth will be capped to {MAX_ALLOWED_DEPTH} at runtime. Consider using depth={MAX_ALLOWED_DEPTH} or less."
            );
            let warn_const = quote::format_ident!(
                "_CRUDCRATE_DEPTH_WARNING_{}_{}",
                entity_name.to_uppercase(),
                field_name.to_uppercase()
            );
            warnings.push(quote! {
                #[deprecated(note = #msg)]
                const #warn_const: () = ();
                const _: () = #warn_const;
            });
        }
    } else {
        // Check for self-referencing using full type path comparison (same as loading.rs)
        let inner_type = extract_api_struct_type_for_recursive_call(&field.ty);
        if inner_type.to_string().trim() == entity_name {
            let msg = format!(
                "crudcrate: self-referencing join '{field_name}' has no explicit depth. \
                 Defaults to depth={MAX_ALLOWED_DEPTH} at runtime. \
                 Consider: #[crudcrate(join(..., depth = 2))]"
            );
            let warn_const = quote::format_ident!(
                "_CRUDCRATE_SELFREF_WARNING_{}_{}",
                entity_name.to_uppercase(),
                field_name.to_uppercase()
            );
            warnings.push(quote! {
                #[deprecated(note = #msg)]
                const #warn_const: () = ();
                const _: () = #warn_const;
            });
        }
    }
}

/// Generate compile-time bidirectional relation detection for all join fields.
///
/// Uses the `impls!` crate to check at compile time whether each join target entity
/// has a `Related<SelfEntity>` impl. If so:
/// - `depth = 1`: OK (crudcrate uses safe `Entity::find().filter()`)
/// - `depth` unspecified: COMPILE ERROR (must explicitly set depth = 1)
/// - `depth > 1`: COMPILE ERROR (recursive `get_one()` calls would infinitely recurse)
pub fn generate_bidirectional_checks(
    analysis: &EntityFieldAnalysis,
    entity_name: &str,
) -> proc_macro2::TokenStream {
    let mut checks = Vec::new();

    let mut seen = std::collections::HashSet::new();
    let all_join_fields: Vec<&syn::Field> = analysis
        .join_on_one_fields
        .iter()
        .chain(analysis.join_on_all_fields.iter())
        .copied()
        .filter(|f| {
            f.ident
                .as_ref()
                .is_none_or(|name| seen.insert(name.to_string()))
        })
        .collect();

    for field in &all_join_fields {
        let result = get_join_config(field);
        if let Some(ref join_config) = result.config {
            check_join_depth(field, join_config, entity_name, &mut checks);
        }
        if let Some(tokens) = check_bidirectional_relation(field, entity_name) {
            checks.push(tokens);
        }
    }

    quote! { #( #checks )* }
}

/// Check if a join field's target entity has a `Related<SelfEntity>` impl (bidirectional).
///
/// SeaORM's `find_related()` infinitely recurses when both A: Related<B> and B: Related<A>
/// exist, because `Relation::def()` calls through `EntityTrait::has_many/has_one` which
/// resolves the reverse relation's `def()`, creating an infinite loop.
///
/// - `depth = 1` is safe: crudcrate uses `Entity::find().filter()`, not `find_related()`
/// - No depth or `depth > 1` is unsafe: emits a compile error requiring `depth = 1`
fn check_bidirectional_relation(
    field: &syn::Field,
    entity_name: &str,
) -> Option<proc_macro2::TokenStream> {
    // Skip self-referencing joins (already handled separately in loading.rs).
    // Compare full type path via token stream, not substring matching.
    let inner_type = extract_api_struct_type_for_recursive_call(&field.ty);
    if inner_type.to_string().trim() == entity_name {
        return None;
    }

    let join_config = get_join_config(field);
    let depth = join_config.config.as_ref().and_then(|c| c.depth);

    // Get the target Entity path from the field type
    let target_entity = get_path_from_field_type(&field.ty, "Entity");

    let field_name = field
        .ident
        .as_ref()
        .map_or_else(|| "unknown".to_string(), std::string::ToString::to_string);

    match depth {
        Some(1) => {
            // depth = 1 is safe — crudcrate uses Entity::find().filter(), not find_related().
            // Generate a const flag for documentation / downstream checks.
            let const_name = quote::format_ident!(
                "_BIDIRECTIONAL_RELATION_{}_{}",
                entity_name.to_uppercase(),
                field_name.to_uppercase()
            );
            Some(quote! {
                #[doc(hidden)]
                pub const #const_name: bool =
                    crudcrate::impls!(#target_entity: sea_orm::Related<Entity>);
            })
        }
        Some(d) => {
            // Direct bidirectional = 2-hop cycle. Max safe depth = cycle_length - 1 = 1.
            let cycle_length = 2u8;
            let max_safe = cycle_length - 1;
            let msg = format!(
                "Bidirectional SeaORM relation on join '{field_name}' in '{entity_name}' \
                 with depth = {d}.\n\
                 \n\
                 Detected cycle ({cycle_length} hops): {entity_name} -> {field_name} -> {entity_name}\n\
                 Maximum safe depth = cycle_length - 1 = {cycle_length} - 1 = {max_safe}\n\
                 \n\
                 At depth >= {cycle} the child's get_one() loads its own joins — including \
                 the relation back to {entity_name} — causing infinite recursion through \
                 SeaORM's Relation::def() chain.\n\
                 \n\
                 Fix: #[crudcrate(join(one, depth = {max_safe}))]",
                cycle = max_safe + 1
            );
            Some(quote! {
                const _: () = assert!(
                    !crudcrate::impls!(#target_entity: sea_orm::Related<Entity>),
                    #msg
                );
            })
        }
        None => {
            let cycle_length = 2u8;
            let max_safe = cycle_length - 1;
            let msg = format!(
                "Bidirectional SeaORM relation on join '{field_name}' in '{entity_name}' \
                 with no explicit depth (defaults to 5).\n\
                 \n\
                 Detected cycle ({cycle_length} hops): {entity_name} -> {field_name} -> {entity_name}\n\
                 Maximum safe depth = cycle_length - 1 = {cycle_length} - 1 = {max_safe}\n\
                 \n\
                 At depth >= {cycle} the child's get_one() loads its own joins — including \
                 the relation back to {entity_name} — causing infinite recursion through \
                 SeaORM's Relation::def() chain.\n\
                 \n\
                 Fix: #[crudcrate(join(one, depth = {max_safe}))]",
                cycle = max_safe + 1
            );
            Some(quote! {
                const _: () = assert!(
                    !crudcrate::impls!(#target_entity: sea_orm::Related<Entity>),
                    #msg
                );
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_self_ref_detection_exact_match() {
        // "Category" -> Vec<Category> should be detected as self-referencing
        let ty: syn::Type = parse_quote!(Vec<Category>);
        let inner = extract_api_struct_type_for_recursive_call(&ty);
        assert_eq!(inner.to_string().trim(), "Category");
    }

    #[test]
    fn test_self_ref_detection_no_false_positive() {
        // "Site" -> Vec<SiteReplicate> must NOT match as self-referencing
        let ty: syn::Type = parse_quote!(Vec<SiteReplicate>);
        let inner = extract_api_struct_type_for_recursive_call(&ty);
        assert_ne!(inner.to_string().trim(), "Site");
    }

    #[test]
    fn test_self_ref_detection_full_path_no_false_positive() {
        // Full path: "crate :: sites :: replicates :: db :: SiteReplicate" != "Site"
        let ty: syn::Type = parse_quote!(Vec<crate::sites::replicates::db::SiteReplicate>);
        let inner = extract_api_struct_type_for_recursive_call(&ty);
        assert_ne!(inner.to_string().trim(), "Site");
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
