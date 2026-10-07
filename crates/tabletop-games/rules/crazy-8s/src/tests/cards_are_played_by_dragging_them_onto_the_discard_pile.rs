use game_api::board::Sprite;
use game_api::table::{DISCARD_PILE, DRAW_PILE};
use game_api::{Gesture, Spot};
use uuid::Uuid;

use super::{show, started};

#[test]
fn cards_are_played_by_dragging_them_onto_the_discard_pile() {
    let players = [Uuid::new_v4(), Uuid::new_v4()];
    let actions = started(&players);

    let screen = show(&actions, players[0]);
    let board = &screen.board;
    assert!(board.items.iter().all(|item| {
        item.spot
            .is_none_or(|spot| !matches!(spot, Spot::Card { pile: 2, .. }))
    }));
    assert!(
        board
            .items
            .iter()
            .any(|item| matches!(item.spot, Some(Spot::Card { pile: 3, .. })))
    );

    for option in &screen.actions {
        match option.gesture {
            Some(Gesture::Drag {
                from: Spot::Card { pile: 2, card },
                to: Spot::Pile(DISCARD_PILE),
            }) => {
                let held = &board.hand[card as usize];
                assert_eq!(held.spot, Spot::card(2, card));
                let Sprite::Card(dragged) = held.sprite else {
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
