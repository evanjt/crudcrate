// Feature Group 3: Advanced Filtering & Search
// Query parameter handling, fulltext search, SQL condition building

pub mod query_parser;
pub mod search;
pub mod conditions;
pub mod sort;
pub mod pagination;

// Re-export commonly used items  
pub use query_parser::FilterOptions;
pub use conditions::{apply_filters, parse_pagination, parse_range};
pub use search::build_fulltext_condition;
pub use sort::parse_sorting;
pub use pagination::calculate_content_range;