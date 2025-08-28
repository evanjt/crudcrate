use super::structs::EntityFieldAnalysis;
use super::attribute_parser::field_has_crudcrate_flag;

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

/// Analyzes entity fields and creates the EntityFieldAnalysis structure.
/// This processes all fields and categorizes them based on their attributes.
pub(crate) fn analyze_entity_fields(fields: &[syn::Field]) -> EntityFieldAnalysis<'_> {
    let mut primary_key_field = None;
    let mut sortable_fields = Vec::new();
    let mut filterable_fields = Vec::new();
    let mut fulltext_fields = Vec::new();

    // Separate database and non-database fields
    let (db_fields, non_db_fields): (Vec<&syn::Field>, Vec<&syn::Field>) = fields
        .iter()
        .partition(|field| {
            // Non-database fields have #[crudcrate(non_db_attr = true)]
            super::attribute_parser::get_crudcrate_bool(field, "non_db_attr").unwrap_or(false) == false
        });

    for field in &db_fields {
        // Check for primary key
        if field_has_crudcrate_flag(field, "primary_key") {
            primary_key_field = Some(*field);
        }

        // Check for sortable
        if field_has_crudcrate_flag(field, "sortable") {
            sortable_fields.push(*field);
        }

        // Check for filterable
        if field_has_crudcrate_flag(field, "filterable") {
            filterable_fields.push(*field);
        }

        // Check for fulltext
        if field_has_crudcrate_flag(field, "fulltext") {
            fulltext_fields.push(*field);
        }
    }

    EntityFieldAnalysis {
        db_fields,
        non_db_fields,
        primary_key_field,
        sortable_fields,
        filterable_fields,
        fulltext_fields,
    }
}

/// Validates the field analysis to ensure it meets requirements.
/// Returns an error if validation fails (e.g., no primary key found).
pub(crate) fn validate_field_analysis(analysis: &EntityFieldAnalysis<'_>) -> Result<(), proc_macro::TokenStream> {
    // Ensure we have a primary key
    if analysis.primary_key_field.is_none() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "EntityToModels requires exactly one field marked with #[crudcrate(primary_key)]",
        )
        .to_compile_error()
        .into());
    }

    Ok(())
}