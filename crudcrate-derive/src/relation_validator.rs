use super::attribute_parser::{JoinConfig, get_join_config};
use super::field_analyzer::find_crudcrate_join_attr;
use super::structs::EntityFieldAnalysis;
use heck::ToPascalCase;
use std::collections::HashMap;

// Recursion depth constants
const DEFAULT_RECURSION_DEPTH: u8 = 3;
const SAFE_EXPLICIT_DEPTH: u8 = 2;
const WARNING_DEPTH_THRESHOLD: u8 = 3;

/// Detects potentially dangerous cyclic join dependencies that could cause stack overflow
/// Returns a compile-time error if unsafe cycles are detected
pub fn generate_cyclic_dependency_check(
    analysis: &EntityFieldAnalysis,
    entity_name: &str,
) -> proc_macro2::TokenStream {
    use quote::quote;

    let mut deep_recursion_warnings: Vec<proc_macro2::TokenStream> = Vec::new();

    // Collect all join fields with their target types and configurations
    let mut join_dependencies = HashMap::new();

    // Process join_on_one fields
    for field in &analysis.join_on_one_fields {
        if let Some(field_name) = &field.ident
            && let Ok(target_type) = extract_target_entity_type(&field.ty)
            && let Some(join_config) = get_join_config(field)
        {
            join_dependencies.insert(field_name.to_string(), (target_type, join_config));
        }
    }

    // Process join_on_all fields
    for field in &analysis.join_on_all_fields {
        if let Some(field_name) = &field.ident
            && let Ok(target_type) = extract_target_entity_type(&field.ty)
            && let Some(join_config) = get_join_config(field)
        {
            join_dependencies.insert(field_name.to_string(), (target_type, join_config));
        }
    }

    // Check for joins with unlimited recursion (no explicit depth set)
    for (field_name, (target_entity, join_config)) in &join_dependencies {
        if join_config.is_unlimited_recursion() {
            let estimated_depth =
                estimate_relationship_depth(entity_name, target_entity, field_name);

            if estimated_depth > WARNING_DEPTH_THRESHOLD {
                let warning_path = format!(
                    "{entity_name} -> {field_name} -> {target_entity} (estimated depth: {estimated_depth})"
                );

                let default_depth = DEFAULT_RECURSION_DEPTH;
                let safe_depth = SAFE_EXPLICIT_DEPTH;
                deep_recursion_warnings.push(quote! {
                    compile_error!(concat!(
                        "Deep recursion warning: ",
                        #warning_path,
                        ". This join will recurse more than ", stringify!(#default_depth), " levels deep by default, which may impact performance. ",
                        "Consider adding explicit depth control: join(..., depth = ", stringify!(#default_depth), ") or join(..., depth = ", stringify!(#safe_depth), ")."
                    ));
                });
            }
        }
    }

    // Check for cyclic dependencies that could cause infinite recursion
    // Enhanced logic: Only flag cycles that are actually unsafe (unlimited recursion)
    let cycle_warnings: Vec<proc_macro2::TokenStream> = join_dependencies
        .iter()
        .filter_map(|(field_name, (target_entity, join_config))| {
            if is_unsafe_cycle(entity_name, target_entity, field_name, join_config, &join_dependencies) {
                let warning_path = format!("{entity_name} -> {field_name} -> {target_entity}");

                // Provide specific guidance based on the issue
                let guidance = if join_config.is_unlimited_recursion() {
                    "Unlimited recursion detected. Add explicit depth limit: depth = 1"
                } else {
                    "This bidirectional relationship creates infinite recursion. Use depth = 1 on at least one side"
                };

                // Use syn::Error for better error spanning
                let error = if let Some(crudcrate_attr) = analysis
                    .join_on_one_fields
                    .iter()
                    .chain(&analysis.join_on_all_fields)
                    .find(|f| f.ident.as_ref().map(std::string::ToString::to_string) == Some(field_name.clone()))
                    .and_then(|f| find_crudcrate_join_attr(f))
                {
                    // Target the crudcrate join attribute specifically
                    syn::Error::new_spanned(crudcrate_attr, format!(
                        "Cyclic dependency detected: {warning_path}. {guidance}"
                    ))
                } else {
                    // Fallback to targeting the field
                    let field_span = analysis.join_on_one_fields
                        .iter()
                        .chain(&analysis.join_on_all_fields)
                        .find(|f| f.ident.as_ref().map(std::string::ToString::to_string) == Some(field_name.clone()))
                        .map(|f| f.ident.as_ref().unwrap().span());

                    if let Some(span) = field_span {
                        syn::Error::new(span, format!(
                            "Cyclic dependency detected: {warning_path}. {guidance}"
                        ))
                    } else {
                        syn::Error::new(proc_macro2::Span::call_site(), format!(
                            "Cyclic dependency detected: {warning_path}. {guidance}"
                        ))
                    }
                };

                Some(error.to_compile_error())
            } else {
                None
            }
        })
        .collect();

    // Generate helpful validation guidance for Sea-ORM relations
    let relation_validation_warnings: Vec<proc_macro2::TokenStream> = join_dependencies
        .iter()
        .map(|(_field_name, (target_entity, _join_config))| {
            // Extract module path and entity name from target (e.g., "module::Model" -> "module", "Model")
            let target_parts: Vec<&str> = target_entity.split("::").collect();
            let (_target_module, _target_model_name) = if target_parts.len() >= 2 {
                (target_parts[0], target_parts[1])
            } else {
                ("unknown", "Unknown")
            };

            // Generate helpful validation guidance for Sea-ORM relations
            quote! {
                // Sea-ORM Relation Required: #target_entity
                // If compilation fails, ensure your target entity has:
                // 1. #[derive(DeriveRelation)] on the Relation enum
                // 2. A Relation variant pointing to this entity
                // 3. An impl Related<ThisEntity> block
                //
                // Example structure for #target_model_name:
                // #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
                // pub enum Relation {
                //     #[sea_orm(belongs_to = "super::source_entity::Entity", ...)]
                //     SourceEntity,
                // }
                //
                // impl Related<super::source_entity::Entity> for Entity {
                //     fn to() -> RelationDef {
                //         Relation::SourceEntity.def()
                //     }
                // }
            }
        })
        .collect();

    quote! {
        #( #cycle_warnings )*
        #( #deep_recursion_warnings )*
        #( #relation_validation_warnings )*
    }
}

/// Extract the target entity type from a field type (Vec<T> or Option<T>)
/// Returns the full path to uniquely identify different entities
fn extract_target_entity_type(field_type: &syn::Type) -> Result<String, String> {
    if let syn::Type::Path(type_path) = field_type
        && let Some(last_seg) = type_path.path.segments.last()
    {
        let inner_type = match last_seg.ident.to_string().as_str() {
            "Vec" => {
                if let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                        inner_type
                    } else {
                        return Err("Invalid Vec type".to_string());
                    }
                } else {
                    return Err("Invalid Vec arguments".to_string());
                }
            }
            "Option" => {
                if let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                        inner_type
                    } else {
                        return Err("Invalid Option type".to_string());
                    }
                } else {
                    return Err("Invalid Option arguments".to_string());
                }
            }
            _ => field_type,
        };

        // Extract the full path from the inner type to uniquely identify entities
        if let syn::Type::Path(inner_path) = inner_type {
            let path_segments: Vec<String> = inner_path
                .path
                .segments
                .iter()
                .map(|seg| seg.ident.to_string())
                .collect();

            if !path_segments.is_empty() {
                return Ok(path_segments.join("::"));
            }
        }
    }

    Err("Could not extract entity type".to_string())
}

