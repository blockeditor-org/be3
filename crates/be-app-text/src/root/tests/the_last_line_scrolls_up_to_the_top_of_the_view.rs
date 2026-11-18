use beui::Vec2;

use super::*;

#[test]
fn the_last_line_scrolls_up_to_the_top_of_the_view() {
    let text = (1..=40)
        .map(|line| format!("Line {line}\n"))
        .collect::<String>();
    let mut editor = editor(text.trim_end());
    editor.scroll("text.surface", Vec2::new(0.0, 1.0e6));

    let document = editor.document();
    let surface = document
        .find_test_id("text.surface")
        .expect("the text is drawn");
    let top = document.node_rect(surface).expect("laid out").min.y;
    let mut pending = vec![surface];
    let mut last = None;
    while let Some(node) = pending.pop() {
        if document.node_kind(node) == "text"
            && document
                .node_detail(node)
                .unwrap_or_default()
                .trim_matches('"')
                == "Line 40"
        {
            last = document.node_rect(node);
        }
        pending.extend(document.children(node));
    }
    let last = last.expect("the last line is shown");
    assert!(
        last.min.y - top < 40.0,
        "the last line is at the top of the view: {last:?} below {top}"
    );
    drop(document);

    editor.snapshot("the_last_line_scrolls_up_to_the_top_of_the_view");
}
