// Core Feature Groups
pub mod core;
pub mod filtering;
pub mod relationships;
pub mod database;
pub mod dev_experience;

// Legacy modules for backward compatibility (re-export from new structure)
pub mod filter {
    pub use crate::filtering::conditions::*;
}
pub mod models {
    pub use crate::filtering::query_parser::*;
}
pub mod pagination {
    pub use crate::filtering::pagination::*;
}
pub mod routes {
    // Re-export CRUD handlers macros
    pub use crate::core::crud_operations::*;
}
pub mod sort {
    pub use crate::filtering::sort::*;
}
pub mod traits {
    pub use crate::core::traits::*;
}
pub mod index_analysis {
    pub use crate::database::index_analysis::*;
}

// Export procedural macros
pub use crudcrate_derive::*;

// Export commonly used items from feature groups
pub use core::{CRUDResource, MergeIntoActiveModel};
pub use filtering::{FilterOptions, apply_filters, parse_pagination, parse_range, parse_sorting, calculate_content_range};
pub use database::{analyse_indexes_for_resource, display_index_recommendations, register_analyser, analyse_all_registered_models, ensure_all_analysers_registered};

/// Macro to register a CRUD resource for automatic index analysis
/// Usage: `register_crud_analyser!(MyModel)`;
#[macro_export]
macro_rules! register_crud_analyser {
    ($model_type:ty) => {
        $crate::register_analyser::<$model_type>();
    };
    ($($model_type:ty),+ $(,)?) => {
        $(
            $crate::register_analyser::<$model_type>();
        )+
    };
}

pub use serde_with;