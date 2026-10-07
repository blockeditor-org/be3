use super::*;

#[test]
fn switching_on_natural_scrolling_stores_it() {
    let mut editor = editor();
    assert!(!content(&editor).root().natural_scroll);

    editor.click("input-settings.natural-scroll");
    editor.run();

    assert!(content(&editor).root().natural_scroll);
    editor.snapshot("switching_on_natural_scrolling_stores_it");
}
