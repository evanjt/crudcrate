//! Sorting pagination tests, one module per file under `tests/it/`.

#[path = "it/comprehensive_pagination_test.rs"]
mod comprehensive_pagination_test;
#[path = "it/comprehensive_sorting_test.rs"]
mod comprehensive_sorting_test;
#[path = "it/configurable_limits_test.rs"]
mod configurable_limits_test;
#[path = "it/pagination_content_range_http_coverage_test.rs"]
mod pagination_content_range_http_coverage_test;
#[path = "it/sort_http_coverage_test.rs"]
mod sort_http_coverage_test;
#[path = "it/stable_pagination_test.rs"]
mod stable_pagination_test;
