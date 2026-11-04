use super::*;
use beui::PointerButton;
use std::time::Duration;

#[test]
fn a_tree_row_dragged_between_two_others_parts_them_and_moves_there() {
    let mut test = alone(WIDE, Page::Tree);
    test.frame(Vec::new());
    let grab = test.rect_of("demo.tree.8.row").center();
    let assets = test.rect_of("demo.tree.4.row");
    let above_assets = pos2(assets.center().x, assets.top() + 2.0);
    test.down(grab, PointerButton::Primary);
    test.frame(vec![Input::PointerMoved(grab - Vec2::new(0.0, 8.0))]);
    test.frame(vec![Input::PointerMoved(above_assets)]);
    test.record();
    for _ in 0..3 {
        test.advance(Duration::from_millis(50));
        test.frame(Vec::new());
        test.record();
    }
    test.settle();
    assert!(
        test.rect_of("demo.tree.4.row").top() >= assets.bottom(),
        "assets moves down a row to make room above it"
    );
    test.up(above_assets, PointerButton::Primary);
    test.record();
    for _ in 0..3 {
        test.advance(Duration::from_millis(50));
        test.frame(Vec::new());
        test.record();
    }
    test.settle();
    let tops: Vec<f32> = [3, 8, 4]
        .map(|row| test.rect_of(&format!("demo.tree.{row}.row")).top())
        .to_vec();
    assert!(
        tops[0] < tops[1] && tops[1] < tops[2],
        "Cargo.toml lands after src's last file and before assets: {tops:?}"
    );
    test.snapshot("a_tree_row_dragged_between_two_others_parts_them_and_moves_there");
}
