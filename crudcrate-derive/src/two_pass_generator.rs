//! Two-pass generation system for resolving join field types
//!
//! This module implements a two-pass approach to handle circular dependencies
//! in join field type resolution:
//!
//! Pass 1: Discovery - Scan entities and build a type registry
//! Pass 2: Generation - Use the registry to generate properly-typed code

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::collections::HashMap;
use std::sync::Mutex;
use syn::Type;

/// Registry of discovered entity types and their API struct names
#[derive(Debug, Default)]
pub struct EntityTypeRegistry {
    /// Maps entity type names (e.g., "Vehicle") to API struct names (e.g., "Vehicle")
    /// Handles both same-name and renamed API structs
    entity_to_api: HashMap<String, String>,
}

impl EntityTypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an entity and its API struct name
    pub fn register_entity(&mut self, entity_name: String, api_struct_name: String) {
        self.entity_to_api.insert(entity_name, api_struct_name);
    }

    /// Resolve a join field type to its proper API struct type
    pub fn resolve_join_type(&self, field_type: &Type) -> Option<TokenStream> {
        // For type aliases, we need to preserve the container structure while resolving the inner type
        if let Type::Path(type_path) = field_type
            && let Some(segment) = type_path.path.segments.last()
        {
            let type_name = segment.ident.to_string();

            // Handle type aliases that end with "Join" (VehicleJoin, VehiclePartJoin, etc.)
            if type_name.ends_with("Join") {
                // These are type aliases like `pub type VehicleJoin = Vec<super::vehicle::Vehicle>;`
                // We assume they are collection types (Vec<T>) based on the pattern
                let base_type = Self::extract_base_type(field_type)?;

                // Try multiple resolution strategies
                let api_struct_name = if let Some(api_name) = self.entity_to_api.get(&base_type) {
                    // Direct mapping found
                    api_name
                } else if let Some(api_name) = self.entity_to_api.get(&format!("{base_type}API")) {
                    // Try with API suffix
                    api_name
                } else if let Some(api_name) = self.entity_to_api.get(&Self::pluralize(&base_type))
                {
                    // Try with plural form (Vehicle -> Vehicles)
                    api_name
                } else {
                    // Fallback to base type (this should work for most cases)
                    &base_type
                };

                // Type aliases ending with "Join" are assumed to be Vec<T> based on the pattern
                // e.g., VehicleJoin = Vec<super::vehicle::Vehicle> -> Vec<Vehicle>
                let api_ident = format_ident!("{}", api_struct_name);
                return Some(quote! { Vec<#api_ident> });
            }
        }

        // For regular types (Vec<T>, Option<T>, or direct T), use the standard resolution
        let base_type = Self::extract_base_type(field_type)?;

        // Try multiple resolution strategies
        let api_struct_name = if let Some(api_name) = self.entity_to_api.get(&base_type) {
            // Direct mapping found
            api_name
        } else if let Some(api_name) = self.entity_to_api.get(&format!("{base_type}API")) {
            // Try with API suffix
            api_name
        } else if let Some(api_name) = self.entity_to_api.get(&Self::pluralize(&base_type)) {
            // Try with plural form (Vehicle -> Vehicles)
            api_name
        } else {
            // Fallback to base type (this should work for most cases)
            &base_type
        };

        // Reconstruct the full type with the resolved API struct
        self.reconstruct_type(field_type, api_struct_name)
    }

    /// Simple pluralization for common cases
    fn pluralize(word: &str) -> String {
        if let Some(stripped) = word.strip_suffix('y') {
            format!("{stripped}ies")
        } else if word.ends_with('s') {
            word.to_string()
        } else {
            format!("{word}s")
        }
    }

    fn extract_base_type(field_type: &Type) -> Option<String> {
        if let Type::Path(type_path) = field_type
            && let Some(segment) = type_path.path.segments.last()
        {
            let ident = segment.ident.to_string();

            if (ident == "Vec" || ident == "Option")
                && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
                && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
            {
                return Self::extract_base_type(inner_ty);
            }

            if ident.ends_with("Join") {
                let base_name = ident.strip_suffix("Join").unwrap_or(&ident);
                return Some(heck::ToPascalCase::to_pascal_case(base_name));
            }

            // Handle Sea-ORM model paths (super::vehicle::Model -> Vehicle)
            let path_segments: Vec<String> = type_path
                .path
                .segments
                .iter()
                .map(|seg| seg.ident.to_string())
                .collect();

            // Convert Sea-ORM model paths to entity names
            if path_segments.len() == 3
                && path_segments[0] == "super"
                && path_segments[2] == "Model"
            {
                // "super::vehicle::Model" -> "Vehicle"
                return Some(heck::ToPascalCase::to_pascal_case(
                    path_segments[1].as_str(),
                ));
            } else if path_segments.len() == 2 && path_segments[1] == "Model" {
                // "vehicle::Model" -> "Vehicle"
                return Some(heck::ToPascalCase::to_pascal_case(
                    path_segments[0].as_str(),
                ));
            }

            // Handle other patterns
            // Skip "super" and take the next meaningful segment
            let meaningful_segment =
                if path_segments.first().map(std::string::String::as_str) == Some("super") {
                    path_segments.get(1)
                } else {
                    path_segments.first()
                };

            if let Some(segment) = meaningful_segment {
                if segment.as_str() == "Model" {
                    // If we only have "Model", use the previous segment as entity name
                    if path_segments.len() >= 2 {
                        return Some(heck::ToPascalCase::to_pascal_case(
                            path_segments[path_segments.len() - 2].as_str(),
                        ));
                    }
                } else {
                    return Some(heck::ToPascalCase::to_pascal_case(segment.as_str()));
                }
            }

            // Handle API struct suffixes (VehicleAPI -> Vehicle)
            if ident.ends_with("API") {
                return Some(ident.strip_suffix("API").unwrap_or(&ident).to_string());
            }

            return Some(heck::ToPascalCase::to_pascal_case(ident.as_str()));
        }
        None
    }

    /// Reconstruct the full type with resolved API struct name
    fn reconstruct_type(&self, original_type: &Type, api_struct_name: &str) -> Option<TokenStream> {
        if let Type::Path(type_path) = original_type
            && let Some(segment) = type_path.path.segments.last()
        {
            let ident = segment.ident.to_string();

            if (ident == "Vec" || ident == "Option")
                && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
                && let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first()
            {
                let inner_base_type = Self::extract_base_type(inner_ty)?;

                let resolved_inner_name =
                    if let Some(resolved_api) = self.entity_to_api.get(&inner_base_type) {
                        resolved_api
                    } else if let Some(resolved_api) =
                        self.entity_to_api.get(&Self::pluralize(&inner_base_type))
                    {
                        resolved_api
                    } else {
                        &inner_base_type
                    };

                let api_ident = format_ident!("{}", resolved_inner_name);

                if ident == "Vec" {
                    return Some(quote! { Vec<#api_ident> });
                }
                return Some(quote! { Option<#api_ident> });
            }

            // Direct type replacement for non-Join types
            let api_ident = format_ident!("{}", api_struct_name);
            return Some(quote! { #api_ident });
        }
        None
    }
}

/// Global registry for cross-entity type resolution using thread-safe storage
static GLOBAL_TYPE_REGISTRY: Mutex<Option<EntityTypeRegistry>> = Mutex::new(None);

/// Register an entity in the global registry
pub fn register_entity_globally(entity_name: &str, api_struct_name: &str) {
    let mut registry = GLOBAL_TYPE_REGISTRY.lock().unwrap();
    if registry.is_none() {
        *registry = Some(EntityTypeRegistry::new());
    }
    if let Some(ref mut reg) = *registry {
        reg.register_entity(entity_name.to_string(), api_struct_name.to_string());
        #[cfg(feature = "debug")]
        eprintln!("🔍 REGISTER: '{}' -> '{}'", entity_name, api_struct_name);
    }
}

/// Resolve a join field type using the global registry
pub fn resolve_join_type_globally(field_type: &Type) -> Option<TokenStream> {
    let registry = GLOBAL_TYPE_REGISTRY.lock().unwrap();
    if let Some(ref reg) = *registry {
        #[cfg(feature = "debug")]
        eprintln!("🔍 RESOLVE: trying to resolve {:?}", quote! {#field_type});
        let result = reg.resolve_join_type(field_type);
        #[cfg(feature = "debug")]
        eprintln!("🔍 RESOLVE: result: {:?}", result);
        result
    } else {
        #[cfg(feature = "debug")]
        eprintln!("🔍 RESOLVE: no registry available");
        None
    }
}

/// Extract the base type name as a string (e.g., "Vehicle" from Vec<Vehicle> or `super::vehicle::Model`)
pub fn extract_base_type_string(field_type: &Type) -> Option<String> {
    EntityTypeRegistry::extract_base_type(field_type)
}

/// Find the API struct name for a given base type (e.g., "Vehicle" -> "Vehicle")
pub fn find_api_struct_name(base_type: &str) -> Option<String> {
    let registry = GLOBAL_TYPE_REGISTRY.lock().unwrap();
    if let Some(ref reg) = *registry {
        // Try multiple resolution strategies
        if let Some(api_name) = reg.entity_to_api.get(base_type) {
            Some(api_name.clone())
        } else if let Some(api_name) = reg.entity_to_api.get(&format!("{base_type}API")) {
            Some(api_name.clone())
        } else if let Some(api_name) = reg
            .entity_to_api
            .get(&EntityTypeRegistry::pluralize(base_type))
        {
            Some(api_name.clone())
        } else {
            // Default to the base type itself
            Some(base_type.to_string())
        }
    } else {
        None
    }
}
