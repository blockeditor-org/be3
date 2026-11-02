use super::*;
use be_sdk_beui::beui::Vec2;

#[test]
fn an_image_on_a_phone_on_its_side_shows_its_panel_beside_the_picture() {
    let mut editor = BeuiTest::<ImageRoot>::editing(Uuid::new_v4())
        .in_viewport()
        .with_size(Vec2::new(844.0, 390.0));
    editor.hold(None, ImageFile::from_file("picture.png", png(8, 4)));
    editor.run();
    editor.settle();

    assert!(
        editor.shown("image.picture") && editor.shown("image.replace"),
        "a phone on its side shows the picture with its panel gathered beside it"
    );
    editor.snapshot("an_image_on_a_phone_on_its_side_shows_its_panel_beside_the_picture");
}