/// Analyzes if a relationship could potentially create a cycle
/// Uses semantic analysis based on join configurations rather than string manipulation
fn has_potential_cycle(
    entity_name: &str,
    target_entity: &str,
    field_name: &str,
    _join_config: &JoinConfig,
) -> bool {
    // Check for direct entity name match (self-referencing relationships)
    if entity_name == target_entity {
        return true;
    }

    // Extract entity name from target path
    let target_entity_name = extract_entity_name_from_path(target_entity);

    // Pattern 1: super:: references suggest parent-child relationships that could be cyclic
    if target_entity.contains("super::") {
        return true;
    }

    // Pattern 2: Check if field name suggests a relationship that could be bidirectional
    // by analyzing the relationship between field name and target entity
    let field_lower = field_name.to_lowercase();
    let target_entity_lower = target_entity_name.to_lowercase();

    // If field name contains the target entity name (e.g., "items" field pointing to "module::Model")
    // this suggests a potential bidirectional relationship
    if field_lower.contains(&target_entity_lower) || target_entity_lower.contains(&field_lower) {
        return true;
    }

    false
}

/// Enhanced cycle detection that only flags truly unsafe bidirectional relationships
/// Returns true if the relationship could cause infinite recursion
fn is_unsafe_cycle(
    entity_name: &str,
    target_entity: &str,
    field_name: &str,
    join_config: &JoinConfig,
    all_dependencies: &std::collections::HashMap<String, (String, JoinConfig)>,
) -> bool {
    // Self-referencing relationships are always unsafe
    if entity_name == target_entity {
        return true;
    }

    // Check for unlimited recursion (always unsafe)
    if join_config.is_unlimited_recursion() {
        // But only if there's a potential cycle detected
        return has_potential_cycle(entity_name, target_entity, field_name, join_config);
    }

    // For relationships with explicit depths, check if there's actually a reverse relationship
    let target_entity_name = extract_entity_name_from_path(target_entity);

    // Look for the reverse relationship (B->A when checking A->B)
    let has_reverse_relationship =
        all_dependencies
            .iter()
            .any(|(_reverse_field_name, (reverse_target, _reverse_config))| {
                let reverse_target_name = extract_entity_name_from_path(reverse_target);

                // Check if this is actually a reverse relationship (target entity points back to source entity)
                // This must be a true bidirectional pattern, not just any relationship
                (reverse_target_name.contains(&entity_name.to_lowercase())
                    || entity_name.to_lowercase().contains(&reverse_target_name))
                    && (target_entity_name.contains(&reverse_target_name.to_lowercase())
                        || reverse_target_name
                            .to_lowercase()
                            .contains(&target_entity_name))
            });

    // If there's no reverse relationship, then it's unidirectional and safe with explicit depth
    if !has_reverse_relationship {
        return false; // Unidirectional relationships are safe with explicit depth
    }

    // If we get here, there IS a bidirectional relationship
    // Now check if both sides have explicit depth limits
    for (reverse_target, reverse_config) in all_dependencies.values() {
        let reverse_target_name = extract_entity_name_from_path(reverse_target);

        // Find the actual reverse relationship
        if (reverse_target_name.contains(&entity_name.to_lowercase())
            || entity_name.to_lowercase().contains(&reverse_target_name))
            && (target_entity_name.contains(&reverse_target_name.to_lowercase())
                || reverse_target_name
                    .to_lowercase()
                    .contains(&target_entity_name))
        {
            // Both sides have explicit depth limits - this is safe
            if join_config.depth.is_some() && reverse_config.depth.is_some() {
                return false; // Safe bidirectional with explicit depths
            }

            // One side has unlimited recursion - unsafe
            if reverse_config.is_unlimited_recursion() {
                return true;
            }
        }
    }

    // Fall back to the original heuristic for ambiguous cases
    has_potential_cycle(entity_name, target_entity, field_name, join_config)
}

