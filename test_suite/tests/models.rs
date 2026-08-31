//! Models tests, one module per file under `tests/it/`.

#[path = "it/axum_extras_router_test.rs"]
mod axum_extras_router_test;
#[path = "it/crud_operations_default_coverage_test.rs"]
mod crud_operations_default_coverage_test;
#[path = "it/dense_entity_format_test.rs"]
mod dense_entity_format_test;
#[path = "it/deny_unknown_fields_test.rs"]
mod deny_unknown_fields_test;
#[path = "it/exclude_combinations_coverage_test.rs"]
mod exclude_combinations_coverage_test;
#[path = "it/exclude_functionality_test.rs"]
mod exclude_functionality_test;
#[path = "it/field_types_roundtrip_coverage_test.rs"]
mod field_types_roundtrip_coverage_test;
#[path = "it/generated_paths_pin_test.rs"]
mod generated_paths_pin_test;
#[path = "it/list_model_serde_attrs_test.rs"]
mod list_model_serde_attrs_test;
#[path = "it/list_model_test.rs"]
mod list_model_test;
#[path = "it/multi_database_test.rs"]
mod multi_database_test;
#[path = "it/postgres_array_bind_test.rs"]
mod postgres_array_bind_test;
#[path = "it/postgres_native_enum_test.rs"]
mod postgres_native_enum_test;
#[path = "it/standalone_derive_test.rs"]
mod standalone_derive_test;
#[path = "it/trait_defaults_coverage_test.rs"]
mod trait_defaults_coverage_test;
#[path = "it/validatable_auto_test.rs"]
mod validatable_auto_test;
#[path = "it/validation_test.rs"]
mod validation_test;
