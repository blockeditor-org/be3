use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_reports_beui_macros_named_outside_beui_tree() {
    let root = temporary_directory();
    let source = root.join("crates/widget/src");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("lib.rs"),
        r#"use beui::reactive::*;
use beui_macros::component;

#[beui_macros::component]
fn Label() -> NodeId {
    view! { <Frame /> }
}
"#,
    )
    .unwrap();
    let allowed = root.join("crates/beui-tree/src");
    fs::create_dir_all(&allowed).unwrap();
    fs::write(
        allowed.join("lib.rs"),
        "pub mod reactive {\n    pub use beui_macros::{component, sample, view};\n}\n",
    )
    .unwrap();

    let error = fix(&root, true).unwrap_err().to_string();

    assert!(error.contains("beui_macros outside beui-tree: crates/widget/src/lib.rs:2"));
    assert!(error.contains("beui_macros outside beui-tree: crates/widget/src/lib.rs:4"));
    assert!(!error.contains("crates/beui-tree"));

    fs::remove_dir_all(root).unwrap();
}
