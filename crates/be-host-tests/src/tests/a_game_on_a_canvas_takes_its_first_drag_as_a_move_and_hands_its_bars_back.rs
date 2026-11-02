use super::*;

const CARD: &str = "GenericContainer #infinite-canvas.entity.";
const BOARD: &str = "#game.board";
const CANVAS_TOOLS: &str = "#infinite-canvas.dock";
const HISTORY: &str = "#game.history";
const FIRST_MOVE: &str = "#game.history.0.0";

#[test]
fn a_game_on_a_canvas_takes_its_first_drag_as_a_move_and_hands_its_bars_back() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Canvas");
    host.click("#infinite-canvas.dock.block");
    choose_type(&mut host, "Game");
    host.click("#game.staged.1");
    host.click("#creation.confirm");
    host.gone("#creation.confirm");
    host.wait("#game.tile.4.6");
    host.gone("#editor.loading");
    host.run(&["settle"]);
    let card = host.place(CARD);
    let away = format!("{:.0},{:.0}", card.0 - 120.0, card.1 - 60.0);
    host.run(&["click", &away]);
    host.run(&["settle"]);

    assert!(
        !host.shown(HISTORY) && !host.shown("#game.actions"),
        "on the canvas the game shows only its board"
    );
    assert!(host.shown(CANVAS_TOOLS));
    let card = host.place(CARD);
    let board = host.place(BOARD);
    host.snapshot("a_game_on_a_canvas_shows_only_its_board");

    let center = |(x, y, width, height): (f32, f32, f32, f32)| {
        format!("{:.0},{:.0}", x + width / 2.0, y + height / 2.0)
    };
    let from = center(host.place("#game.tile.4.6"));
    let to = center(host.place("#game.tile.4.4"));
    host.run(&["down", &from]);
    host.run(&["move", &to]);
    host.run(&["up", &to]);
    host.wait(FIRST_MOVE);
    host.wait(HISTORY);
    host.wait("#game.actions");
    assert!(
        !host.shown(CANVAS_TOOLS),
        "the game's bars take the place of the canvas's"
    );
    assert_eq!(host.place(CARD), card, "the canvas does not move");
    assert_eq!(host.place(BOARD), board, "and neither does the game");
    host.snapshot("a_game_on_a_canvas_takes_its_first_drag_as_a_move");

    host.key("Escape");
    host.gone(HISTORY);
    host.wait(CANVAS_TOOLS);
    assert_eq!(
        host.place(CARD),
        card,
        "Escape leaves everything where it was"
    );

    let square = center(host.place("#game.tile.2.4"));
    host.run(&["click", &square]);
    host.wait(HISTORY);
    host.run(&["click", &away]);
    host.gone(HISTORY);
    host.wait(CANVAS_TOOLS);
    assert_eq!(
        host.place(CARD),
        card,
        "a click away hands the canvas its bars back"
    );
    host.close();
}
