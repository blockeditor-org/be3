use super::*;

#[test]
fn a_spacer_keeps_each_space_token_between_its_neighbours() {
    let mut test = alone(WIDE, Page::Layout);
    reveal(&mut test, "demo.layout.spacers");
    test.snapshot("a_spacer_keeps_each_space_token_between_its_neighbours");
}
