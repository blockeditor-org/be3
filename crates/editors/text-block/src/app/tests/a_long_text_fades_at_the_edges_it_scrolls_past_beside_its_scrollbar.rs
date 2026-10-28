use beui::Vec2;
use block_ui_test::prelude::*;

use super::editor;

#[test]
fn a_long_text_fades_at_the_edges_it_scrolls_past_beside_its_scrollbar() {
    let text = (1..=80)
        .map(|line| format!("Line {line}\n"))
        .collect::<String>();
    let mut editor = editor(&text);
    editor.scroll("text.surface", Vec2::new(0.0, 200.0));

    editor.snapshot("a_long_text_fades_at_the_edges_it_scrolls_past_beside_its_scrollbar");
}
