use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_keeps_multiline_attributes_with_their_test_modules() {
    let root = temporary_directory();
    let source = root.join("crates/widget/src");
    fs::create_dir_all(source.join("tests")).unwrap();
    fs::write(source.join("lib.rs"), "#[cfg(test)]\nmod tests;\n").unwrap();
    for name in ["a_test", "b_test", "c_test"] {
        fs::write(
            source.join("tests").join(format!("{name}.rs")),
            format!("use super::*;\n\n#[test]\nfn {name}() {{}}\n"),
        )
        .unwrap();
    }
    fs::write(
        source.join("tests.rs"),
        "use super::*;\n\nmod c_test;\n#[cfg(\n    unix\n)]\nmod b_test;\nmod a_test;\n",
    )
    .unwrap();

    fix(&root, false).unwrap();

    assert_eq!(
        fs::read_to_string(source.join("tests.rs")).unwrap(),
        "use super::*;\n\nmod a_test;\nmod c_test;\n#[cfg(\n    unix\n)]\nmod b_test;\n"
    );
    assert!(fix(&root, true).is_ok());

    fs::remove_dir_all(root).unwrap();
}
