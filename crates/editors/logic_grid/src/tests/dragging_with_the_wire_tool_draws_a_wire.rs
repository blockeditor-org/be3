use super::*;

#[test]
fn dragging_with_the_wire_tool_draws_a_wire() {
    let mut editor = editor();

    editor.click("logic-grid.slot.0");
    editor.run();
    let from = canvas_point(&editor, Vec2::new(-60.0, 10.0));
    let to = canvas_point(&editor, Vec2::new(60.0, 10.0));
    editor.drag(from, to);
    editor.run();
    editor.run();

    let grid = grid(&editor);
    assert_eq!(grid.wires().len(), 1, "the drag drew one wire");
    let wire = grid.wires()[0];
    assert_eq!(
        wire.start.y, wire.end.y,
        "a sideways drag draws a horizontal wire"
    );
    assert!(
        wire.end.x - wire.start.x >= 4,
        "the wire spans the cells dragged over"
    );
}
