use game_api::board::Sprite;
use game_api::{Gesture, Spot};
use uuid::Uuid;

use super::{play, show};

#[test]
fn marks_are_drawn_on_the_board_and_open_tiles_are_clicked() {
    let x = Uuid::new_v4();
    let o = Uuid::new_v4();
    let actions = vec![play(&[], x, 4)];

    let screen = show(&actions, o);
    let board = &screen.board;
    assert_eq!(
        board.sprites_at(Spot::tile(1, 1)),
        [Sprite::Cell, Sprite::piece("x", 0)]
    );
    assert_eq!(board.sprites_at(Spot::tile(0, 0)), [Sprite::Cell]);
    assert_eq!(board.sprites_at(Spot::tile(2, 2)), [Sprite::Cell]);
    assert!(board.sprites_at(Spot::tile(3, 0)).is_empty());

    assert_eq!(screen.actions.len(), 8);
    assert_eq!(
        screen.actions[0].gesture,
        Some(Gesture::Click(Spot::tile(0, 0)))
    );
    assert!(
        screen
            .actions
            .iter()
            .all(|action| action.gesture != Some(Gesture::Click(Spot::tile(1, 1))))
    );
}
