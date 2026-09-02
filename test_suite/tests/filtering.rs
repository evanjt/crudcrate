//! Filtering tests, one module per file under `tests/it/`.

#[path = "it/comprehensive_filtering_test.rs"]
mod comprehensive_filtering_test;
#[path = "it/comprehensive_fulltext_test.rs"]
mod comprehensive_fulltext_test;
#[path = "it/enum_filtering_test.rs"]
mod enum_filtering_test;
#[path = "it/filter_column_typing_test.rs"]
mod filter_column_typing_test;
#[path = "it/filtering_null_operator_test.rs"]
mod filtering_null_operator_test;
#[path = "it/filtering_operators_http_coverage_test.rs"]
mod filtering_operators_http_coverage_test;
#[path = "it/filtering_sorting_pagination_test.rs"]
mod filtering_sorting_pagination_test;
#[path = "it/filtering_test.rs"]
mod filtering_test;
#[path = "it/fulltext_like_escape_http_test.rs"]
mod fulltext_like_escape_http_test;
