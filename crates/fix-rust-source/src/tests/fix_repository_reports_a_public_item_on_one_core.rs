use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_reports_a_public_item_on_one_core() {
    let root = temporary_directory();
    let view = root.join("crates/beui-view/src");
    fs::create_dir_all(view.join("native")).unwrap();
    fs::write(
        view.join("lib.rs"),
        r#"#[cfg(not(beui_html))]
mod native;

#[cfg(beui_html)]
pub fn only_html() {}

#[cfg(not(beui_html))]
pub(crate) fn private_to_native() {}

#[cfg(beui_html)]
pub mod running {
    pub fn run() {}
}

#[cfg(not(beui_html))]
pub mod chosen {
    pub fn inner() {}
}

pub fn shared() {}
"#,
    )
    .unwrap();
    fs::write(
        view.join("native/text.rs"),
        "pub struct Leaked;\n\npub(crate) fn body() {}\n\nimpl Leaked {\n    pub fn method(&self) {}\n}\n",
    )
    .unwrap();
    let facade = root.join("crates/beui/src");
    fs::create_dir_all(&facade).unwrap();
    fs::write(
        facade.join("lib.rs"),
        "#[cfg(not(beui_html))]\npub mod native {\n    pub fn build() {}\n}\n\n#[cfg(not(beui_html))]\npub use other::App;\n",
    )
    .unwrap();

    let error = fix(&root, true).unwrap_err().to_string();

    for line in [4, 15, 17] {
        assert!(
            error.contains(&format!(
                "one-core public item: crates/beui-view/src/lib.rs:{line}"
            )),
            "{error}"
        );
    }
    for line in [7, 10, 12, 20] {
        assert!(!error.contains(&format!("crates/beui-view/src/lib.rs:{line} ")));
    }
    assert!(error.contains("public item in a core: crates/beui-view/src/native/text.rs:1"));
    assert!(!error.contains("crates/beui-view/src/native/text.rs:3"));
    assert!(!error.contains("crates/beui-view/src/native/text.rs:6"));
    assert!(error.contains("one-core public item: crates/beui/src/lib.rs:6"));
    assert!(!error.contains("crates/beui/src/lib.rs:3"));

    fs::remove_dir_all(root).unwrap();
}
