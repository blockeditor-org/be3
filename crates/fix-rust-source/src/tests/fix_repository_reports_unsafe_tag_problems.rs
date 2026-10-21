use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_reports_unsafe_tag_problems() {
    let root = temporary_directory();
    let source = root.join("crates/widget/src");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("lib.rs"),
        r#"pub fn untagged() {
    unsafe { std::env::remove_var("A") };
}
// unsafe(magic)
pub fn unknown() {}
// unsafe(memory)
pub fn stale() {}
// unsafe(ffi) because it calls C
pub fn prose() {}
"#,
    )
    .unwrap();

    let error = fix(&root, true).unwrap_err().to_string();

    assert!(error.contains("unsafe tag missing: crates/widget/src/lib.rs:2"));
    assert!(error.contains("unsafe tag malformed: crates/widget/src/lib.rs:4"));
    assert!(error.contains("unsafe tag stale: crates/widget/src/lib.rs:6"));
    assert!(error.contains("unsafe tag malformed: crates/widget/src/lib.rs:8"));
    assert!(error.contains("send-sync: unsafe impl Send"));
    assert!(!error.contains("comments: crates/widget/src/lib.rs"));

    fs::remove_dir_all(root).unwrap();
}
