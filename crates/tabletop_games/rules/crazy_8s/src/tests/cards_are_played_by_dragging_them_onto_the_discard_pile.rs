use game_api::board::{Board, PilePlace, Sprite};
use game_api::table::{DISCARD_PILE, DRAW_PILE};
use game_api::{Gesture, Spot};
use uuid::Uuid;

use super::{show, started};

#[test]
fn cards_are_played_by_dragging_them_onto_the_discard_pile() {
    let players = [Uuid::new_v4(), Uuid::new_v4()];
    let actions = started(&players);

    let screen = show(&actions, players[0]);
    let Board::Cards(table) = &screen.board else {
        panic!("crazy 8s is played on a card table");
    };
    let hand = &table.piles[2];
    assert_eq!(hand.place, PilePlace::Hand);
    assert_eq!(table.piles[3].place, PilePlace::Opponent);

    for option in &screen.actions {
        match option.gesture {
            Some(Gesture::Drag {
                from: Spot::Card { pile: 2, card },
                to: Spot::Pile(DISCARD_PILE),
            }) => {
                let Sprite::Card(dragged) = hand.cards[card as usize] else {
                    panic!("your own hand is face up");
                };
                assert!(option.label.starts_with(&format!("Play {dragged}")));
            }
            Some(Gesture::Click(Spot::Pile(DRAW_PILE))) => {
                assert_eq!(option.label, "Draw a card");
            }
            gesture => panic!("{} is offered as {gesture:?}", option.label),
        }
    }
}
