use super::*;
use beui_core::input::TouchPhase;
use block_editor_plugin::PointerButton;

#[test]
fn losing_focus_ends_an_emulated_touch() {
    let context = beui::context();
    context.set_touch_emulation(true);
    let whole = region(
        Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 30.0)),
        [40, 30],
    );
    let mut input = Input::default();
    input.translate(
        &context,
        &whole,
        &InputEvent::PointerButton {
            button: PointerButton::Primary,
            pressed: true,
            x: 5.0,
            y: 5.0,
        },
    );
    let events = input.translate(&context, &whole, &InputEvent::Focus(false));
    assert!(
        matches!(
            events.as_slice(),
            [
                Event::Touch {
                    phase: TouchPhase::Cancel,
                    ..
                },
                Event::Focus(false)
            ]
        ),
        "{events:?}"
    );
}
