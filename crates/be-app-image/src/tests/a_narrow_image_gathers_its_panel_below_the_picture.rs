use super::*;
use be_sdk_beui::beui::Vec2;
use be_type_image::ImageFile;

#[test]
fn a_narrow_image_gathers_its_panel_below_the_picture() {
    let block = Uuid::new_v4();
    let mut editor = BeuiTest::<ImageRoot>::editing(block).with_size(Vec2::new(390.0, 760.0));
    editor.hold(None, ImageFile::from_file("picture.png", png(8, 4)));
    editor.run();
    editor.settle();

    assert!(
        editor.shown("image.picture") && editor.shown("image.replace"),
        "a narrow editor shows the picture with its panel gathered below it"
    );
    let gathered = editor.rect_of("dock.compact");
    assert!(
        editor.rect_of("image.picture").bottom() <= gathered.top() + 1.0
            && gathered.contains_rect(editor.rect_of("image.replace")),
        "the panel sits below the picture"
    );
    editor.snapshot("a_narrow_image_gathers_its_panel_below_the_picture");
}
