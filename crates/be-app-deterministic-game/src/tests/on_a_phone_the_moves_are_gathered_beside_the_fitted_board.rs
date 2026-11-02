use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);
const LANDSCAPE_PHONE: Vec2 = Vec2::new(844.0, 390.0);

fn fitted(editor: &BeuiTest<DeterministicGameRoot>) {
    let stage = editor.content_rect().expect("the game reports its board");
    let board = editor
        .rect_of("game.tile.0.0")
        .union(editor.rect_of("game.tile.7.7"));
    assert!(
        stage.expand(0.5).contains_rect(board),
        "the whole board is fitted into the stage, {board:?} in {stage:?}"
    );
}

#[test]
fn on_a_phone_the_moves_are_gathered_beside_the_fitted_board() {
    let actions = played(CHESS, &[(ACCOUNT, "e4"), (OPPONENT, "e5")]);
    let mut editor = editor_after(CHESS.to_vec(), actions).with_size(PHONE);
    editor.settle();
    let stage = editor.content_rect().expect("the game reports its board");
    let gathered = editor.rect_of("dock.compact");
    assert!(
        editor.shown("game.history.0.0") && stage.bottom() <= gathered.top() + 1.0,
        "the moves are gathered below the board, {stage:?} {gathered:?}"
    );
    assert!(editor.shown("game.actions"));
    fitted(&editor);
    editor.snapshot("on_a_phone_the_moves_are_gathered_below_the_fitted_board");

    let splitter = editor.rect_of("dock.compact.splitter").center();
    editor.drag(splitter, splitter - Vec2::new(0.0, 200.0));
    editor.settle();
    fitted(&editor);

    editor.set_size(LANDSCAPE_PHONE);
    editor.settle();
    let stage = editor.content_rect().expect("the game reports its board");
    let gathered = editor.rect_of("dock.compact");
    assert!(
        editor.shown("game.history.0.0") && stage.right() <= gathered.left() + 1.0,
        "on a phone on its side the moves are gathered right of the board, {stage:?} {gathered:?}"
    );
    fitted(&editor);
    editor.snapshot("on_a_phone_on_its_side_the_moves_are_gathered_right_of_the_fitted_board");
}
