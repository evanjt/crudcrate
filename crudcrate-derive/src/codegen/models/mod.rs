pub mod create;
pub mod list;
pub mod response;
pub mod shared;
pub mod update;

use crate::attribute_parser::get_crudcrate_bool;
use crate::codegen::joins::get_join_config;

/// Shared field filtering logic for model generation
/// Determines if a field should be included in a specific model type
pub(crate) fn should_include_in_model(field: &syn::Field, model_type: &str) -> bool {
    // Check the model-specific attribute (create_model, update_model, list_model)
    let include_in_model = get_crudcrate_bool(field, model_type).unwrap_or(true);

    // Handle join field exclusion based on model type
    if let Some(join_config) = get_join_config(field) {
        match model_type {
            "create_model" | "update_model" => {
                // Create/Update models: exclude ALL join fields
                return false;
            }
            "list_model" => {
                // List model: only exclude join(one) fields, keep join(all)
                // Exclude if NOT loading in get_all (on_all = false)
                if !join_config.on_all {
                    return false;
                }
            }
            _ => {}
        }
    }

    include_in_model
}
