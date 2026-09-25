use super::*;
use block_editor_plugin::beui::Vec2;

#[test]
fn dragging_with_the_rectangle_tool_adds_a_rectangle() {
    let mut editor = editor(&[]);

    editor.click("infinite-canvas.tool.Rectangle");
    editor.run();
    let canvas = editor.rect_of("infinite-canvas.canvas");
    let from = canvas.center();
    editor.drag(from, from + Vec2::new(120.0, 80.0));
    editor.run();

    let entities = entities(&editor);
    assert_eq!(entities.len(), 1, "the drag adds one entity");
    assert_eq!(entities[0].kind, CanvasEntityKind::Rectangle);
    assert!(entities[0].transform.size.x > 4.0);
    assert!(entities[0].transform.size.y > 4.0);
}
