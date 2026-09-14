use super::*;

#[test]
fn the_sidebar_goes_away_when_the_host_takes_the_chrome() {
    let (mut editor, host) = editor();

    editor.run();
    let with_sidebar = canvas(&mut editor);

    host.set_chrome_shown(false);
    editor.run();

    let without_sidebar = canvas(&mut editor);
    assert!(with_sidebar.left() > without_sidebar.left());
    assert_eq!(without_sidebar, editor.rect());
}
