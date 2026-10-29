use super::*;

#[test]
fn the_tool_bar_pops_out_of_the_canvas_into_a_window() {
    let mut editor = editor(&[card()]);
    let stage = editor.rect_of("infinite-canvas.canvas");
    let docked = editor.rect_of("infinite-canvas.dock");
    editor.click("infinite-canvas.dock.grip");
    editor.run();
    editor.click("grip.dock.bar.window");
    editor.settle();

    assert!(editor.shown("infinite-canvas.dock.tool.Select"));
    let popped = editor.rect_of("infinite-canvas.dock");
    assert_ne!(popped.min, docked.min, "the bar left its edge");
    assert_eq!(popped.size(), docked.size(), "and keeps its tools in a row");
    assert_eq!(
        editor.rect_of("infinite-canvas.canvas"),
        stage,
        "while the stage keeps its size"
    );
    editor.snapshot("the_tool_bar_pops_out_of_the_canvas_into_a_window");
}
