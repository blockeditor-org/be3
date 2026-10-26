use super::*;

#[test]
fn tile_buttons_pick_one_of_a_row_of_pictures() {
    let mut test = alone(WIDE, Page::Choices);
    reveal(&mut test, "demo.choices.tiles");
    test.snapshot("tile_buttons_pick_one_of_a_row_of_pictures");
}
