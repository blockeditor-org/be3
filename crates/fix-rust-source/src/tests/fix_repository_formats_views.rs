use super::*;
use crate::fix_repository as fix;
use std::fs;

#[test]
fn fix_repository_formats_views() {
    let root = temporary_directory();
    let source = root.join("crates/widget/src");
    fs::create_dir_all(&source).unwrap();
    fs::write(
        source.join("lib.rs"),
        r#"pub fn build() -> NodeId {
    view! {
        <Frame color={surface} outline={border} outline_width=BORDER_WIDTH radius=CARD_RADIUS padding=PADDING>
            {children}
        </Frame>
    }
}
"#,
    )
    .unwrap();

    assert!(fix(&root, true).is_err());
    fix(&root, false).unwrap();

    let formatted = fs::read_to_string(source.join("lib.rs")).unwrap();
    assert!(formatted.contains("        <Frame\n            color={surface}\n"));
    assert!(formatted.lines().all(|line| line.len() <= 100));
    assert!(fix(&root, true).is_ok());

    fs::remove_dir_all(root).unwrap();
}
