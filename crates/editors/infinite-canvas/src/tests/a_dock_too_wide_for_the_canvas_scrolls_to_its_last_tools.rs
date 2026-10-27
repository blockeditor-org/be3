use super::*;
use block_editor_beui::beui::{TouchPhase, Vec2};

#[test]
fn a_dock_too_wide_for_the_canvas_scrolls_to_its_last_tools() {
    let mut editor = open_sized(
        &Canvas::with_entities(Vec::new()),
        false,
        &[],
        Some(Vec2::new(300.0, 640.0)),
    );
    let dock = editor.rect_of("infinite-canvas.dock");
    let last = editor.rect_of("infinite-canvas.dock.ignore-touch");
    assert!(
        dock.right() <= 300.0,
        "the dock keeps inside the canvas, got {dock:?}"
    );
    assert!(
        last.left() > dock.right(),
        "the last tool starts scrolled out of view, at {last:?}"
    );
    editor.record();

    let from = dock.center() + Vec2::new(dock.width() / 3.0, 0.0);
    let to = from - Vec2::new(dock.width() * 2.0 / 3.0, 0.0);
    editor.fingers(TouchPhase::Start, &[(1, from)]);
    editor.fingers(TouchPhase::Move, &[(1, from + (to - from) * 0.5)]);
    editor.fingers(TouchPhase::Move, &[(1, to)]);
    editor.fingers(TouchPhase::End, &[(1, to)]);
    editor.settle();

    let last = editor.rect_of("infinite-canvas.dock.ignore-touch");
    assert!(
        last.right() <= dock.right(),
        "swiping the dock brings the last tool into view, at {last:?}"
    );
    editor.snapshot("a_dock_too_wide_for_the_canvas_scrolls_to_its_last_tools");
}
