//! Type introspection utilities for field analysis
//!
//! Provides low-level functions for examining field types, extracting inner types,
//! and resolving target model types for `use_target_models` attribute.

use crate::codegen::type_resolution::extract_vec_inner_type_ref;

/// Returns true if the field's type is `Option<…>` (including `std::option::Option<…>`).
pub fn field_is_optional(field: &syn::Field) -> bool {
    if let syn::Type::Path(type_path) = &field.ty {
        // Look at the *last* segment in the path to see if its identifier is "Option"
        if let Some(last_seg) = type_path.path.segments.last() {
            last_seg.ident == "Option"
        } else {
            false
        }
    } else {
        false
    }
}

/// Resolves the target models (Create/Update/List) for a field with `use_target_models` attribute.
/// Returns (`CreateModel`, `UpdateModel`, `ListModel`) types for the target `CRUDResource`.
/// If you only need Create/Update, call `resolve_target_models()` instead.
pub fn resolve_target_models_with_list(
    field_type: &syn::Type,
) -> Option<(syn::Type, syn::Type, syn::Type)> {
    if let Some((create_model, update_model)) = resolve_target_models(field_type) {
        // Extract the target type path to create the List model
        let target_type = extract_vec_inner_type_ref(field_type);
        if let syn::Type::Path(type_path) = target_type
            && let Some(last_seg) = type_path.path.segments.last()
        {
            let base_name = &last_seg.ident;
            let mut list_path = type_path.clone();

            if let Some(last_seg_mut) = list_path.path.segments.last_mut() {
                last_seg_mut.ident = quote::format_ident!("{}List", base_name);
            }

            let list_model = syn::Type::Path(list_path);
            return Some((create_model, update_model, list_model));
        }
    }
    None
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
