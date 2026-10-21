use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_accepts_tagged_unsafe() {
    let root = temporary_directory();
    let source = root.join("crates/widget/src");
    fs::create_dir_all(&source).unwrap();
    let content = r#"// unsafe(ffi)
unsafe extern "C" {
    fn abs(value: i32) -> i32;
}
pub fn call() -> i32 {
    // keep this @yeskeep
    // unsafe(ffi)
    unsafe { abs(-1) }
}
"#;
    fs::write(source.join("lib.rs"), content).unwrap();

    fix(&root, true).unwrap();
    fix(&root, false).unwrap();

    assert_eq!(fs::read_to_string(source.join("lib.rs")).unwrap(), content);
    fs::remove_dir_all(root).unwrap();
}
