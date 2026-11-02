use crate::board::{Spot, Sprite};
use crate::table::{DISCARD_PILE, DRAW_PILE};

use super::seated;

#[test]
fn each_viewer_sees_their_own_hand_and_the_backs_of_everyone_elses() {
    let (players, table) = seated(2);

    let board = table.board(players[1]);
    let on = |spot: Spot| -> Vec<Sprite> {
        board
            .items
            .iter()
            .filter(|item| item.spot == Some(spot) && item.sprite != Sprite::Slot)
            .map(|item| item.sprite.clone())
            .collect()
    };
    assert_eq!(
        on(Spot::Pile(DISCARD_PILE)),
        [Sprite::Card(table.face_up())]
    );
    assert_eq!(on(Spot::Pile(DRAW_PILE)).len(), table.draw_pile.len());
    assert!(
        on(Spot::Pile(DRAW_PILE))
            .iter()
            .all(|card| *card == Sprite::CardBack)
    );
    let theirs: Vec<Sprite> = (0..table.hands[0].len() as u32)
        .flat_map(|card| on(Spot::card(2, card)))
        .collect();
    assert_eq!(theirs, vec![Sprite::CardBack; table.hands[0].len()]);
    assert!(board.items.iter().all(|item| {
        item.spot
            .is_none_or(|spot| !matches!(spot, Spot::Card { pile: 3, .. }))
    }));

    let own: Vec<(Sprite, Spot)> = board
        .hand
        .iter()
        .map(|held| (held.sprite.clone(), held.spot))
        .collect();
    let expected: Vec<(Sprite, Spot)> = table.hands[1]
        .iter()
        .enumerate()
        .map(|(card, held)| (Sprite::Card(held.card), Spot::card(3, card as u32)))
        .collect();
    assert_eq!(own, expected);
    let ids: Vec<_> = board.hand.iter().map(|held| held.id).collect();
    assert_eq!(
        ids,
        table.hands[1]
            .iter()
            .map(|held| held.id)
            .collect::<Vec<_>>()
    );

    assert_eq!(table.hand_pile(players[1]), Some(3));
    let first = table.hand()[0];
    assert_eq!(table.in_hand(first), Spot::card(2, 0));
}
