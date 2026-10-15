use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_reports_a_primitive_used_outside_beui() {
    let root = temporary_directory();
    let source = root.join("crates/widget/src");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("lib.rs"),
        r#"use beui::primitives::Text;
use beui::reactive::*;

#[component]
fn Label() -> NodeId {
    view! { <beui::primitives::Frame /> }
}
"#,
    )
    .unwrap();
    let allowed = root.join("crates/beui-components-styled/src");
    fs::create_dir_all(&allowed).unwrap();
    fs::write(
        allowed.join("lib.rs"),
        "use beui_view::primitives::Frame;\n",
    )
    .unwrap();

    let error = fix(&root, true).unwrap_err().to_string();

    assert!(error.contains("primitive outside beui: crates/widget/src/lib.rs:1"));
    assert!(error.contains("primitive outside beui: crates/widget/src/lib.rs:6"));
    assert!(!error.contains("crates/beui-components-styled"));

    fs::remove_dir_all(root).unwrap();
}
