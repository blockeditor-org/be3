use super::*;
use block_editor_beui::beui::Vec2;

#[test]
fn the_text_tool_writes_its_box_while_it_is_dragged() {
    let mut editor = editor(&[]);
    editor.click("infinite-canvas.dock.tool.Text");
    editor.run();
    let start = editor.rect_of("infinite-canvas.canvas").center();
    editor.press_at(start);
    editor.next_frame();
    editor.hover_at(start + Vec2::new(120.0, 50.0));
    editor.run();

    let drawn = entities(&editor);
    assert_eq!(drawn.len(), 1, "the box is written before the press lifts");
    assert_eq!(drawn[0].shape, CanvasShape::Text);
    assert_eq!(drawn[0].transform.size, CanvasPoint::new(120.0, 50.0));

    editor.hover_at(start + Vec2::new(200.0, 60.0));
    editor.run();
    editor.release_at(start + Vec2::new(200.0, 60.0));
    editor.run();
    let drawn = entities(&editor);
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].transform.size, CanvasPoint::new(200.0, 60.0));

    undo(&mut editor);
    assert!(entities(&editor).is_empty(), "one undo takes the box away");
}
