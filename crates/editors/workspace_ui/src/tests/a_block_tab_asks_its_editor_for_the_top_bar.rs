use super::*;

#[test]
fn a_block_tab_asks_its_editor_for_the_top_bar() {
    let (mut fixture, opened) = editor();

    show(&mut fixture, opened, None);

    let placement = fixture
        .test
        .children()
        .iter()
        .find(|placement| Uuid::from_bytes(placement.block_id) == opened)
        .expect("the shown block is placed");
    assert!(placement.own_frame, "a block tab owns its frame");
    assert!(placement.top_bar, "a block tab asks for the top bar");
}
