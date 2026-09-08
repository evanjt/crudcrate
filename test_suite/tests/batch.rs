//! Batch tests, one module per file under `tests/it/`.

#[path = "it/batch_create_partial_test.rs"]
mod batch_create_partial_test;
#[path = "it/batch_create_returning_test.rs"]
mod batch_create_returning_test;
#[path = "it/batch_row_hooks_test.rs"]
mod batch_row_hooks_test;
#[path = "it/duplicate_key_conflict_test.rs"]
mod duplicate_key_conflict_test;
#[path = "it/fk_violation_test.rs"]
mod fk_violation_test;
#[path = "it/lifecycle_transaction_test.rs"]
mod lifecycle_transaction_test;
#[path = "it/partial_success_batch_test.rs"]
mod partial_success_batch_test;
#[path = "it/upsert_registration_test.rs"]
mod upsert_registration_test;