/// Extract the base entity name from a full path
/// Examples: "`entity::Model`" -> "Entity", "`super::entity::Model`" -> "Entity"
fn extract_entity_name_from_path(path: &str) -> String {
    // Split by :: and take the meaningful segment
    let segments: Vec<&str> = path.split("::").collect();

    if segments.is_empty() {
        return path.to_string();
    }

    // Handle different path patterns
    match segments.as_slice() {
        // "super::entity::Model" -> "Entity" or "entity::Model" -> "Entity"
        ["super", module, "Model"] | [module, "Model"] => module.to_pascal_case(),

        // Single segment like "Model" -> "Model"
        [single] => (*single).to_string(),

        // Fallback: take first meaningful segment (skip "super") and convert to PascalCase
        segments => {
            let meaningful_segment = if segments.first() == Some(&"super") {
                segments.get(1).unwrap_or(&"Unknown")
            } else {
                segments.first().unwrap_or(&"Unknown")
            };
            meaningful_segment.to_pascal_case()
        }
    }
}

/// Generates compile-time validation code for join relations
/// Since proc macros cannot access sibling Relation enums, we generate code that
/// references the required relations - if they don't exist, compilation fails
pub fn generate_join_relation_validation(
    analysis: &EntityFieldAnalysis,
) -> proc_macro2::TokenStream {
    use quote::quote;

    let mut validation_checks = Vec::new();

    // Generate validation checks for join_on_one fields (only if custom relation is specified)
    for field in &analysis.join_on_one_fields {
        if let Some(field_name) = &field.ident
            && let Some(join_config) = get_join_config(field)
            && let Some(custom_relation) = join_config.relation
        {
            // Only validate if a custom relation name is explicitly provided
            let expected_relation_ident = syn::Ident::new(&custom_relation, field_name.span());

            // Generate a compile-time check that references the relation
            validation_checks.push(quote! {
                // Compile-time validation: This will fail if Relation::#expected_relation_ident doesn't exist
                const _: () = {
                    fn _validate_relation_exists() {
                        let _ = Relation::#expected_relation_ident;
                    }
                };
            });
        }
        // If no custom relation is specified, we use entity path resolution - no validation needed
    }

    // Generate validation checks for join_on_all fields (only if custom relation is specified)
    for field in &analysis.join_on_all_fields {
        if let Some(field_name) = &field.ident
            && let Some(join_config) = get_join_config(field)
            && let Some(custom_relation) = join_config.relation
        {
            // Only validate if a custom relation name is explicitly provided
            let expected_relation_ident = syn::Ident::new(&custom_relation, field_name.span());

            validation_checks.push(quote! {
                // Compile-time validation: This will fail if Relation::#expected_relation_ident doesn't exist
                const _: () = {
                    fn _validate_relation_exists() {
                        let _ = Relation::#expected_relation_ident;
                    }
                };
            });
        }
        // If no custom relation is specified, we use entity path resolution - no validation needed
    }

    quote! {
        #( #validation_checks )*
    }
}

