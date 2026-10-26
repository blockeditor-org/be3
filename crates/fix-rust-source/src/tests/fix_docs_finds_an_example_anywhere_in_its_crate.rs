use super::*;
use crate::fix_docs;
use std::fs;

#[test]
fn fix_docs_finds_an_example_anywhere_in_its_crate() {
    let root = temporary_directory();
    let docs = root.join("docs");
    let code = root.join("code");
    fs::create_dir_all(docs.join("guides")).unwrap();
    fs::create_dir_all(code.join("crates/widget/src/tests")).unwrap();
    fs::create_dir_all(code.join("crates/widget/inner/src")).unwrap();
    fs::write(
        code.join("crates/widget/Cargo.toml"),
        "[package]\nname = \"widget\"\n",
    )
    .unwrap();
    fs::write(
        code.join("crates/widget/src/tests/adds.rs"),
        "fn example_adds() {\n    add(1);\n}\n",
    )
    .unwrap();
    fs::write(
        code.join("crates/widget/inner/Cargo.toml"),
        "[package]\nname = \"inner\"\n",
    )
    .unwrap();
    fs::write(
        code.join("crates/widget/inner/src/lib.rs"),
        "fn example_adds() {\n    other();\n}\n",
    )
    .unwrap();
    let guide = docs.join("guides/widget.md");
    fs::write(&guide, "```rust example=widget.adds\nstale();\n```\n").unwrap();

    fix_docs(&docs, &code).unwrap();

    assert_eq!(
        fs::read_to_string(&guide).unwrap(),
        "```rust example=widget.adds\nadd(1);\n```\n"
    );

    fs::write(&guide, "```rust example=widget.gone\n```\n").unwrap();
    let error = fix_docs(&docs, &code).unwrap_err().to_string();
    assert!(
        error.contains(
            "guides/widget.md:1: example=widget.gone: crate widget has no fn example_gone"
        ),
        "{error}"
    );

    fs::remove_dir_all(root).unwrap();
}
