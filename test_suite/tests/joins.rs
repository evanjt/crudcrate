//! Joins tests, one module per file under `tests/it/`.

#[path = "it/batch_create_join_shape_test.rs"]
mod batch_create_join_shape_test;
#[path = "it/batch_loading_benchmark_test.rs"]
mod batch_loading_benchmark_test;
#[path = "it/batch_loading_joins_test.rs"]
mod batch_loading_joins_test;
#[path = "it/deep_recursion_test.rs"]
mod deep_recursion_test;
#[path = "it/join_filter_sort_test.rs"]
mod join_filter_sort_test;
#[path = "it/join_functionality_test.rs"]
mod join_functionality_test;
#[path = "it/join_get_all_depth_coverage_test.rs"]
mod join_get_all_depth_coverage_test;
#[path = "it/join_pk_field_name_test.rs"]
mod join_pk_field_name_test;
#[path = "it/join_relation_test.rs"]
mod join_relation_test;
#[path = "it/joined_filter_http_test.rs"]
mod joined_filter_http_test;
#[path = "it/joined_sort_http_test.rs"]
mod joined_sort_http_test;
#[path = "it/nonstandard_fk_join_test.rs"]
mod nonstandard_fk_join_test;
#[path = "it/operations_join_test.rs"]
mod operations_join_test;
#[path = "it/option_belongs_to_join_all_test.rs"]
mod option_belongs_to_join_all_test;
#[path = "it/option_belongs_to_join_test.rs"]
mod option_belongs_to_join_test;
