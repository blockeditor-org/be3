use super::*;
use beui::PointerButton;

#[test]
fn a_tree_row_dropped_on_the_middle_of_a_folder_moves_inside_it() {
    let mut test = alone(WIDE, Page::Tree);
    test.frame(Vec::new());
    let grab = test.rect_of("demo.tree.7.row").center();
    let src = test.rect_of("demo.tree.1.row").center();
    let indent = test.rect_of("demo.tree.7.row").left();
    test.down(grab, PointerButton::Primary);
    test.frame(vec![Input::PointerMoved(grab - Vec2::new(0.0, 8.0))]);
    test.frame(vec![Input::PointerMoved(src)]);
    test.settle();
    test.up(src, PointerButton::Primary);
    test.settle();
    let moved = test.rect_of("demo.tree.7.row");
    assert!(
        moved.left() > indent,
        "README.md is indented one more level, inside src"
    );
    assert!(
        moved.top() > test.rect_of("demo.tree.3.row").top()
            && moved.top() < test.rect_of("demo.tree.4.row").top(),
        "and listed last in src, before assets"
    );
}
