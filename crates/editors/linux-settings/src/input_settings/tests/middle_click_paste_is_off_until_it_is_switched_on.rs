use super::*;

const PASTE: &str = "input-settings.middle-click-paste";
const PASTE_RESET: &str = "input-settings.middle-click-paste.reset";

#[test]
fn middle_click_paste_is_off_until_it_is_switched_on() {
    let mut editor = editor(vec![mouse()]);
    editor.set_size(block_editor_beui::beui::Vec2::new(800.0, 900.0));
    editor.run();
    assert!(!content(&editor).root().middle_click_paste());

    editor.click(PASTE);
    editor.run();
    assert_eq!(content(&editor).root().middle_click_paste, Some(true));
    assert!(content(&editor).root().middle_click_paste());
    editor.snapshot("middle_click_paste_is_off_until_it_is_switched_on");

    editor.click(PASTE_RESET);
    editor.run();
    assert_eq!(content(&editor).root().middle_click_paste, None);
}
