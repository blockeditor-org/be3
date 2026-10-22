use super::*;

#[test]
fn the_dock_ends_with_ignore_touch_after_the_block_button() {
    let editor = editor(&[]);
    let left = |id: &str| editor.rect_of(&format!("infinite-canvas.dock.{id}")).left();
    let order = ["tool.Select", "tool.Arrow", "block", "ignore-touch"];
    for pair in order.windows(2) {
        assert!(
            left(pair[0]) < left(pair[1]),
            "{} comes before {} in the dock",
            pair[0],
            pair[1]
        );
    }
}
