use super::*;

#[test]
fn a_block_in_a_tab_points_its_dialogs_at_the_tab() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1200.0, 800.0)));
    show(&mut fixture, opened, None);
    let placed = fixture
        .test
        .children()
        .iter()
        .find(|placement| placement.content.block_id() == Some(opened.into_bytes()))
        .cloned()
        .expect("the block is on show");
    let tab = placed.anchor.expect("its tab shows");
    assert!(
        tab.bottom() <= placed.rect.top() + 1.0 && tab.height() < 60.0,
        "what the block opens points at its tab above it: {tab:?} over {:?}",
        placed.rect
    );
    assert!(
        tab.left() >= placed.rect.left() - 1.0 && tab.right() <= placed.rect.right() + 1.0,
        "in the strip of the pane showing it: {tab:?} over {:?}",
        placed.rect
    );
}
