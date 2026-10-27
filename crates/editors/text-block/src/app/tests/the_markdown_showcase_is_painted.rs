use beui::Vec2;
use block_ui_test::prelude::*;

use super::editor;

#[test]
fn the_markdown_showcase_is_painted() {
    let mut editor =
        editor(include_str!("markdown_showcase.md")).with_size(Vec2::new(900.0, 5200.0));
    editor.run();

    editor.snapshot("the_markdown_showcase_is_painted");
}
