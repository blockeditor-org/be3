use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_reports_document_use_in_a_component() {
    let root = temporary_directory();
    let source = root.join("crates/beui-components-unstyled/src");
    fs::create_dir_all(source.join("tests")).unwrap();
    fs::write(
        source.join("lib.rs"),
        r#"use beui_core::document::Document;
use text_editor_core::Document as Text;

pub fn open(tree: &impl Inspect, node: NodeId) -> bool {
    with_document(|document| document.is_overlay_open(node))
}
"#,
    )
    .unwrap();
    fs::write(
        source.join("tests/reads.rs"),
        "use beui_core::document::Document;\n",
    )
    .unwrap();
    let elsewhere = root.join("crates/beui-view/src");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(
        elsewhere.join("lib.rs"),
        "use beui_core::document::Document;\n",
    )
    .unwrap();

    let error = fix(&root, true).unwrap_err().to_string();

    assert!(
        error.contains("document in a component: crates/beui-components-unstyled/src/lib.rs:1")
    );
    assert!(!error.contains("crates/beui-components-unstyled/src/lib.rs:2"));
    assert!(
        error.contains("document in a component: crates/beui-components-unstyled/src/lib.rs:5")
    );
    assert!(!error.contains("tests/reads.rs"));
    assert!(!error.contains("crates/beui-view"));

    fs::remove_dir_all(root).unwrap();
}
