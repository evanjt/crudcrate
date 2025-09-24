// Feature Group 5: Multi-Database Optimization  
// Database-specific features and index recommendations

pub mod index_analysis;

// Re-export commonly used items
pub use index_analysis::{analyse_indexes_for_resource, display_index_recommendations, register_analyser, analyse_all_registered_models, ensure_all_analysers_registered};