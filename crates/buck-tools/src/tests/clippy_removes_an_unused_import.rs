use super::*;

#[test]
fn clippy_removes_an_unused_import() {
    let source = "use std::collections::HashMap;\npub fn f() {}\n";

    let fixed = run_clippy(
        "removes",
        source,
        &[unused_import("crates/demo/lib.rs", 0, 31)],
    );

    assert_eq!(fixed, "pub fn f() {}\n");
}
