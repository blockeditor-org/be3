use super::*;
use block_editor_beui::beui::{Key, Modifiers};

#[test]
fn resizing_a_raster_layer_opens_a_card_on_its_button() {
    let layer = raster_layer(1, CanvasPoint::default(), 8);
    let mut editor = raster_editor(std::slice::from_ref(&layer));
    paint(&mut editor, layer.id, &[(0, 0)], RED);
    editor.click(format!("infinite-canvas.entity.{}", layer.id));
    editor.run();

    editor.click("infinite-canvas.raster.resize");
    editor.run();
    assert!(editor.shown("infinite-canvas.raster.resizing.confirm"));
    let button = editor.rect_of("infinite-canvas.raster.resize");
    let card = editor.rect_of("infinite-canvas.raster.resizing");
    assert!(
        card.left() <= button.center().x && button.center().x <= card.right(),
        "the card points at the button that opened it: {card:?} at {button:?}"
    );
    editor.snapshot("resizing_a_raster_layer_opens_a_card_on_its_button");

    editor.click("infinite-canvas.raster.resize-width");
    editor.key(
        Key::A,
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    );
    editor.type_text("12");
    editor.key(Key::Enter, Modifiers::NONE);
    editor.run();
    if editor.shown("infinite-canvas.raster.resizing.confirm") {
        editor.click("infinite-canvas.raster.resizing.confirm");
        editor.run();
    }
    let raster = raster_of(&editor, layer.id);
    assert_eq!((raster.width(), raster.height()), (12, 8));
    assert_eq!(
        raster.pixel(2, 0),
        Some(RED),
        "growing from the centre moves the pixels two along"
    );
}
