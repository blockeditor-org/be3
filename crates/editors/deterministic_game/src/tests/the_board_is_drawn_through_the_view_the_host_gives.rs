use block_editor_beui::beui::{Rect, pos2, vec2};

use super::*;

#[test]
fn the_board_is_drawn_through_the_view_the_host_gives() {
    let mut editor = placed(CHESS.to_vec(), Vec::new(), false);

    editor.set_view(
        Rect::from_min_size(pos2(100.0, 50.0), vec2(270.0, 270.0)),
        0.5,
    );
    editor.run();

    assert_eq!(
        editor.rect_of("game.tile.0.0"),
        Rect::from_min_size(pos2(109.0, 59.0), vec2(32.0, 32.0))
    );
    assert_eq!(
        editor.rect_of("game.tile.7.7"),
        Rect::from_min_size(pos2(333.0, 283.0), vec2(32.0, 32.0))
    );
}
