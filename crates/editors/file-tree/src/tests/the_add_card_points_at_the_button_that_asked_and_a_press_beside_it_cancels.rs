use super::*;

#[test]
fn the_add_card_points_at_the_button_that_asked_and_a_press_beside_it_cancels() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    my_files(&mut fixture, Vec::new());
    fixture.settle();

    fixture.test.click("file-tree.new-block");
    fixture.settle();
    fixture.test.show_picker(Vec2::new(240.0, 160.0), true);
    fixture.settle();
    fixture
        .test
        .snapshot("the_add_card_points_at_the_button_that_asked");

    let button = fixture.test.rect_of("file-tree.new-block");
    let beside = Pos2::new(button.left() + 400.0, button.bottom() + 300.0);
    fixture.test.click(beside);
    fixture.settle();
    assert!(
        fixture.test.placed_picker().is_none(),
        "a press beside the card cancels the pick"
    );
}
