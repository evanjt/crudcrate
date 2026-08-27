//! Security tests, one module per file under `tests/it/`.

#[path = "it/secure_partial_delete_test.rs"]
mod secure_partial_delete_test;
#[path = "it/security_array_filter_cap_test.rs"]
mod security_array_filter_cap_test;
#[path = "it/security_body_bomb_test.rs"]
mod security_body_bomb_test;
#[path = "it/security_delete_enumeration_test.rs"]
mod security_delete_enumeration_test;
#[path = "it/security_invalid_json_test.rs"]
mod security_invalid_json_test;
#[path = "it/security_joined_filter_dos_test.rs"]
mod security_joined_filter_dos_test;
#[path = "it/security_joined_sort_scope_test.rs"]
mod security_joined_sort_scope_test;
#[path = "it/security_option_scope_batch_test.rs"]
mod security_option_scope_batch_test;
#[path = "it/security_profile_per_resource_test.rs"]
mod security_profile_per_resource_test;
#[path = "it/security_scope_side_channel_test.rs"]
mod security_scope_side_channel_test;
#[path = "it/security_test.rs"]
mod security_test;
