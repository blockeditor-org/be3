use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_rewrites_a_split_test_files_imports_for_its_parent() {
    let root = temporary_directory();
    let source = root.join("crates/widget/src");
    fs::create_dir_all(source.join("tests")).unwrap();
    fs::write(source.join("lib.rs"), "#[cfg(test)]\nmod tests;\n").unwrap();
    fs::write(
        source.join("tests.rs"),
        "use super::*;\n\nfn helper() {}\n\nmod moved;\n",
    )
    .unwrap();
    // `super` here is tests.rs, and `super::super` the crate root.
    fs::write(
        source.join("tests/moved.rs"),
        "use super::{helper, scratch, helper as aid};\nuse super::super::Widget;\nuse self::local as here;\nuse std::fs;\n\nfn scratch() {}\n\nfn local() {}\n\n#[test]\nfn first() {}\n\n#[test]\nfn second() {}\n",
    )
    .unwrap();

    fix(&root, false).unwrap();

    assert_eq!(
        fs::read_to_string(source.join("tests.rs")).unwrap(),
        "use super::*;\nuse self::helper as aid;\nuse super::Widget;\nuse self::local as here;\nuse std::fs;\n\nfn helper() {}\n\nfn scratch() {}\nfn local() {}\nmod first;\nmod second;\n"
    );
    assert!(fix(&root, true).is_ok());

    fs::remove_dir_all(root).unwrap();
}
