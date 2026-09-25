use super::*;

#[test]
fn hovering_a_pan_zoom_viewport_reads_its_gestures() {
    let at = pos2(50.0, 50.0);
    host::test_frame(vec![beui::Event::PointerMoved(at)], Some(at), false);
    let mut viewport = DirectEditorViewport::new();

    viewport_gesture_input(
        Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0)),
        None,
        &mut viewport,
    );

    assert!(viewport.commands.is_empty());
}
