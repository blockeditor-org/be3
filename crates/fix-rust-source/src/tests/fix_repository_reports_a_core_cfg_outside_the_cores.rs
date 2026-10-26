use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_reports_a_core_cfg_outside_the_cores() {
    let root = temporary_directory();
    let source = root.join("crates/beui-components-unstyled/src");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("lib.rs"),
        r#"use beui_html::dom::mount;

#[cfg(beui_html)]
pub fn web() {}

#[cfg_attr(not(beui_html), allow(dead_code))]
pub fn either() -> bool {
    cfg!(beui_html)
}
"#,
    )
    .unwrap();
    for allowed in ["beui-view", "beui", "beui-core-html"] {
        let elsewhere = root.join("crates").join(allowed).join("src");
        fs::create_dir_all(&elsewhere).unwrap();
        fs::write(
            elsewhere.join("lib.rs"),
            "#[cfg(beui_html)]\npub fn web() {}\n",
        )
        .unwrap();
    }

    let error = fix(&root, true).unwrap_err().to_string();

    assert!(!error.contains("crates/beui-components-unstyled/src/lib.rs:1"));
    for line in [3, 6, 8] {
        assert!(error.contains(&format!(
            "core cfg outside the cores: crates/beui-components-unstyled/src/lib.rs:{line}"
        )));
    }
    assert!(!error.contains("crates/beui-view"));
    assert!(!error.contains("crates/beui/"));
    assert!(!error.contains("crates/beui-core-html"));

    fs::remove_dir_all(root).unwrap();
}
