use super::*;

#[test]
fn emptying_a_pane_gives_its_room_to_the_pane_it_was_split_from() {
    let mut state = DockState::new([TabId::new(1)]);
    let first = state.leaves(state.main())[0];
    let second = state
        .split(first, Side::Below, 0.5, vec![TabId::new(2)])
        .expect("the pane was split");
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 206.0));

    state.remove(TabId::new(2));

    assert_eq!(
        state.leaves(state.main()),
        vec![first],
        "the pane that was emptied is gone and the other one is left"
    );
    assert!(
        state.find(TabId::new(2)).is_none() && state.surface_of(second).is_none(),
        "nothing is left pointing at the pane that was removed"
    );
    let layout = layout_surface(&state, state.main(), area, 6.0);
    assert_eq!(
        layout.leaves[0].1, area,
        "the pane that is left takes the whole surface back"
    );
    assert!(
        layout.splitters.is_empty(),
        "the bar between them goes with the split"
    );
}
