use super::*;

#[test]
fn a_split_leaves_each_pane_room_for_its_tab_bar() {
    let mut state = DockState::new([TabId::new(1)]);
    let leaf = state.leaves(state.main())[0];
    state.split(leaf, Side::Right, 0.1, vec![TabId::new(2)]);
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(406.0, 200.0));
    let layout = layout_surface(&state, state.main(), area, 6.0);

    assert_eq!(
        layout.leaves[1].1.width(),
        MIN_PANE_LENGTH,
        "a pane given a sliver of a narrow area is still laid out wide enough to use"
    );
    assert_eq!(
        layout.splitters[0].fraction,
        1.0 - MIN_PANE_LENGTH / 400.0,
        "the splitter reports the share it is drawn at, so dragging it starts from there"
    );

    let cramped = Rect::from_min_size(Pos2::ZERO, Vec2::new(206.0, 200.0));
    let layout = layout_surface(&state, state.main(), cramped, 6.0);
    assert_eq!(
        layout.leaves[1].1.width(),
        20.0,
        "an area too small for two such panes splits by the share it was given"
    );
}
