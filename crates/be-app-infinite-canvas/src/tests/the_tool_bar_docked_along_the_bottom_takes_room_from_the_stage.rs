use super::*;

#[test]
fn the_tool_bar_docked_along_the_bottom_takes_room_from_the_stage() {
    let mut editor = editor(&[card()]);
    let over = editor.rect_of("infinite-canvas.canvas");
    editor.click("infinite-canvas.dock.grip");
    editor.run();
    editor.click("grip.dock.bar.beside.bottom");
    editor.settle();

    let stage = editor.rect_of("infinite-canvas.canvas");
    let dock = editor.rect_of("infinite-canvas.dock");
    assert!(
        stage.bottom() <= dock.top(),
        "the stage ends above the bar, {stage:?} over {dock:?}"
    );
    assert!(
        stage.height() < over.height(),
        "and is shorter than it was under a floating bar"
    );
    editor.snapshot("the_tool_bar_docked_along_the_bottom_takes_room_from_the_stage");
}
