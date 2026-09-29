use super::*;

#[test]
fn a_view_change_the_editor_asks_for_repaints_the_frame() {
    let viewport = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0));
    let content = vec2(400.0, 300.0);
    let mut state = DirectEditorTabViewport::default();

    host::test_frame(Vec::new(), None, false);
    settle_viewport(&mut state, Vec::new(), viewport, content, 32.0);
    assert!(!host::repaint_requested());

    for command in [
        DirectEditorViewportCommand::Zoom {
            factor: 1.25,
            anchor: None,
        },
        DirectEditorViewportCommand::Fit,
    ] {
        host::test_frame(Vec::new(), None, false);
        settle_viewport(&mut state, vec![command], viewport, content, 32.0);
        assert!(host::repaint_requested());
    }
}
