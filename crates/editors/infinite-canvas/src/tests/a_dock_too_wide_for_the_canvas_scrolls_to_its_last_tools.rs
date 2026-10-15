use super::*;
use block_editor_beui::beui::Vec2;

#[test]
fn a_dock_too_wide_for_the_canvas_scrolls_to_its_last_tools() {
    let mut editor = open_sized(
        &Canvas::with_entities(Vec::new()),
        false,
        &[],
        Some(Vec2::new(300.0, 640.0)),
    );
    let dock = editor.rect_of("infinite-canvas.dock");
    let redo = editor.rect_of("infinite-canvas.dock.redo");
    assert!(dock.right() <= 300.0, "the dock keeps inside the canvas, got {dock:?}");
    assert!(redo.left() > dock.right(), "redo starts scrolled out of view, at {redo:?}");
    editor.record();

    let from = dock.center() + Vec2::new(dock.width() / 3.0, 0.0);
    let to = from - Vec2::new(dock.width() * 2.0 / 3.0, 0.0);
    editor.touch_start(from);
    editor.run();
    editor.touch_move(from + (to - from) * 0.5);
    editor.run();
    editor.touch_move(to);
    editor.run();
    editor.touch_end(to);
    editor.settle();

    let redo = editor.rect_of("infinite-canvas.dock.redo");
    assert!(
        redo.right() <= dock.right(),
        "swiping the dock brings redo into view, at {redo:?}"
    );
    editor.snapshot("a_dock_too_wide_for_the_canvas_scrolls_to_its_last_tools");
}
