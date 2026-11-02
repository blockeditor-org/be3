use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_keeps_a_public_test_module_declared_once() {
    let root = temporary_directory();
    let source = root.join("crates/widget/src");
    fs::create_dir_all(source.join("theme/tests")).unwrap();
    fs::write(source.join("lib.rs"), "mod theme;\n").unwrap();
    let theme = "pub fn value() -> u8 {\n    1\n}\n\n#[cfg(test)]\npub(crate) mod tests;\n";
    fs::write(source.join("theme.rs"), theme).unwrap();
    fs::write(
        source.join("theme/tests.rs"),
        "use super::*;\n\nmod a_test;\n\npub(crate) fn helper() {}\n",
    )
    .unwrap();
    fs::write(
        source.join("theme/tests/a_test.rs"),
        "use super::*;\n\n#[test]\nfn a_test() {\n    helper();\n}\n",
    )
    .unwrap();

    fix(&root, false).unwrap();

    assert_eq!(fs::read_to_string(source.join("theme.rs")).unwrap(), theme);
    assert!(fix(&root, true).is_ok());

    fs::remove_dir_all(root).unwrap();
}
