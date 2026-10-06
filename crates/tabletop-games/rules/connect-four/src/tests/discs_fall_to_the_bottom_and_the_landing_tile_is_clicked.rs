use game_api::board::Sprite;
use game_api::{Gesture, Spot};
use uuid::Uuid;

use super::{play, show};

#[test]
fn discs_fall_to_the_bottom_and_the_landing_tile_is_clicked() {
    let red = Uuid::new_v4();
    let yellow = Uuid::new_v4();
    let actions = vec![play(&[], red, 3)];

    let screen = show(&actions, yellow);
    let board = &screen.board;
    assert_eq!(
        board.sprites_at(Spot::tile(3, 5)),
        [Sprite::Cell, Sprite::piece("disc", 0)]
    );
    assert_eq!(board.sprites_at(Spot::tile(3, 4)), [Sprite::Cell]);
    assert_eq!(board.sprites_at(Spot::tile(6, 0)), [Sprite::Cell]);
    assert!(board.sprites_at(Spot::tile(7, 0)).is_empty());

    assert_eq!(
        screen.actions[3].gesture,
        Some(Gesture::Click(Spot::tile(3, 4)))
    );
    assert_eq!(
        screen.actions[0].gesture,
        Some(Gesture::Click(Spot::tile(0, 5)))
    );
}
