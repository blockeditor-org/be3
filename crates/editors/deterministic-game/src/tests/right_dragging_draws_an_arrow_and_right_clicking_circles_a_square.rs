use super::*;

#[test]
fn right_dragging_draws_an_arrow_and_right_clicking_circles_a_square() {
    let mut editor = editor(CHESS.to_vec());
    let e2 = editor.rect_of("game.tile.4.6").center();
    let e4 = editor.rect_of("game.tile.4.4").center();
    let d5 = editor.rect_of("game.tile.3.3").center();

    editor.secondary_drag(e2, e4);
    editor.run();
    editor.secondary_drag(d5, d5);
    editor.run();

    assert!(moves(&editor).is_empty());
    editor.record();

    editor.click_at(d5);
    editor.run();
    editor.record();
    editor.snapshot("right_dragging_draws_an_arrow_and_right_clicking_circles_a_square");
}
