use super::*;

#[test]
fn squares_put_every_tile_under_its_own_spot() {
    let squares = Squares::checkered(8, 8);
    let mut board = squares.board(|column, row| {
        Sprite::Square(match (column + row) % 2 {
            0 => Shade::Light,
            _ => Shade::Dark,
        })
    });
    squares.place(
        &mut board,
        ItemId::new(1, 0),
        4,
        6,
        Sprite::piece("pawn", 0),
    );

    assert_eq!((board.width, board.height), (8 * 64 + 12, 8 * 64 + 12));
    assert_eq!(board.items[0].sprite, Sprite::Frame);
    assert_eq!(
        board.items[0].area,
        Area::new(0, 0, board.width, board.height)
    );
    assert_eq!(squares.area(1, 0), Area::new(6 + 64, 6, 64, 64));
    assert_eq!(
        board.sprites_at(Spot::tile(4, 6)),
        [Sprite::Square(Shade::Light), Sprite::piece("pawn", 0)]
    );
    assert_eq!(
        board.sprites_at(Spot::tile(7, 7)),
        [Sprite::Square(Shade::Light)]
    );
    assert!(board.sprites_at(Spot::tile(8, 0)).is_empty());

    let cells = Squares::cells(3, 3);
    assert_eq!(cells.area(2, 2), Area::new(6 + 2 * 70, 6 + 2 * 70, 64, 64));
    assert_eq!(cells.board(|_, _| Sprite::Cell).width, 3 * 64 + 2 * 6 + 12);
}
