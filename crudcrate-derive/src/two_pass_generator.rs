//! Two-pass generation system for resolving join field types
//!
//! This module implements a two-pass approach to handle circular dependencies
//! in join field type resolution:
//!
//! Pass 1: Discovery - Scan entities and build a type registry
//! Pass 2: Generation - Use the registry to generate properly-typed code

use std::collections::HashMap;
use std::sync::Mutex;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, Type};

/// Registry of discovered entity types and their API struct names
#[derive(Debug, Default)]
pub struct EntityTypeRegistry {
    /// Maps entity type names (e.g., "Vehicle") to API struct names (e.g., "Vehicle")
    /// Handles both same-name and renamed API structs
    entity_to_api: HashMap<String, String>,
    /// Maps module paths to entity names for complex imports
    module_to_entity: HashMap<String, String>,
}

impl EntityTypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an entity and its API struct name
    pub fn register_entity(&mut self, entity_name: String, api_struct_name: String) {
        self.entity_to_api.insert(entity_name, api_struct_name);
    }

    /// Register a module path mapping
    pub fn register_module(&mut self, module_path: String, entity_name: String) {
        self.module_to_entity.insert(module_path, entity_name);
    }

    /// Resolve a join field type to its proper API struct type
    pub fn resolve_join_type(&self, field_type: &Type) -> Option<TokenStream> {
        // For type aliases, we need to preserve the container structure while resolving the inner type
        if let Type::Path(type_path) = field_type {
            if let Some(segment) = type_path.path.segments.last() {
                let type_name = segment.ident.to_string();

                // Handle type aliases that end with "Join" (VehicleJoin, VehiclePartJoin, etc.)
                if type_name.ends_with("Join") {
                    // These are type aliases like `pub type VehicleJoin = Vec<super::vehicle::Vehicle>;`
                    // We assume they are collection types (Vec<T>) based on the pattern
                    let base_type = self.extract_base_type(field_type)?;

                    // Try multiple resolution strategies
                    let api_struct_name = if let Some(api_name) = self.entity_to_api.get(&base_type) {
                        // Direct mapping found
                        api_name
                    } else if let Some(api_name) = self.entity_to_api.get(&format!("{}API", base_type)) {
                        // Try with API suffix
                        api_name
                    } else if let Some(api_name) = self.entity_to_api.get(&self.pluralize(&base_type)) {
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
        }

        // For regular types (Vec<T>, Option<T>, or direct T), use the standard resolution
        let base_type = self.extract_base_type(field_type)?;

        // Try multiple resolution strategies
        let api_struct_name = if let Some(api_name) = self.entity_to_api.get(&base_type) {
            // Direct mapping found
            api_name
        } else if let Some(api_name) = self.entity_to_api.get(&format!("{}API", base_type)) {
            // Try with API suffix
            api_name
        } else if let Some(api_name) = self.entity_to_api.get(&self.pluralize(&base_type)) {
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
    fn pluralize(&self, word: &str) -> String {
        if word.ends_with('y') {
            format!("{}ies", &word[..word.len()-1])
        } else if word.ends_with('s') {
            word.to_string()
        } else {
            format!("{}s", word)
        }
    }

    fn extract_base_type(&self, field_type: &Type) -> Option<String> {
        if let Type::Path(type_path) = field_type {
            if let Some(segment) = type_path.path.segments.last() {
                let ident = segment.ident.to_string();

                if ident == "Vec" || ident == "Option" {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            return self.extract_base_type(inner_ty);
                        }
                    }
                }

                if ident.ends_with("Join") {
                    let base_name = ident.strip_suffix("Join").unwrap_or(&ident);
                    return Some(heck::ToPascalCase::to_pascal_case(base_name));
                }

                // Handle Sea-ORM model paths (super::vehicle::Model -> Vehicle)
                let path_segments: Vec<String> = type_path.path.segments
                    .iter()
                    .map(|seg| seg.ident.to_string())
                    .collect();

                // Convert Sea-ORM model paths to entity names
                if path_segments.len() == 3 && path_segments[0] == "super" && path_segments[2] == "Model" {
                    // "super::vehicle::Model" -> "Vehicle"
                    return Some(heck::ToPascalCase::to_pascal_case(path_segments[1].as_str()));
                } else if path_segments.len() == 2 && path_segments[1] == "Model" {
                    // "vehicle::Model" -> "Vehicle"
                    return Some(heck::ToPascalCase::to_pascal_case(path_segments[0].as_str()));
                } else {
                    // Handle other patterns
                    // Skip "super" and take the next meaningful segment
                    let meaningful_segment = if path_segments.first().map(|s| s.as_str()) == Some("super") {
                        path_segments.get(1)
                    } else {
                        path_segments.first()
                    };

                    if let Some(segment) = meaningful_segment {
                        if segment.as_str() == "Model" {
                            // If we only have "Model", use the previous segment as entity name
                            if path_segments.len() >= 2 {
                                return Some(heck::ToPascalCase::to_pascal_case(path_segments[path_segments.len() - 2].as_str()));
                            }
                        } else {
                            return Some(heck::ToPascalCase::to_pascal_case(segment.as_str()));
                        }
                    }
                }

                // Handle API struct suffixes (VehicleAPI -> Vehicle)
                if ident.ends_with("API") {
                    return Some(ident.strip_suffix("API").unwrap_or(&ident).to_string());
                }

                return Some(heck::ToPascalCase::to_pascal_case(ident.as_str()));
            }
        }
        None
    }

    /// Resolve type from module path (e.g., crate::models::Vehicle -> Vehicle)
    fn resolve_from_module_path(&self, _module_path: &str) -> Option<String> {
        // For now, try simple name resolution
        // TODO: Implement more sophisticated module path resolution
        None
    }

    /// Reconstruct the full type with resolved API struct name
    fn reconstruct_type(&self, original_type: &Type, api_struct_name: &str) -> Option<TokenStream> {
        if let Type::Path(type_path) = original_type {
            if let Some(segment) = type_path.path.segments.last() {
                let ident = segment.ident.to_string();

                if ident == "Vec" || ident == "Option" {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            let inner_base_type = self.extract_base_type(inner_ty)?;

                            let resolved_inner_name = if let Some(resolved_api) = self.entity_to_api.get(&inner_base_type) {
                                resolved_api
                            } else if let Some(resolved_api) = self.entity_to_api.get(&self.pluralize(&inner_base_type)) {
                                resolved_api
                            } else {
                                &inner_base_type
                            };

                            let api_ident = format_ident!("{}", resolved_inner_name);

                            if ident == "Vec" {
                                return Some(quote! { Vec<#api_ident> });
                            } else {
                                return Some(quote! { Option<#api_ident> });
                            }
                        }
                    }
                } else {
                    // Check if the original type name ends with "Join" (type alias case)
                    if ident.ends_with("Join") {
                        // For type aliases like "VehicleJoin", we need to check if the original field type
                        // was a Vec<T> or Option<T> and preserve that structure
                        // This is handled by the caller, so just return the resolved type
                        let api_ident = format_ident!("{}", api_struct_name);
                        return Some(quote! { #api_ident });
                    } else {
                        // Direct type replacement for non-Join types
                        let api_ident = format_ident!("{}", api_struct_name);
                        return Some(quote! { #api_ident });
                    }
                }
            }
        }
        None
    }
}

/// Context for pass 1: Discovery phase
pub struct DiscoveryContext {
    pub registry: EntityTypeRegistry,
}

impl DiscoveryContext {
    pub fn new() -> Self {
        Self {
            registry: EntityTypeRegistry::new(),
        }
    }

    /// Analyze a struct and register its join field dependencies
    pub fn analyze_entity(&mut self, struct_name: &Ident, api_struct_name: &Ident, fields: &syn::FieldsNamed) {
        let entity_name = struct_name.to_string();
        let api_name = api_struct_name.to_string();

        // Register this entity
        self.registry.register_entity(entity_name.clone(), api_name);

        // Analyze fields for join dependencies
        for field in &fields.named {
            self.analyze_field_for_joins(&entity_name, field);
        }
    }

    /// Analyze a single field for join attributes
    fn analyze_field_for_joins(&mut self, _entity_name: &str, field: &syn::Field) {
        // Check if this field has join attributes
        for attr in &field.attrs {
            if attr.path().is_ident("crudcrate") {
                // Look for join(...) in the attribute
                // For now, we'll register any potential join target types
                // TODO: Implement more sophisticated attribute parsing for discovery
            }
        }
    }
}

/// Context for pass 2: Generation phase
pub struct GenerationContext<'a> {
    pub registry: &'a EntityTypeRegistry,
}

impl<'a> GenerationContext<'a> {
    pub fn new(registry: &'a EntityTypeRegistry) -> Self {
        Self { registry }
    }

    /// Resolve a join field type using the discovered type registry
    pub fn resolve_join_field_type(&self, field_type: &Type) -> Option<TokenStream> {
        self.registry.resolve_join_type(field_type)
    }
}

/// Main two-pass generator
pub struct TwoPassGenerator {
    discovery: DiscoveryContext,
}

impl TwoPassGenerator {
    pub fn new() -> Self {
        Self {
            discovery: DiscoveryContext::new(),
        }
    }

    /// Get mutable reference to discovery context for pass 1
    pub fn discovery_mut(&mut self) -> &mut DiscoveryContext {
        &mut self.discovery
    }

    /// Create generation context for pass 2
    pub fn generation_context(&self) -> GenerationContext {
        GenerationContext::new(&self.discovery.registry)
    }
}

/// Global registry for cross-entity type resolution using thread-safe storage
static GLOBAL_TYPE_REGISTRY: Mutex<Option<EntityTypeRegistry>> = Mutex::new(None);

/// Register an entity in the global registry
pub fn register_entity_globally(entity_name: String, api_struct_name: String) {
    let mut registry = GLOBAL_TYPE_REGISTRY.lock().unwrap();
    if registry.is_none() {
        *registry = Some(EntityTypeRegistry::new());
    }
    if let Some(ref mut reg) = *registry {
        reg.register_entity(entity_name.clone(), api_struct_name.clone());
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

/// Extract the base type name as a string (e.g., "Vehicle" from Vec<Vehicle> or super::vehicle::Model)
pub fn extract_base_type_string(field_type: &Type) -> Option<String> {
    let registry = GLOBAL_TYPE_REGISTRY.lock().unwrap();
    if let Some(ref reg) = *registry {
        reg.extract_base_type(field_type)
    } else {
        None
    }
}

/// Find the API struct name for a given base type (e.g., "Vehicle" -> "Vehicle")
pub fn find_api_struct_name(base_type: &str) -> Option<String> {
    let registry = GLOBAL_TYPE_REGISTRY.lock().unwrap();
    if let Some(ref reg) = *registry {
        // Try multiple resolution strategies
        if let Some(api_name) = reg.entity_to_api.get(base_type) {
            Some(api_name.clone())
        } else if let Some(api_name) = reg.entity_to_api.get(&format!("{}API", base_type)) {
            Some(api_name.clone())
        } else if let Some(api_name) = reg.entity_to_api.get(&reg.pluralize(base_type)) {
            Some(api_name.clone())
        } else {
            // Default to the base type itself
            Some(base_type.to_string())
        }
    } else {
        None
    }
}