#[cfg(test)]
fn is_optional_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "Option";
    }
    false
}

/// Estimate the potential recursion depth for a relationship
/// This is a heuristic that analyzes relationship patterns to estimate how deep recursion might go
fn estimate_relationship_depth(current_entity: &str, target_entity: &str, field_name: &str) -> u8 {
    // Simple heuristic based on relationship patterns
    // Could be enhanced with graph analysis in the future

    // Base case: direct relationships typically add 1 level
    let mut estimated_depth = 1;

    // Check field name patterns that suggest deeper relationships
    let field_lower = field_name.to_lowercase();

    // Plural field names often indicate collection relationships
    if field_lower.ends_with('s') && field_lower.len() > 3 {
        estimated_depth += 2;
    }

    // Common hierarchical patterns
    if field_lower.contains("sub")
        || field_lower.contains("child")
        || field_lower.contains("nested")
    {
        estimated_depth += 2;
    }

    // Tree-like structures
    if field_lower.contains("categor")
        || field_lower.contains("tree")
        || field_lower.contains("parent")
    {
        estimated_depth += 2;
    }

    // Chain-like structures
    if field_lower.contains("next") || field_lower.contains("prev") || field_lower.contains("chain")
    {
        estimated_depth += 3;
    }

    // Self-referencing relationships suggest deeper recursion
    if target_entity.starts_with("super::") || target_entity.contains(current_entity) {
        estimated_depth += 2;
    }

    estimated_depth
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_validation_helpers() {
        use crate::codegen::type_resolution::is_vec_type;

        let vec_type: syn::Type = syn::parse_quote!(Vec<String>);
        let option_type: syn::Type = syn::parse_quote!(Option<String>);
        let plain_type: syn::Type = syn::parse_quote!(String);

        // Test is_vec_type function
        assert!(is_vec_type(&vec_type));
        assert!(!is_vec_type(&option_type));
        assert!(!is_vec_type(&plain_type));

        assert!(!is_optional_type(&vec_type));
        assert!(is_optional_type(&option_type));
        assert!(!is_optional_type(&plain_type));
    }
}
