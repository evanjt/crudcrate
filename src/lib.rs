pub mod filter;
pub mod index_analysis;
pub mod models;
pub mod pagination;
pub mod routes;
pub mod sort;
pub mod traits;

pub use crudcrate_derive::*; // Export the proc macros
pub use index_analysis::{analyze_indexes_for_resource, display_index_recommendations};
pub use serde_with;
pub use traits::CRUDResource;
