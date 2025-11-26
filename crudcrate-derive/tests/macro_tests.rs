//! Proc-macro testing using trybuild
//!
//! Tests are organized into:
//! - ui-fail: Tests that invalid macro usage produces helpful error messages
//! - ui-pass: Tests that valid macro usage compiles and runs correctly
//!
//! Each ui-pass test verifies a specific feature:
//! - basic_entity.rs: Basic EntityToModels derive
//! - entity_with_hooks.rs: Lifecycle hooks
//! - join_filter_sort.rs: filterable/sortable inside join() attribute
//! - field_exclusion.rs: exclude(create, update, one, list) attributes

/// Tests that invalid macro usage produces helpful error messages
#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui-fail/*.rs");
}

/// Tests that valid macro usage compiles and runs successfully
///
/// Each test file in ui-pass/ tests a specific feature and includes
/// runtime assertions to verify the generated code behaves correctly.
#[test]
fn compile_pass_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui-pass/*.rs");
}
