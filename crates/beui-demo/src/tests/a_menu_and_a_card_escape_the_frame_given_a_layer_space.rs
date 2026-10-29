use super::*;
use beui::{HostTransform, LayerSpace, PointerButton, Rect, WindowId};

const FRAME: Vec2 = Vec2::new(640.0, 420.0);
const SPACE: LayerSpace = LayerSpace {
    to_host: HostTransform {
        origin: pos2(80.0, 60.0),
        scale: Vec2::splat(1.0),
    },
    bounds: Rect::from_min_max(pos2(0.0, 0.0), pos2(960.0, 720.0)),
};

#[test]
fn a_menu_and_a_card_escape_the_frame_given_a_layer_space() {
    let mut test = alone(FRAME, Page::Menus);
    test.context().set_layer_space(WindowId::MAIN, Some(SPACE));
    test.frame(Vec::new());
    reveal(&mut test, "demo.actions");
    let card = test.rect_of("demo.actions");
    let at = pos2(
        card.right().min(FRAME.x) - 24.0,
        card.bottom().min(FRAME.y) - 12.0,
    );
    test.click_with(at, PointerButton::Secondary);
    test.settle();
    let layers = test.document().escaped_layers();
    assert_eq!(layers.len(), 1, "the menu escapes the frame");
    let frame = SPACE.to_host.rect(Rect::from_min_size(Pos2::ZERO, FRAME));
    assert!(
        layers[0].rect.bottom() > frame.bottom() || layers[0].rect.right() > frame.right(),
        "the menu reaches past the frame rather than being squeezed into it: {:?}",
        layers[0].rect
    );
    test.snapshot_layers("a_menu_escapes_the_frame_given_a_layer_space");
    press_key(&mut test, beui::Key::Escape);
    test.settle();
    assert!(
        test.document().escaped_layers().is_empty(),
        "Escape closes it"
    );

    let mut test = alone(FRAME, Page::Overlays);
    test.context().set_layer_space(WindowId::MAIN, Some(SPACE));
    test.frame(Vec::new());
    reveal(&mut test, "demo.rename.open");
    let button = test.rect_of("demo.rename.open");
    test.click("demo.rename.open");
    test.settle();
    let field = test.rect_of("demo.rename.field");
    assert!(
        field.top() > button.bottom(),
        "the rename card hangs from the button that opened it: {field:?} under {button:?}"
    );
    assert_eq!(
        test.document().escaped_layers().len(),
        1,
        "and escapes the frame"
    );
    test.snapshot_layers("a_card_from_a_button_escapes_the_frame_given_a_layer_space");

    let mut test = alone(FRAME, Page::Menus);
    let zoomed = LayerSpace {
        to_host: HostTransform {
            origin: pos2(80.0, 60.0),
            scale: Vec2::splat(0.5),
        },
        ..SPACE
    };
    test.context().set_layer_space(WindowId::MAIN, Some(zoomed));
    test.frame(Vec::new());
    reveal(&mut test, "demo.actions");
    let card = test.rect_of("demo.actions");
    test.click_with(card.center(), PointerButton::Secondary);
    test.settle();
    let layers = test.document().escaped_layers();
    assert_eq!(layers.len(), 1);
    let card = zoomed.to_host.rect(card);
    assert!(
        layers[0].rect.width() > card.width() / 2.0,
        "the menu of a frame shown at half size is drawn at full size: {:?} beside {card:?}",
        layers[0].rect
    );
    test.snapshot_layers("a_menu_escapes_a_zoomed_out_frame_at_full_size");
}
