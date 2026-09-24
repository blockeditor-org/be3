use super::*;

#[test]
fn the_sidebar_captures_the_preview_region() {
    let mut editor = editor();

    editor.click("map.preview-region");
    editor.run();
    editor.run();

    let region = map(&editor).preview_region;
    assert!(region.is_some());
    assert_eq!(map(&editor).displayed_region(), region.unwrap());
}
