use super::*;
use block_editor_beui::beui::{TouchPhase, Vec2};

#[test]
fn a_second_finger_calls_off_the_shape_being_drawn() {
    for tool in ["Rectangle", "Pen"] {
        let mut editor = phone(&[]);

        editor.click(format!("infinite-canvas.dock.tool.{tool}"));
        editor.run();
        let from = editor.rect_of("infinite-canvas.canvas").center();
        editor.fingers(TouchPhase::Start, &[(1, from)]);
        editor.run();
        editor.fingers(TouchPhase::Move, &[(1, from + Vec2::new(80.0, 80.0))]);
        editor.run();
        editor.fingers(TouchPhase::Start, &[(2, from + Vec2::new(-60.0, -60.0))]);
        editor.run();
        editor.fingers(TouchPhase::Move, &[(2, from + Vec2::new(-90.0, -90.0))]);
        editor.run();
        editor.fingers(TouchPhase::End, &[(1, from + Vec2::new(80.0, 80.0))]);
        editor.run();
        editor.fingers(TouchPhase::End, &[(2, from + Vec2::new(-90.0, -90.0))]);
        editor.run();

        assert!(
            entities(&editor).is_empty(),
            "pinching partway through a {tool} drag draws nothing"
        );
    }
}
