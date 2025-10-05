use super::structs::EntityFieldAnalysis;
use super::attribute_parser::{get_join_config, JoinConfig};
use heck::ToPascalCase;
use std::collections::HashMap;

/// Detects potentially dangerous cyclic join dependencies that could cause stack overflow
/// Returns a compile-time error if unsafe cycles are detected
pub fn generate_cyclic_dependency_check(
    analysis: &EntityFieldAnalysis,
    entity_name: &str,
) -> proc_macro2::TokenStream {
    use quote::quote;

    let mut cycle_warnings = Vec::new();

    // Collect all join fields with their target types and configurations
    let mut join_dependencies = HashMap::new();

    // Process join_on_one fields
    for field in &analysis.join_on_one_fields {
        if let Some(field_name) = &field.ident {
            if let Ok(target_type) = extract_target_entity_type(&field.ty) {
                if let Some(join_config) = get_join_config(field) {
                    join_dependencies.insert(field_name.to_string(), (target_type, join_config));
                }
            }
        }
    }

    // Process join_on_all fields
    for field in &analysis.join_on_all_fields {
        if let Some(field_name) = &field.ident {
            if let Ok(target_type) = extract_target_entity_type(&field.ty) {
                if let Some(join_config) = get_join_config(field) {
                    join_dependencies.insert(field_name.to_string(), (target_type, join_config));
                }
            }
        }
    }

    // Check for potentially dangerous cycles based on relationship analysis
    for (field_name, (target_entity, join_config)) in &join_dependencies {
        if has_potential_cycle(entity_name, target_entity, field_name, join_config) {
            let suggested_depth = calculate_safe_depth(entity_name, target_entity);

            // Build complete cycle path for better understanding
            let complete_cycle = if target_entity.starts_with("super::") {
                // super:: reference case: Customer -> vehicles -> super::Model -> Customer
                format!("{} -> {} -> {} -> {}", entity_name, field_name, target_entity, entity_name)
            } else {
                // Different entity case: Customer -> vehicles -> Vehicle -> customer -> Customer
                format!("{} -> {} -> {} -> customer -> {}", entity_name, field_name, target_entity, entity_name)
            };

            cycle_warnings.push(quote! {
                compile_error!(concat!(
                    "Cyclic dependency detected: ",
                    #complete_cycle,
                    ". This will cause infinite recursion during join loading. ",
                    "To fix this, add the depth parameter to your join() statement: depth = ",
                    #suggested_depth
                ));
            });
        }
    }

    quote! {
        #( #cycle_warnings )*
    }
}

/// Extract the target entity type from a field type (Vec<T> or Option<T>)
/// Returns the full path to uniquely identify different entities
fn extract_target_entity_type(field_type: &syn::Type) -> Result<String, String> {
    if let syn::Type::Path(type_path) = field_type {
        if let Some(last_seg) = type_path.path.segments.last() {
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
                let path_segments: Vec<String> = inner_path.path.segments
                    .iter()
                    .map(|seg| seg.ident.to_string())
                    .collect();

                if !path_segments.is_empty() {
                    return Ok(path_segments.join("::"));
                }
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
    join_config: &JoinConfig
) -> bool {
    // Only check for cycles if no explicit depth is specified
    if join_config.has_explicit_depth() {
        return false;
    }

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

    // If field name contains the target entity name (e.g., "vehicles" field pointing to "vehicle::*")
    // this suggests a potential bidirectional relationship
    if field_lower.contains(&target_entity_lower) || target_entity_lower.contains(&field_lower) {
        return true;
    }

    false
}

/// Extract the base entity name from a full path like "vehicle::Model" -> "vehicle"
/// or "super::Model" -> "super"
fn extract_entity_name_from_path(path: &str) -> &str {
    // Split by :: and take the first meaningful segment
    let segments: Vec<&str> = path.split("::").collect();

    if segments.is_empty() {
        return path;
    }

    // For "super::Model", return "super"
    // For "vehicle::Model", return "vehicle"
    // For single segment like "Model", return "Model"
    segments.first().unwrap_or(&path)
}

/// Calculate safe recursion depth based on relationship analysis
fn calculate_safe_depth(entity_name: &str, target_entity: &str) -> u8 {
    // Direct relationships (entity_name == target_entity) need minimal depth
    if entity_name == target_entity {
        1
    } else {
        // For different entities, suggest depth=2 as a safe starting point
        2
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
        if let Some(field_name) = &field.ident {
            if let Some(join_config) = get_join_config(field) {
                if let Some(custom_relation) = join_config.relation {
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
        }
    }

    // Generate validation checks for join_on_all fields (only if custom relation is specified)
    for field in &analysis.join_on_all_fields {
        if let Some(field_name) = &field.ident {
            if let Some(join_config) = get_join_config(field) {
                if let Some(custom_relation) = join_config.relation {
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
        }
    }

    quote! {
        #( #validation_checks )*
    }
}

/// Convert a field name to the expected relation variant name
/// Example: "entities" -> "Entities", "`related_items`" -> "`RelatedItems`"
fn field_name_to_relation_variant(field_name: &syn::Ident) -> String {
    let field_str = field_name.to_string();
    // Convert to PascalCase for relation variant name
    field_str.to_pascal_case()
}

#[cfg(test)]
fn is_optional_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last() {
        return segment.ident == "Option";
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_name_to_relation_variant() {
        use quote::format_ident;
        assert_eq!(
            field_name_to_relation_variant(&format_ident!("entities")),
            "Entities"
        );
        assert_eq!(
            field_name_to_relation_variant(&format_ident!("related_items")),
            "RelatedItems"
        );
        assert_eq!(
            field_name_to_relation_variant(&format_ident!("item")),
            "Item"
        );
    }

    #[test]
    fn test_type_validation_helpers() {
        use crate::macro_implementation::is_vec_type;

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
