use super::field_analyzer::field_is_optional;
use super::structs::EntityFieldAnalysis;
use convert_case::{Case, Casing};
use quote::{format_ident, quote};

/// Helper function to determine if a type is Vec<T>
fn is_vec_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Vec";
        }
    false
}

/// Generates join loading statements for direct queries (all join fields regardless of one/all flags)
pub fn generate_join_loading_for_direct_query(analysis: &EntityFieldAnalysis) -> Vec<proc_macro2::TokenStream> {
    let mut statements = Vec::new();
    
    // Process all join fields (both 'one' and 'all' flags)
    let all_join_fields: Vec<_> = analysis.join_on_one_fields
        .iter()
        .chain(analysis.join_on_all_fields.iter())
        .collect();
    
    for field in all_join_fields {
        if let Some(field_name) = &field.ident {
            // Check if this is a Vec<T> field or a single T field by analyzing the type
            let is_vec_field = is_vec_type(&field.ty);
            
            if is_vec_field {
                // Generate code for Vec<T> fields (has_many relationships)
                let loading_stmt = quote! {
                    // Load related entities for #field_name field
                    let related_models = model.find_related(crate::Relation::#field_name).all(db).await.unwrap_or_default();
                    let related_with_joins: Vec<_> = related_models.into_iter().map(|related_model| {
                        related_model.into() // Convert to API struct
                    }).collect();
                    result.#field_name = related_with_joins;
                };
                statements.push(loading_stmt);
            } else {
                // Generate code for single T or Option<T> fields (belongs_to/has_one relationships)
                let is_optional = field_is_optional(field);
                if is_optional {
                    let loading_stmt = quote! {
                        // Load related entity for #field_name field (Option<T>)
                        if let Ok(Some(related_model)) = model.find_related(crate::Relation::#field_name).one(db).await {
                            result.#field_name = Some(related_model.into());
                        }
                    };
                    statements.push(loading_stmt);
                } else {
                    let loading_stmt = quote! {
                        // Load related entity for #field_name field (T)
                        if let Ok(Some(related_model)) = model.find_related(crate::Relation::#field_name).one(db).await {
                            result.#field_name = related_model.into();
                        }
                    };
                    statements.push(loading_stmt);
                }
            }
        }
    }
    
    statements
}

/// Generate single-level join loading implementation (no complex recursion for now)
pub fn generate_recursive_loading_implementation(analysis: &EntityFieldAnalysis) -> proc_macro2::TokenStream {
    // Check if there are any join fields at all
    if analysis.join_on_one_fields.is_empty() && analysis.join_on_all_fields.is_empty() {
        return quote! {
            Ok(model.into())
        };
    }
    
    // Generate generic single-level loading for all join fields
    let join_loading_statements = generate_join_loading_for_direct_query(analysis);
    
    quote! {
        // Convert the base model to the API struct first
        let mut result: Self = model.into();
        
        // Load all related entities for join fields  
        #(#join_loading_statements)*
        
        Ok(result)
    }
}

/// Extract Vec inner type for relationship analysis
pub fn extract_vec_inner_type(ty: &syn::Type) -> proc_macro2::TokenStream {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Vec" {
                if let syn::PathArguments::AngleBracketed(angle_bracketed) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_type)) = angle_bracketed.args.first() {
                        return quote! { #inner_type };
                    }
                }
            }
        }
    }
    quote! { () }
}

/// Extract Option or direct inner type for relationship analysis
pub fn extract_option_or_direct_inner_type(ty: &syn::Type) -> proc_macro2::TokenStream {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(angle_bracketed) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_type)) = angle_bracketed.args.first() {
                        return quote! { #inner_type };
                    }
                }
            }
        }
    }
    // Not an Option, return the type directly
    quote! { #ty }
}

/// Generate join loading statements for all join(all) fields with recursive depth support
pub fn generate_join_loading_for_all_fields(
    join_fields: &[&syn::Field], 
    join_configs: &std::collections::HashMap<&syn::Field, crate::attribute_parser::JoinConfig>
) -> proc_macro2::TokenStream {
    let mut loading_statements = Vec::new();
    
    for field in join_fields {
        if let Some(field_name) = &field.ident {
            let _config = join_configs.get(field).unwrap_or(&Default::default());
            let is_vec_field = is_vec_type(&field.ty);
            
            if is_vec_field {
                let loading_stmt = quote! {
                    // Load related entities for #field_name field (Vec<T>)
                    let related_models = loaded_model.find_related(crate::Relation::#field_name).all(db).await.unwrap_or_default();
                    let related_api_models: Vec<_> = related_models.into_iter().map(|m| m.into()).collect();
                    loaded_model.#field_name = related_api_models;
                };
                loading_statements.push(loading_stmt);
            } else {
                let loading_stmt = quote! {
                    // Load related entity for #field_name field (single/Option<T>)
                    if let Ok(Some(related_model)) = loaded_model.find_related(crate::Relation::#field_name).one(db).await {
                        loaded_model.#field_name = Some(related_model.into());
                    }
                };
                loading_statements.push(loading_stmt);
            }
        }
    }
    
    quote! {
        #(#loading_statements)*
    }
}

/// Get target type from field for relationship analysis
pub fn get_target_type_from_field(field_type: &syn::Type) -> proc_macro2::TokenStream {
    // Extract inner type from Vec<T>, Option<T>, or T
    if let syn::Type::Path(type_path) = field_type {
        if let Some(segment) = type_path.path.segments.last() {
            match segment.ident.to_string().as_str() {
                "Vec" => {
                    if let syn::PathArguments::AngleBracketed(angle_bracketed) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_type)) = angle_bracketed.args.first() {
                            return quote! { #inner_type };
                        }
                    }
                }
                "Option" => {
                    if let syn::PathArguments::AngleBracketed(angle_bracketed) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_type)) = angle_bracketed.args.first() {
                            return quote! { #inner_type };
                        }
                    }
                }
                _ => return quote! { #field_type },
            }
        }
    }
    quote! { #field_type }
}

/// Get entity path from field type for relationship loading
pub fn get_entity_path_from_field_type(field_type: &syn::Type) -> proc_macro2::TokenStream {
    let target_type = get_target_type_from_field(field_type);
    
    // Assume the entity is in a module with the same name as the type (lowercase)
    // This is a common pattern: Customer type -> customer::Entity
    if let syn::Type::Path(type_path) = field_type {
        if let Some(segment) = type_path.path.segments.last() {
            let type_name = segment.ident.to_string();
            let module_name = type_name.to_case(Case::Snake);
            let module_ident = format_ident!("{}", module_name);
            return quote! { super::#module_ident::Entity };
        }
    }
    
    quote! { super::Entity }
}