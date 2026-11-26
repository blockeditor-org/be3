use super::*;
use be_sdk_beui::beui::{Key, Modifiers, PointerButton, vec2};

#[test]
fn escape_returns_the_arrangement_to_its_fit() {
    let displays = laptop_beside(gaming_monitor());
    let mut settings = Harness::in_a_session(displays).on_this_device();
    let before = settings.editor.rect_of("display.arrangement.DP-1");

    let from = settings.editor.rect_of("display.arrangement.eDP-1").center();
    settings.editor.down(from, PointerButton::Primary);
    settings.editor.hover(from + vec2(0.0, 8.0));
    settings.run();
    let lifted = settings.editor.rect_of("display.arrangement.DP-1");
    assert!(
        lifted.width() < before.width() - 1.0,
        "the lift zooms out to fit every place: {lifted:?} from {before:?}"
    );
    settings.editor.hover(from + vec2(0.0, 30.0));
    settings.run();
    assert_eq!(
        settings.editor.rect_of("display.arrangement.DP-1"),
        lifted,
        "and holds that fit while the carry lasts"
    );

    settings.editor.key(Key::Escape, Modifiers::NONE);
    settings.run();
    assert_eq!(
        settings.editor.rect_of("display.arrangement.DP-1"),
        before,
        "put down, the area fits the displays again"
    );
    settings.editor.up(from + vec2(0.0, 30.0), PointerButton::Primary);
    settings.run();
    assert_eq!(settings.here().arrangement().places, Vec::new(), "and nothing moved");
}
