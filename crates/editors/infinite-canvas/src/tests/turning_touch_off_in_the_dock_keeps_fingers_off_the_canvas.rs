use super::*;
use block_editor_beui::Preferences;
use block_editor_beui::be_block::DisplaySettingsContent;
use block_editor_beui::beui::{CanvasTouch, PenPhase, PointerButton, TouchPhase};

#[test]
fn turning_touch_off_in_the_dock_keeps_fingers_off_the_canvas() {
    let rectangle = card();
    let mut editor = phone(std::slice::from_ref(&rectangle));
    let display = display_settings_here(&mut editor);
    let shape = format!("infinite-canvas.entity.{}", rectangle.id);
    let toggle = "infinite-canvas.dock.ignore-touch";
    let kept = |editor: &BeuiTest<CanvasApp>| {
        editor
            .content::<DisplaySettingsContent>(Some(display))
            .root()
            .canvas_touch
    };

    editor.reveal(toggle);
    let at = editor.point_of(toggle);
    editor.fingers(TouchPhase::Start, &[(1, at)]);
    editor.fingers(TouchPhase::End, &[(1, at)]);
    assert_eq!(
        kept(&editor),
        Some(CanvasTouch::Off),
        "a finger on the dock turns touch off on this device"
    );
    editor.set_preferences(Preferences {
        canvas_touch: CanvasTouch::Off,
        ..Preferences::default()
    });
    editor.run();
    editor.snapshot("touch_is_off_in_the_dock");

    let center = editor.rect_of(&shape).center();
    editor.fingers(TouchPhase::Start, &[(1, center)]);
    for step in 1..=4 {
        editor.fingers(
            TouchPhase::Move,
            &[(1, center + Vec2::new(30.0 * step as f32, 0.0))],
        );
    }
    editor.advance(std::time::Duration::from_secs(1));
    editor.run();
    editor.fingers(TouchPhase::End, &[(1, center + Vec2::new(120.0, 0.0))]);
    assert_eq!(
        entities(&editor),
        vec![rectangle.clone()],
        "a resting hand moves nothing"
    );
    assert!(
        !editor.shown("infinite-canvas.selection-bar"),
        "and selects nothing"
    );
    assert!(
        !editor.shown("infinite-canvas.menu.canvas.delete"),
        "nor opens the menu"
    );

    editor.pen(PenPhase::Move, center, 0.0);
    editor.next_frame();
    editor.pen(PenPhase::Down(PointerButton::Primary), center, 0.5);
    editor.next_frame();
    editor.pen(PenPhase::Up(PointerButton::Primary), center, 0.0);
    editor.run();
    assert!(
        editor.shown("infinite-canvas.selection-bar"),
        "the pen still selects"
    );

    editor.reveal(toggle);
    let at = editor.point_of(toggle);
    editor.fingers(TouchPhase::Start, &[(1, at)]);
    editor.fingers(TouchPhase::End, &[(1, at)]);
    assert_eq!(
        kept(&editor),
        Some(CanvasTouch::On),
        "a finger still reaches the dock to turn touch back on"
    );
}
