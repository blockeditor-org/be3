use super::*;

#[test]
fn a_split_follows_the_code_until_the_user_moves_it() {
    let mut state = DockState::default();
    let layout = |fraction: f32| {
        spec(split(
            "root",
            Direction::Horizontal,
            fraction,
            pane("left", &[1]),
            pane("right", &[2]),
        ))
    };
    state.reconcile(&layout(0.3));
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1006.0, 400.0));
    let split = layout_surface(&state, state.main(), area, 6.0).splitters[0].id;

    state.reconcile(&layout(0.4));
    assert_eq!(
        state.split_fraction(split),
        0.4,
        "a split nobody moved follows the fraction in the code"
    );

    state.set_split_fraction(split, 0.6);
    state.reconcile(&layout(0.45));
    assert_eq!(
        state.split_fraction(split),
        0.6,
        "once the user moves a split it stays where they put it"
    );
}
