use super::*;
use block_editor_beui::beui::Vec2;

#[test]
fn a_narrow_image_folds_its_sidebar_under_the_picture() {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let mut editor = BeuiTest::<ImageApp>::new(editor).with_size(Vec2::new(390.0, 760.0));
    editor.hold(None, ImageContent::from_file("picture.png", png(8, 4)));
    editor.run();

    assert!(
        !editor.shown("image.replace"),
        "a narrow editor gives the picture the whole width"
    );
    assert!(editor.shown("chrome.sidebar"));
    editor.record();

    editor.click("chrome.sidebar");
    editor.run();
    editor.record();

    assert!(
        editor.shown("image.replace"),
        "the sidebar opens under the picture"
    );
    editor.snapshot("a_narrow_image_folds_its_sidebar_under_the_picture");
}
