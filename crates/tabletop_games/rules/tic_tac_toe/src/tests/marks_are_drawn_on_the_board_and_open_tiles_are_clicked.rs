use game_api::board::{Board, Sprite};
use game_api::{Gesture, Spot};
use uuid::Uuid;

use super::{play, show};

#[test]
fn marks_are_drawn_on_the_board_and_open_tiles_are_clicked() {
    let x = Uuid::new_v4();
    let o = Uuid::new_v4();
    let actions = vec![play(&[], x, 4)];

    let screen = show(&actions, o);
    let Board::Grid(grid) = &screen.board else {
        panic!("tic-tac-toe is played on a grid");
    };
    assert_eq!((grid.columns, grid.rows), (3, 3));
    assert_eq!(grid.tile(1, 1).layers, [Sprite::piece("x", 0)]);
    assert!(grid.tile(0, 0).layers.is_empty());

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
