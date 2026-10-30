use super::*;

#[test]
fn the_tool_bar_moved_to_the_left_stands_its_tools_in_a_column() {
    let rectangle = card();
    let mut editor = editor(std::slice::from_ref(&rectangle));
    editor.click("infinite-canvas.dock.grip");
    editor.run();
    editor.click("grip.dock.bar.over.left");
    editor.settle();

    let dock = editor.rect_of("infinite-canvas.dock");
    assert!(
        dock.height() > dock.width(),
        "a bar on the left edge is a column, got {dock:?}"
    );
    assert!(
        dock.left() < editor.rect().center().x / 4.0,
        "and it floats over the left of the stage, at {dock:?}"
    );
    let top = editor.rect_of("infinite-canvas.dock.tool.Select");
    let below = editor.rect_of("infinite-canvas.dock.tool.Pen");
    assert!(top.bottom() <= below.top(), "its tools run down the side");

    editor.click(format!("infinite-canvas.entity.{}", rectangle.id));
    editor.run();
    let selection = editor.rect_of("infinite-canvas.selection-bar");
    assert!(
        selection.left() >= dock.right(),
        "the selection's tools stand beside the bar, at {selection:?}"
    );
    assert!(
        selection.height() > selection.width(),
        "in a column like it, got {selection:?}"
    );
    editor.snapshot("the_tool_bar_moved_to_the_left_stands_its_tools_in_a_column");
}
