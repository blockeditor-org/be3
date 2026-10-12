use super::*;

#[test]
fn clippy_applies_no_fix_to_a_crate_that_failed_to_compile() {
    let source = "use std::collections::HashMap;\npub fn f() { view! { <HashMap } }\n";
    let macro_error = json!({
        "level": "error",
        "code": null,
        "message": "expected `>`",
        "rendered": "error: expected `>`",
        "spans": [{"file_name": "crates/demo/lib.rs", "byte_start": 52, "byte_end": 60}],
        "children": [],
    });

    let fixed = run_clippy(
        "failed",
        source,
        &[macro_error, unused_import("crates/demo/lib.rs", 0, 31)],
    );

    assert_eq!(fixed, source);
}
