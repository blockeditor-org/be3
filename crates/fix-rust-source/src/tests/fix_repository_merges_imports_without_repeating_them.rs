use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_merges_imports_without_repeating_them() {
    let root = temporary_directory();
    let source = root.join("crates/widget/src");
    fs::create_dir_all(source.join("tests")).unwrap();
    fs::write(source.join("lib.rs"), "#[cfg(test)]\nmod tests;\n").unwrap();
    fs::write(
        source.join("tests.rs"),
        "use super::*;\nuse std::collections::{BTreeMap, HashMap};\nuse std::fs;\n\nfn helper() {}\n\nmod moved;\n",
    )
    .unwrap();
    fs::write(
        source.join("tests/moved.rs"),
        "use super::*;\nuse std::collections::HashMap;\nuse std::fs;\nuse std::io::{Read, Write};\nuse std::path::{Path, PathBuf};\n\nfn helper() {}\n\n#[test]\nfn first() {}\n\n#[test]\nfn second() {}\n",
    )
    .unwrap();

    fix(&root, false).unwrap();

    assert_eq!(
        fs::read_to_string(source.join("tests.rs")).unwrap(),
        "use super::*;\nuse std::collections::{BTreeMap, HashMap};\nuse std::fs;\nuse std::io::{Read, Write};\nuse std::path::{Path, PathBuf};\n\nfn helper() {}\nmod first;\nmod second;\n"
    );
    assert!(fix(&root, true).is_ok());

    fs::write(
        source.join("tests/third.rs"),
        "use super::*;\n\nfn helper() -> u8 {\n    1\n}\n\n#[test]\nfn third() {}\n\n#[test]\nfn fourth() {}\n",
    )
    .unwrap();
    let error = fix(&root, false).unwrap_err().to_string();
    assert!(error.contains("cannot move helper into"), "{error}");

    fs::remove_dir_all(root).unwrap();
}
