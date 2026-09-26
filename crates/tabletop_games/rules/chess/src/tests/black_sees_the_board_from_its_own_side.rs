use game_api::board::{Board, Sprite};
use game_api::{Gesture, Spot};

use super::*;

fn corner(screen: &GameScreen) -> Option<Sprite> {
    let Board::Grid(grid) = &*screen.board else {
        panic!("chess is played on a grid");
    };
    grid.tile(0, 7).layers.last().cloned()
}

#[test]
fn black_sees_the_board_from_its_own_side() {
    let actions = played(&[(FIRST, "e4")]);

    let white = show(&actions, FIRST);
    let black = show(&actions, SECOND);

    assert_eq!(corner(&white), Some(Sprite::piece("rook", 0)));
    assert_eq!(corner(&black), Some(Sprite::piece("rook", 1)));
    let push = black
        .actions
        .iter()
        .find(|option| option.label == "e5")
        .expect("black can answer e5");
    assert_eq!(
        push.gesture,
        Some(Gesture::Drag {
            from: Spot::tile(3, 6),
            to: Spot::tile(3, 4),
        })
    );
}
