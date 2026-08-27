//! Type introspection utilities for field analysis
//!
//! Provides low-level functions for examining field types, extracting inner types,
//! and resolving target model types for `use_target_models` attribute.

use crate::codegen::type_resolution::{extract_vec_inner_type_ref, is_option_type};

/// Returns true if the field's type is `Option<…>` (including `std::option::Option<…>`).
pub fn field_is_optional(field: &syn::Field) -> bool {
    is_option_type(&field.ty)
}

/// Resolves the target models (Create/Update) for a field with `use_target_models` attribute.
/// Returns (`CreateModel`, `UpdateModel`) types for the target `CRUDResource`.
pub fn resolve_target_models(field_type: &syn::Type) -> Option<(syn::Type, syn::Type)> {
    // Extract the inner type if it's Vec<T>
    let target_type = extract_vec_inner_type_ref(field_type);

    // Convert target type to Create and Update models
    // For example: crate::path::to::models::Entity -> (EntityCreate, EntityUpdate)
    if let syn::Type::Path(type_path) = target_type
        && let Some(last_seg) = type_path.path.segments.last()
    {
        let base_name = &last_seg.ident;

        // Keep the module path but replace the struct name
        let mut create_path = type_path.clone();
        let mut update_path = type_path.clone();

        // Update the last segment to be the Create/Update versions
        if let Some(last_seg_mut) = create_path.path.segments.last_mut() {
            last_seg_mut.ident = quote::format_ident!("{}Create", base_name);
        }
        if let Some(last_seg_mut) = update_path.path.segments.last_mut() {
            last_seg_mut.ident = quote::format_ident!("{}Update", base_name);
        }

        let create_model = syn::Type::Path(create_path);
        let update_model = syn::Type::Path(update_path);

        return Some((create_model, update_model));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::parse_quote;

    #[test]
    fn test_field_is_optional_with_option_type() {
        let field: syn::Field = parse_quote! { pub field: Option<String> };
        assert!(field_is_optional(&field));
    }

    #[test]
    fn test_field_is_optional_with_std_option() {
        let field: syn::Field = parse_quote! { pub field: std::option::Option<i32> };
        assert!(field_is_optional(&field));
    }

    #[test]
    fn test_field_is_optional_with_non_option_type() {
        let field: syn::Field = parse_quote! { pub field: String };
        assert!(!field_is_optional(&field));
    }

    #[test]
    fn test_field_is_optional_with_vec() {
        let field: syn::Field = parse_quote! { pub field: Vec<String> };
        assert!(!field_is_optional(&field));
    }

    #[test]
    fn test_resolve_target_models_simple_type() {
        let field_type: syn::Type = parse_quote! { Entity };
        let result = resolve_target_models(&field_type);

        assert!(result.is_some());
        let (create, update) = result.unwrap();

        // Verify the model names are correct
        let create_str = quote!(#create).to_string();
        let update_str = quote!(#update).to_string();

        assert!(create_str.contains("EntityCreate"));
        assert!(update_str.contains("EntityUpdate"));
    }

    #[test]
    fn test_resolve_target_models_vec_type() {
        let field_type: syn::Type = parse_quote! { Vec<Product> };
        let result = resolve_target_models(&field_type);

        assert!(result.is_some());
        let (create, update) = result.unwrap();

        let create_str = quote!(#create).to_string();
        let update_str = quote!(#update).to_string();

        assert!(create_str.contains("ProductCreate"));
        assert!(update_str.contains("ProductUpdate"));
    }

    #[test]
    fn test_resolve_target_models_with_path() {
        let field_type: syn::Type = parse_quote! { crate::entities::Customer };
        let result = resolve_target_models(&field_type);

        assert!(result.is_some());
        let (create, update) = result.unwrap();

        let create_str = quote!(#create).to_string();
        let update_str = quote!(#update).to_string();

        // Should preserve the path
        assert!(create_str.contains("crate :: entities"));
        assert!(create_str.contains("CustomerCreate"));
        assert!(update_str.contains("CustomerUpdate"));
    }

    #[test]
    fn test_resolve_target_models_invalid_type() {
        // Non-path types should return None
        let field_type: syn::Type = parse_quote! { &str };
        let result = resolve_target_models(&field_type);
        assert!(result.is_none());
    }
}
