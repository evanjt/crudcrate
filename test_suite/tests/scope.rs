//! Scope tests, one module per file under `tests/it/`.

#[path = "it/get_one_scoped_error_test.rs"]
mod get_one_scoped_error_test;
#[path = "it/require_scope_enforcement_test.rs"]
mod require_scope_enforcement_test;
#[path = "it/require_scope_test.rs"]
mod require_scope_test;
#[path = "it/scope_non_db_exclusion_test.rs"]
mod scope_non_db_exclusion_test;
#[path = "it/scope_security_test.rs"]
mod scope_security_test;
#[path = "it/scope_sql_filtering_test.rs"]
mod scope_sql_filtering_test;
#[path = "it/scoped_custom_body_test.rs"]
mod scoped_custom_body_test;
