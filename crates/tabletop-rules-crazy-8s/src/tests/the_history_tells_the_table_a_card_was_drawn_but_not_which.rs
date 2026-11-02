use uuid::Uuid;

use super::{can_be_played, drawn_by_the_first_player, option, show, started};

#[test]
fn the_history_tells_the_table_a_card_was_drawn_but_not_which() {
    let players = loop {
        let players = [Uuid::new_v4(), Uuid::new_v4()];
        let (drawn, face_up) = drawn_by_the_first_player(&players);
        if !can_be_played(drawn, face_up, face_up.suit) {
            break players;
        }
    };
    let mut actions = started(&players);

    let draw = option(&actions, players[0], "Draw a card");
    actions.push(draw);

    let history: Vec<String> = show(&actions, players[1])
        .history
        .into_iter()
        .map(|turn| turn.description)
        .collect();
    assert_eq!(history, ["joins", "joins", "deals", "draw"]);
}
