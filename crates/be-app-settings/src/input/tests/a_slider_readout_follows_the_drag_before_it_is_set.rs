use super::*;
use be_sdk_beui::beui::{PointerButton, Vec2};

#[test]
fn a_slider_readout_follows_the_drag_before_it_is_set() {
    let host = EditorHost::default();
    host.set_watched::<InputReachWatch>(&InputReach {
        keyboard: true,
        middle_click_paste: true,
    });
    let mut editor = open(host);
    editor.set_size(Vec2::new(800.0, 900.0));
    editor.run();
    let stored = content(&editor).root().repeat_delay();
    let resting = editor.label("input.repeat-delay.value");

    let slider = editor.rect_of("input.repeat-delay");
    let from = slider.center() - Vec2::new(slider.width() / 2.0 - 4.0, 0.0);
    editor.down(from, PointerButton::Primary);
    editor.hover(slider.center());
    editor.hover(slider.center() + Vec2::new(slider.width() / 4.0, 0.0));
    let dragged = editor.label("input.repeat-delay.value");
    assert_ne!(dragged, resting, "the readout follows the drag, as the slider does");
    assert_eq!(
        content(&editor).root().repeat_delay(),
        stored,
        "nothing is set until the drag is let go"
    );

    editor.up(
        slider.center() + Vec2::new(slider.width() / 4.0, 0.0),
        PointerButton::Primary,
    );
    assert_ne!(content(&editor).root().repeat_delay(), stored);
    assert_eq!(
        editor.label("input.repeat-delay.value"),
        dragged,
        "the release sets what the readout showed"
    );
}
