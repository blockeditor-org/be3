use super::*;

#[test]
fn the_folder_toolbar_is_a_bar_above_the_entries_that_goes_with_the_chrome() {
    let (Fixture { mut editor, .. }, children) = editor(3);
    editor.run();

    let toolbar = editor.rect_of("folder.toolbar");
    let entry = editor.rect_of(&format!("folder.entry.{}", children[0]));
    assert!(
        toolbar.bottom() <= entry.top(),
        "the toolbar sits in its band above the entries"
    );
    editor.snapshot("the_folder_toolbar_is_a_bar_above_the_entries");

    editor.set_chrome(false);
    editor.run();
    assert!(!editor.shown("folder.toolbar"));
    assert!(editor.shown(&format!("folder.entry.{}", children[0])));
}
