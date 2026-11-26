use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_writes_schema_lockfiles() {
    let root = temporary_directory();
    let source = root.join("crates/widget/src");
    fs::create_dir_all(&source).unwrap();
    let lock = root.join("crates/widget/schema.lock");
    fs::write(
        source.join("lib.rs"),
        "#[derive(Schema)]\npub struct Point {\n    pub x: i32,\n    pub y: i32,\n}\n",
    )
    .unwrap();

    let stale = fix(&root, true).unwrap_err().to_string();
    assert!(
        stale.contains("schema lockfile: crates/widget/schema.lock"),
        "{stale}"
    );
    assert!(!lock.exists());
    fix(&root, false).unwrap();
    let written = fs::read_to_string(&lock).unwrap();
    assert!(
        written.contains("struct Point\n  1 x: i32\n  2 y: i32\n"),
        "{written}"
    );
    fix(&root, true).unwrap();

    // Freezing it, then removing a field without retiring it, is refused.
    fs::write(
        source.join("lib.rs"),
        "be_wasm_schema::frozen!();\n\n#[derive(Schema)]\npub struct Point {\n    pub x: i32,\n    pub y: i32,\n}\n",
    )
    .unwrap();
    fix(&root, false).unwrap();
    assert!(
        fs::read_to_string(&lock)
            .unwrap()
            .contains("struct Point (frozen from 3)")
    );
    fs::write(
        source.join("lib.rs"),
        "be_wasm_schema::frozen!();\n\n#[derive(Schema)]\npub struct Point {\n    pub x: i32,\n}\n",
    )
    .unwrap();
    let refused = fix(&root, false).unwrap_err().to_string();
    assert!(
        refused.contains("Point.y is frozen and no longer in the source"),
        "{refused}"
    );

    // A crate whose Schema types are all gone loses its lockfile, unless one was frozen.
    fs::write(source.join("lib.rs"), "pub fn nothing() {}\n").unwrap();
    assert!(fix(&root, false).is_err());
    fs::write(
        &lock,
        fs::read_to_string(&lock)
            .unwrap()
            .replace(" (frozen from 3)", ""),
    )
    .unwrap();
    fix(&root, false).unwrap();
    assert!(!lock.exists());

    fs::remove_dir_all(root).unwrap();
}
