use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_reports_a_fixed_extent_outside_beui() {
    let root = temporary_directory();
    let source = root.join("crates/widget/src");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("lib.rs"),
        r#"use beui::reactive::*;
use beui::styled::*;

#[component]
fn Thumbnail() -> NodeId {
    view! { <Frame width={Extent::px(48.0)} height={Extent::fraction(1.0)} /> }
}
"#,
    )
    .unwrap();
    let allowed = root.join("crates/beui-components-styled/src");
    fs::create_dir_all(&allowed).unwrap();
    fs::write(
        allowed.join("lib.rs"),
        "const FIXED: Extent = Extent::px(4.0);\n",
    )
    .unwrap();

    let error = fix(&root, true).unwrap_err().to_string();

    assert!(error.contains("fixed extent outside beui: crates/widget/src/lib.rs:6"));
    assert_eq!(error.matches("fixed extent outside beui").count(), 1);

    fs::remove_dir_all(root).unwrap();
}
