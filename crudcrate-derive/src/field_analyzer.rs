
/// Returns true if the field's type is `Option<…>` (including `std::option::Option<…>`).
pub(crate) fn field_is_optional(field: &syn::Field) -> bool {
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
pub(crate) fn resolve_target_models_with_list(
    field_type: &syn::Type,
) -> Option<(syn::Type, syn::Type, syn::Type)> {
    // Extract the inner type if it's Vec<T>
    let target_type = if let syn::Type::Path(type_path) = field_type {
        if let Some(last_seg) = type_path.path.segments.last() {
            if last_seg.ident == "Vec" {
                if let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                        inner_type
                    } else {
                        field_type
                    }
                } else {
                    field_type
                }
            } else {
                field_type
            }
        } else {
            field_type
        }
    } else {
        field_type
    };

    // Convert target type to Create, Update, and List models
    // For example: crate::routes::treatments::models::Treatment -> (TreatmentCreate, TreatmentUpdate, TreatmentList)
    if let syn::Type::Path(type_path) = target_type
        && let Some(last_seg) = type_path.path.segments.last()
    {
        let base_name = &last_seg.ident;

        // Keep the module path but replace the struct name
        let mut create_path = type_path.clone();
        let mut update_path = type_path.clone();
        let mut list_path = type_path.clone();

        // Update the last segment to be the Create/Update/List versions
        if let Some(last_seg_mut) = create_path.path.segments.last_mut() {
            last_seg_mut.ident = quote::format_ident!("{}Create", base_name);
        }
        if let Some(last_seg_mut) = update_path.path.segments.last_mut() {
            last_seg_mut.ident = quote::format_ident!("{}Update", base_name);
        }
        if let Some(last_seg_mut) = list_path.path.segments.last_mut() {
            last_seg_mut.ident = quote::format_ident!("{}List", base_name);
        }

        let create_model = syn::Type::Path(create_path);
        let update_model = syn::Type::Path(update_path);
        let list_model = syn::Type::Path(list_path);

        return Some((create_model, update_model, list_model));
    }
    None
}

/// Resolves the target models (Create/Update) for a field with `use_target_models` attribute.
/// Returns (`CreateModel`, `UpdateModel`) types for the target `CRUDResource`.
pub(crate) fn resolve_target_models(field_type: &syn::Type) -> Option<(syn::Type, syn::Type)> {
    // Extract the inner type if it's Vec<T>
    let target_type = if let syn::Type::Path(type_path) = field_type {
        if let Some(last_seg) = type_path.path.segments.last() {
            if last_seg.ident == "Vec" {
                if let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                        inner_type
                    } else {
                        field_type
                    }
                } else {
                    field_type
                }
            } else {
                field_type
            }
        } else {
            field_type
        }
    } else {
        field_type
    };

    // Convert target type to Create and Update models
    // For example: crate::routes::treatments::models::Treatment -> (TreatmentCreate, TreatmentUpdate)
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

/// For an update field type like `Option<T>` or `Option<Option<T>>`, extract the inner `T`.
pub(crate) fn extract_inner_type_for_update(ty: &syn::Type) -> syn::Type {
    if let syn::Type::Path(type_path) = ty
        && let Some(last_seg) = type_path.path.segments.last()
        && last_seg.ident == "Option"
        && let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments
        && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
    {
        return inner.clone();
    }
    ty.clone()
}

/// Extract the inner type from Vec<T> or return the type as-is
pub(crate) fn extract_inner_type(field_type: &syn::Type) -> syn::Type {
    if let syn::Type::Path(type_path) = field_type
        && let Some(last_seg) = type_path.path.segments.last()
            && last_seg.ident == "Vec"
                && let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments
                    && let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                        return inner_type.clone();
                    }
    field_type.clone()
}

/// Get the type name as a string for cyclic dependency detection
/// Also handles Box<T> patterns for self-references
pub(crate) fn get_type_name(ty: &syn::Type) -> Option<String> {
    if let syn::Type::Path(type_path) = ty
        && let Some(last_seg) = type_path.path.segments.last() {
            // Handle Box<T> pattern
            if last_seg.ident == "Box"
                && let syn::PathArguments::AngleBracketed(args) = &last_seg.arguments
                    && let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                        return get_type_name(inner_type);
                    }
            return Some(last_seg.ident.to_string());
        }
    None
}

/// Check for potential cyclic dependencies in join fields
/// Returns warnings about potential cycles that don't have explicit depth
pub(crate) fn detect_cyclic_dependencies(
    current_type: &str,
    field_analysis: &super::structs::EntityFieldAnalysis,
) -> Vec<syn::Error> {
    
    let mut warnings = Vec::new();
    
    // Check all join fields for potential cycles 
    for field in &field_analysis.join_on_one_fields.iter().chain(field_analysis.join_on_all_fields.iter()).collect::<Vec<_>>() {
        let join_config = super::attribute_parser::get_join_config(field);
        let inner_type = extract_inner_type(&field.ty);
        
        if let Some(target_type_name) = get_type_name(&inner_type) {
            // If the join field type is the same as the current type, it's a direct cycle
            // Also check for "Model" which is a self-reference in the current struct context
            if (target_type_name == current_type || target_type_name == "Model")
                && join_config.as_ref().is_none_or(|c| !c.has_explicit_depth())
                    && let Some(field_name) = &field.ident {

                        // Build the complete cycle path for better understanding
                        let cycle_path = if target_type_name == "Model" {
                            // Self-reference: Customer -> vehicles -> Model (self) -> Customer
                            format!("{current_type} -> {field_name} -> Model -> {current_type}")
                        } else {
                            // Different type: Customer -> vehicles -> Vehicle -> customer -> Customer
                            format!("{current_type} -> {field_name} -> {target_type_name} -> customer -> {current_type}")
                        };

                        // Try to target the join attribute specifically for better error location
                        let warning = if let Some(crudcrate_attr) = find_crudcrate_join_attr(field) {
                            syn::Error::new_spanned(crudcrate_attr,
                                format!(
                                    "Cyclic dependency detected: {cycle_path}. \
                                    This will cause infinite recursion during join loading. \
                                    To fix this, add the depth parameter to your join() statement: depth = 2"
                                ))
                        } else {
                            syn::Error::new_spanned(field,
                                format!(
                                    "Cyclic dependency detected: {cycle_path}. \
                                    This will cause infinite recursion during join loading. \
                                    To fix this, add the depth parameter to your join() statement: depth = 2"
                                ))
                        };

                        warnings.push(warning);
                    }
            
            // TODO: For more complex cycle detection (A -> B -> A), we'd need to analyze
            // multiple entities together, which would require a different approach
            // For now, we focus on direct self-references
        }
    }
    
    warnings
}

/// Try to find the crudcrate join attribute for better error span targeting
pub fn find_crudcrate_join_attr(field: &syn::Field) -> Option<&syn::Attribute> {
    for attr in &field.attrs {
        if attr.path().is_ident("crudcrate") {
            // Check if this attribute contains join configuration
            if let Ok(meta) = attr.meta.require_list() {
                let attr_str = meta.tokens.to_string();
                if attr_str.contains("join") {
                    return Some(attr);
                }
            }
        }
    }
    None
}


