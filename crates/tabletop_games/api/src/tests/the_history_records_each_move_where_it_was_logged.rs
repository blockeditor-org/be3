use uuid::Uuid;

use super::{GameAction, GameHelper, taken};
use crate::Move;

#[test]
fn the_history_records_each_move_where_it_was_logged() {
    let player = Uuid::new_v4();
    let log = [
        taken(player, 1),
        GameAction {
            actor: player,
            action: Vec::new(),
        },
        taken(player, 0),
    ];
    let helper = GameHelper::new(&log, player);
    let turn = || {
        helper.action(
            |_| "Pick a side",
            |_, choose| {
                choose(Move::new("Heads").recorded("Called heads"));
                choose(Move::new("Tails"));
            },
        )
    };
    turn().expect("the first move is in the log");
    helper.annotate("!");
    turn().expect("the second move is in the log");

    let screen = turn().expect_err("the log has run out");

    let history: Vec<(u32, &str)> = screen
        .history
        .iter()
        .map(|turn| (turn.entry, turn.description.as_str()))
        .collect();
    assert_eq!(history, [(0, "Tails!"), (2, "Called heads")]);
}
