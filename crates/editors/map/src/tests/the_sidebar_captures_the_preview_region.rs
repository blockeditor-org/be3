use super::*;

#[test]
fn the_sidebar_captures_the_preview_region() {
    let (mut editor, block) = editor();

    editor.click("map.preview-region");
    editor.run();
    editor.run();

    let region = block.read().unwrap().preview_region();
    assert!(region.is_some());
    assert_eq!(displayed_region(&block), region.unwrap());
}
