use super::*;

#[test]
fn focusing_a_window_raises_it_over_the_windows_it_overlaps() {
    let mut state = DockState::new([TabId::new(1)]);
    let rect = Rect::from_min_size(Pos2::ZERO, FLOATING_SIZE);
    let first = state.open_window(rect, vec![TabId::new(2)]);
    let second = state.open_window(rect, vec![TabId::new(3)]);
    assert_eq!(
        state.surfaces(),
        vec![state.main(), first, second],
        "a window opens above the ones already open"
    );

    state.show(TabId::new(2));

    assert_eq!(
        state.surfaces(),
        vec![state.main(), second, first],
        "showing a tab raises the window it is in"
    );
    assert_eq!(
        state.main(),
        state.surfaces()[0],
        "the main surface stays underneath whatever is raised"
    );
}
