use super::*;

#[test]
fn a_split_divides_its_area_between_the_panes_and_the_handle() {
    let mut state = DockState::new([TabId::new(1)]);
    let leaf = state.leaves(state.main())[0];
    state.split(leaf, Side::Right, 0.25, vec![TabId::new(2)]);
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(406.0, 200.0));
    let layout = layout_surface(&state, state.main(), area, 6.0);

    assert_eq!(
        layout.leaves.len(),
        2,
        "a split surface lays out both of its leaves"
    );
    assert_eq!(
        layout.leaves[0].1.width(),
        300.0,
        "the pane that was split keeps the share it was left"
    );
    assert_eq!(
        layout.leaves[1].1.width(),
        100.0,
        "the new pane takes the share the split was given"
    );
    assert_eq!(
        layout.splitters[0].handle.width(),
        6.0,
        "the handle takes its thickness out of the middle"
    );
}
