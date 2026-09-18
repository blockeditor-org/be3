use super::*;

#[test]
fn the_schema_sidebar_goes_away_with_the_chrome() {
    let (mut test, _client, _block, editor) = editor();

    let with_sidebar = editor.content_rect();

    editor.host().set_chrome_shown(false);
    test.run();

    let without_sidebar = editor.content_rect();
    assert!(with_sidebar.right() < without_sidebar.right());
    assert_eq!(without_sidebar, test.rect());
}
