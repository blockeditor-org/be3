use super::*;

use crate::host::{SurfaceOutput, Ui};

const CHILD: Target = Target {
    instance: EditorInstanceId(2),
    region: REGION,
};

fn press_on_child(at: Pos2, passive: bool) {
    let rect = Rect::from_min_size(pos2(10.0, 10.0), SIZE);
    host::register(TARGET, rect, Rect::EVERYTHING, 0);
    let mut output = SurfaceOutput::default();
    let mut ui = Ui::new(&mut output, rect, Rect::EVERYTHING, 0);
    let child = Rect::from_min_size(pos2(40.0, 40.0), vec2(40.0, 40.0));
    ui.passive(passive).register(CHILD, child);
    host::test_frame(
        vec![
            beui::Event::PointerMoved(at),
            beui::Event::PointerButton {
                pos: at,
                button: beui::PointerButton::Primary,
                pressed: true,
                modifiers: beui::Modifiers::NONE,
            },
        ],
        Some(at),
        true,
    );
}

fn pressed(messages: &[Message]) -> bool {
    messages.iter().any(|message| match message {
        Message::Input(batch) => batch
            .events
            .iter()
            .any(|event| matches!(event, InputEvent::PointerButton { pressed: true, .. })),
        _ => false,
    })
}

#[test]
fn a_press_on_a_passive_child_reaches_the_editor_holding_it() {
    let mut instances = placed();
    let screens = instances.next_screens(PASS).screens;
    instances.screen_set(screens);

    press_on_child(pos2(60.0, 60.0), false);
    let messages = instances.frame_input(PASS, &FrameOverlay::default());
    assert!(!pressed(&messages), "a child that takes input keeps the press");

    press_on_child(pos2(60.0, 60.0), true);
    let messages = instances.frame_input(PASS, &FrameOverlay::default());
    assert!(pressed(&messages), "a passive child lets the press through");
}
