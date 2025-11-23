//! Proc-macro testing using trybuild and macrotest
//!
//! - trybuild: Tests compile-time error messages
//! - macrotest: Tests macro expansion output

/// Tests that invalid macro usage produces helpful error messages
#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui-fail/*.rs");
}

/// Tests that valid macro usage compiles successfully
#[test]
fn compile_pass_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui-pass/*.rs");
}

/// Tests that macro expansion produces expected code
///
/// Run with MACROTEST=overwrite to update .expanded.rs files
#[test]
fn expansion_tests() {
    macrotest::expand("tests/expand/*.rs");
}
