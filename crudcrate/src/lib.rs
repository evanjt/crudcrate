pub mod filter;
pub mod index_analysis;
pub mod models;
pub mod pagination;
pub mod routes;
pub mod sort;
pub mod traits;

pub use crudcrate_derive::*; // Export the proc macros
pub use index_analysis::{analyse_indexes_for_resource, display_index_recommendations, register_analyser, analyse_all_registered_models, ensure_all_analysers_registered};

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
pub use traits::CRUDResource;
