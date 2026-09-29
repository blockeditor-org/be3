use super::*;

#[test]
fn an_editor_may_zoom_past_the_default_limit() {
    let viewport = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0));
    let content = vec2(400.0, 300.0);
    let zoom = DirectEditorViewportCommand::Zoom {
        factor: 1024.0,
        anchor: None,
    };
    let mut limited = DirectEditorTabViewport::default();
    let mut deep = DirectEditorTabViewport::default();

    host::test_frame(Vec::new(), None, false);
    settle_viewport(
        &mut limited,
        vec![zoom],
        viewport,
        content,
        DIRECT_EDITOR_MAX_ZOOM,
    );
    settle_viewport(&mut deep, vec![zoom], viewport, content, 32768.0);

    assert_eq!(limited.zoom, DIRECT_EDITOR_MAX_ZOOM);
    assert_eq!(deep.zoom, 1024.0);
}
