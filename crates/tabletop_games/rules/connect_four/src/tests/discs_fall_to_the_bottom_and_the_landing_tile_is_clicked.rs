use game_api::board::{Board, Sprite};
use game_api::{Gesture, Spot};
use uuid::Uuid;

use super::{play, show};

#[test]
fn discs_fall_to_the_bottom_and_the_landing_tile_is_clicked() {
    let red = Uuid::new_v4();
    let yellow = Uuid::new_v4();
    let actions = vec![play(&[], red, 3)];

    let screen = show(&actions, yellow);
    let Board::Grid(grid) = &screen.board else {
        panic!("connect four is played on a grid");
    };
    assert_eq!((grid.columns, grid.rows), (7, 6));
    assert_eq!(grid.tile(3, 5).layers, [Sprite::piece("disc", 0)]);
    assert!(grid.tile(3, 4).layers.is_empty());

    assert_eq!(
        screen.actions[3].gesture,
        Some(Gesture::Click(Spot::tile(3, 4)))
    );
    assert_eq!(
        screen.actions[0].gesture,
        Some(Gesture::Click(Spot::tile(0, 5)))
    );
}
