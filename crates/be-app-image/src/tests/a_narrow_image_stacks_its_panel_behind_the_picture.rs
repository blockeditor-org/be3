use super::*;
use be_sdk_beui::beui::Vec2;
use be_type_image::ImageFile;

#[test]
fn a_narrow_image_stacks_its_panel_behind_the_picture() {
    let block = Uuid::new_v4();
    let mut editor = BeuiTest::<ImageRoot>::editing(block).with_size(Vec2::new(390.0, 760.0));
    editor.hold(None, ImageFile::from_file("picture.png", png(8, 4)));
    editor.run();

    assert!(
        editor.shown("image.picture") && !editor.shown("image.replace"),
        "a narrow editor opens on the picture, with the panel a page of its own"
    );
    editor.record();

    editor.click("dock.switch");
    editor.settle();
    editor.click("dock.switcher.tab.panel");
    editor.settle();
    editor.record();

    assert!(
        editor.shown("image.replace"),
        "the switcher brings the panel forward"
    );
    editor.snapshot("a_narrow_image_stacks_its_panel_behind_the_picture");
}
