use super::*;

const CARD: &str = "GenericContainer #infinite-canvas.entity.";
const BOARD: &str = "#game.board";
const CANVAS_TOOLS: &str = "#infinite-canvas.dock";
const MOVES: &str = "Tab \"Moves\"";
const FIRST_MOVE: &str = "#game.history.0.0";

#[test]
fn a_game_on_a_canvas_takes_its_first_drag_as_a_move_and_hands_its_bars_back() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Canvas");
    app.click("#infinite-canvas.dock.block");
    choose_type(&mut app, "Game");
    app.click("#game.staged.1");
    app.click("#creation.confirm");
    app.gone("#creation.confirm");
    app.wait("#game.tile.4.6");
    app.gone("#editor.loading");
    app.run(&["settle"]);
    let card = app.place(CARD);
    let away = format!("{:.0},{:.0}", card.0 - 120.0, card.1 - 60.0);
    app.run(&["click", &away]);
    app.run(&["settle"]);

    assert!(
        !app.shown(MOVES) && !app.shown("#game.actions"),
        "on the canvas the game shows only its board"
    );
    assert!(app.shown(CANVAS_TOOLS));
    let card = app.place(CARD);
    let board = app.place(BOARD);
    app.snapshot("a_game_on_a_canvas_shows_only_its_board");

    let center = |(x, y, width, height): (f32, f32, f32, f32)| {
        format!("{:.0},{:.0}", x + width / 2.0, y + height / 2.0)
    };
    let from = center(app.place("#game.tile.4.6"));
    let to = center(app.place("#game.tile.4.4"));
    app.run(&["down", &from]);
    app.run(&["move", &to]);
    app.run(&["up", &to]);
    app.wait(FIRST_MOVE);
    app.wait(MOVES);
    app.wait("#game.actions");
    assert!(
        !app.shown(CANVAS_TOOLS),
        "the game's bars take the place of the canvas's"
    );
    assert_eq!(app.place(CARD), card, "the canvas does not move");
    assert_eq!(app.place(BOARD), board, "and neither does the game");
    app.snapshot("a_game_on_a_canvas_takes_its_first_drag_as_a_move");

    app.key("Escape");
    app.gone(MOVES);
    app.wait(CANVAS_TOOLS);
    assert_eq!(
        app.place(CARD),
        card,
        "Escape leaves everything where it was"
    );

    let square = center(app.place("#game.tile.2.4"));
    app.run(&["click", &square]);
    app.wait(MOVES);
    app.run(&["click", &away]);
    app.gone(MOVES);
    app.wait(CANVAS_TOOLS);
    assert_eq!(
        app.place(CARD),
        card,
        "a click away hands the canvas its bars back"
    );
    app.close();
}
