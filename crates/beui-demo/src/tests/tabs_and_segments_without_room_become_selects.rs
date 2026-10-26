use super::*;

#[test]
fn tabs_and_segments_without_room_become_selects() {
    let mut test = alone(WIDE, Page::Choices);
    reveal(&mut test, "demo.choices.narrow");
    test.snapshot("tabs_and_segments_without_room_become_selects");
}
