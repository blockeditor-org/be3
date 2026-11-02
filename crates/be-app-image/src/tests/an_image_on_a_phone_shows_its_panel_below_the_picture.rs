use super::*;
use be_sdk_beui::beui::Vec2;

#[test]
fn an_image_on_a_phone_shows_its_panel_below_the_picture() {
    let mut editor = BeuiTest::<ImageRoot>::editing(Uuid::new_v4())
        .in_viewport()
        .with_size(Vec2::new(390.0, 844.0));
    editor.hold(None, ImageFile::from_file("picture.png", png(8, 4)));
    editor.run();
    editor.settle();

    assert!(
        editor.shown("image.picture") && editor.shown("image.replace"),
        "a phone shows the picture with its panel gathered below it"
    );
    let gathered = editor.rect_of("dock.compact");
    assert!(
        editor.rect_of("image.picture").bottom() <= gathered.top() + 1.0
            && gathered.contains_rect(editor.rect_of("image.replace")),
        "the panel sits below the picture"
    );
    editor.snapshot("an_image_on_a_phone_shows_its_panel_below_the_picture");
}
