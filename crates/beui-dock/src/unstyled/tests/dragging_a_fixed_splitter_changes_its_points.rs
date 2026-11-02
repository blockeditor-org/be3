use super::*;

#[test]
fn dragging_a_fixed_splitter_changes_its_points() {
    let mut state = DockState::default();
    state.reconcile(&spec(sized(
        "root",
        Direction::Horizontal,
        SplitSize::Second(300.0),
        pane("editors", &[1]),
        pane("inspector", &[2]),
    )));
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1006.0, 300.0));
    let splitter = layout_surface(&state, state.main(), area, 6.0).splitters[0];
    assert_eq!(splitter.size, SplitSize::Second(300.0));

    let moved = splitter.size.moved(splitter.available(6.0), -40.0);
    assert_eq!(
        moved,
        SplitSize::Second(340.0),
        "dragging the splitter toward the fixed half's far side widens it in points"
    );
    state.set_split_size(splitter.id, moved);
    let wider = Rect::from_min_size(Pos2::ZERO, Vec2::new(1606.0, 300.0));
    let layout = layout_surface(&state, state.main(), wider, 6.0);
    assert_eq!(
        layout.leaves[1].1.width(),
        340.0,
        "the half keeps the width it was dragged to when the dock grows"
    );

    let squeezed = Rect::from_min_size(Pos2::ZERO, Vec2::new(306.0, 300.0));
    let splitter = layout_surface(&state, state.main(), squeezed, 6.0).splitters[0];
    assert_eq!(
        splitter.size,
        SplitSize::Second(180.0),
        "a clamped splitter reports the points it is drawn at, so a drag starts from there"
    );
}
