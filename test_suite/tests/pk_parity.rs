//! Pk parity tests, one module per file under `tests/it/`.

#[path = "it/integer_pk_test.rs"]
mod integer_pk_test;
#[path = "it/pk_parity_batch_test.rs"]
mod pk_parity_batch_test;
#[path = "it/pk_parity_crud_test.rs"]
mod pk_parity_crud_test;
#[path = "it/pk_parity_errors_validation_test.rs"]
mod pk_parity_errors_validation_test;
#[path = "it/pk_parity_filter_test.rs"]
mod pk_parity_filter_test;
#[path = "it/pk_parity_joins_test.rs"]
mod pk_parity_joins_test;
#[path = "it/pk_parity_scope_test.rs"]
mod pk_parity_scope_test;
#[path = "it/pk_parity_sort_pagination_test.rs"]
mod pk_parity_sort_pagination_test;
#[path = "it/pk_parity_string_pk_test.rs"]
mod pk_parity_string_pk_test;
