use crate::board::{PilePlace, Spot, Sprite};
use crate::table::{DISCARD_PILE, DRAW_PILE};

use super::seated;

#[test]
fn each_viewer_sees_their_own_hand_and_the_backs_of_everyone_elses() {
    let (players, table) = seated(2);

    let board = table.board(players[1]);
    let places: Vec<PilePlace> = board.piles.iter().map(|pile| pile.place).collect();
    assert_eq!(
        places,
        [
            PilePlace::Deck,
            PilePlace::Discard,
            PilePlace::Opponent,
            PilePlace::Hand
        ]
    );
    assert_eq!(
        board.piles[DISCARD_PILE as usize].cards,
        [Sprite::Card(table.face_up())]
    );
    assert!(
        board.piles[DRAW_PILE as usize]
            .cards
            .iter()
            .chain(&board.piles[2].cards)
            .all(|card| *card == Sprite::CardBack)
    );
    let own: Vec<Sprite> = table.hands[1].iter().copied().map(Sprite::Card).collect();
    assert_eq!(board.piles[3].cards, own);

    assert_eq!(table.hand_pile(players[1]), Some(3));
    let first = table.hand()[0];
    assert_eq!(table.in_hand(first), Spot::card(2, 0));
}
