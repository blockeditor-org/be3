use super::*;

#[test]
fn an_editor_not_docked_draws_its_dock_itself_and_declares_nothing() {
    let mut session = docked_session();

    show_docked(&mut session, &[(EditorRegion::Frame, 0)], false);

    assert!(dock_messages(&mut session).is_empty());
    assert_eq!(BODIES.with(std::cell::Cell::get), 1);
    let placed = session.placed_children(EditorRegion::Frame).to_vec();
    let [child] = placed.as_slice() else {
        panic!("the shown tab's body should place its child block, not {placed:?}");
    };
    assert_eq!(child.content.block_id(), Some(DOCKED_CHILD.into_bytes()));
    assert_eq!(
        child.docks,
        Some(1),
        "a dock that docks here is the root for the children in its tabs"
    );
}
