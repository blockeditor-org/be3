use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_reports_beui_rule_breaks() {
    let root = temporary_directory();
    let source = root.join("crates/widget/src");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("lib.rs"),
        r#"#[component]
fn Rows(keys: Vec<u32>) -> NodeId {
    let total = create_memo(move || {
        count.set(1);
        keys.len()
    });
    view! {
        <List spacing=0.0>
            <ForEach keys>{move |key: u32| view! { <Row key /> }}</ForEach>
        </List>
    }
}
"#,
    )
    .unwrap();

    let error = fix(&root, true).unwrap_err().to_string();

    assert!(error.contains("memo write: crates/widget/src/lib.rs:4"));

    fs::remove_dir_all(root).unwrap();
}
