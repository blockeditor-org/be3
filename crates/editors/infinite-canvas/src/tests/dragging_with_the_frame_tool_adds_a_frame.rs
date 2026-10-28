use super::*;
use block_editor_beui::beui::Vec2;

#[test]
fn dragging_with_the_frame_tool_adds_a_frame() {
    let mut editor = vector_editor(&[card()]).with_top_bar(false);
    assert!(
        entities(&editor).iter().all(|entity| !entity.is_frame()),
        "a canvas has no frame until one is drawn"
    );

    editor.pick_menu("canvas.tool.frame");
    let canvas = editor.rect_of("infinite-canvas.canvas");
    let from = canvas.center() - Vec2::new(200.0, 150.0);
    editor.drag(from, from + Vec2::new(300.0, 200.0));

    let frames: Vec<_> = entities(&editor)
        .into_iter()
        .filter(|entity| entity.is_frame())
        .collect();
    assert_eq!(frames.len(), 1, "the drag adds one frame");
    assert_eq!(frames[0].text.to_str_lossy(), "Frame 1");
    assert_eq!(frames[0].transform.size, CanvasPoint::new(300.0, 200.0));
    editor.snapshot("dragging_with_the_frame_tool_adds_a_frame");
}
