//! Which fields each generated model carries.

use crate::attrs::get_crudcrate_bool;
use crate::attrs::get_join_config;

/// A field only makes the scoped structs differ from the plain models when it
/// would otherwise appear in the list model. Router wiring and model generation
/// must both use this predicate, or scoped structs are generated but never mounted.
pub(crate) fn is_scoped_exclusion(field: &syn::Field) -> bool {
    !should_include_in_model(field, "scoped_model") && should_include_in_model(field, "list_model")
}

/// Shared field filtering logic for model generation
/// Determines if a field should be included in a specific model type
pub(crate) fn should_include_in_model(field: &syn::Field, model_type: &str) -> bool {
    // Check the model-specific attribute (create_model, update_model, list_model)
    let include_in_model = get_crudcrate_bool(field, model_type).unwrap_or(true);

    // Handle join field exclusion based on model type
    if let Some(join_config) = get_join_config(field).config {
        match model_type {
            "create_model" | "update_model" => {
                // Create/Update models: exclude ALL join fields
                return false;
            }
            "list_model"
                // List model: only exclude join(one) fields, keep join(all)
                // Exclude if NOT loading in get_all (on_all = false)
                if !join_config.on_all => {
                    return false;
                }
            _ => {}
        }
    }

    include_in_model
}
