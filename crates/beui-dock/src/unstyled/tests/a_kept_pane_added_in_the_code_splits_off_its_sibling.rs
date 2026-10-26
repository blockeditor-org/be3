use super::*;

#[test]
fn a_kept_pane_added_in_the_code_splits_off_its_sibling() {
    let mut state = DockState::default();
    state.reconcile(&spec(pane("files", &[1])));
    state.reconcile(&spec(split(
        "root",
        Direction::Horizontal,
        0.25,
        kept("outline", &[]),
        pane("files", &[1]),
    )));

    let leaves = state.leaves(state.main());
    assert_eq!(
        leaves.len(),
        2,
        "the new pane is laid out beside its sibling"
    );
    assert_eq!(
        state.pane_key(leaves[0]),
        Some("outline"),
        "the new pane takes the side the code put it on"
    );
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1006.0, 400.0));
    let splitter = layout_surface(&state, state.main(), area, 6.0).splitters[0];
    assert_eq!(
        state.split_fraction(splitter.id),
        0.25,
        "the new pane takes the share the code gave it"
    );
}
