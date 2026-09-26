use super::*;

#[test]
fn the_schema_sidebar_goes_away_with_the_chrome() {
    let (mut harness, editor) = editor();

    let with_sidebar = editor.content_rect();

    harness.set_chrome(false);

    let without_sidebar = editor.content_rect();
    assert!(with_sidebar.right() < without_sidebar.right());
    assert_eq!(without_sidebar, harness.rect());
}
