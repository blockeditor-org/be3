use super::*;

#[test]
fn the_sidebar_goes_away_when_the_host_takes_the_chrome() {
    let (mut test, editor) = editor();

    test.run();
    let with_sidebar = editor.content_rect();

    editor.host().set_chrome_shown(false);
    test.run();

    let without_sidebar = editor.content_rect();
    assert!(with_sidebar.left() > without_sidebar.left());
    assert_eq!(without_sidebar, test.rect());
}
