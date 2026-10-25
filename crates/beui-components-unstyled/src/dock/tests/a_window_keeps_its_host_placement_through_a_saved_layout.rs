use super::*;
use beui_core::window::HostPlacement;

#[test]
fn a_window_keeps_its_host_placement_through_a_saved_layout() {
    let mut state = DockState::new([TabId::new(1)]);
    let window = state.open_window(
        Rect::from_min_size(pos2(40.0, 40.0), vec2(300.0, 200.0)),
        vec![TabId::new(2)],
    );
    let placed = HostPlacement("space 0.5 1.6 -1 0 0 0 1 0.001 0.4".to_owned());
    state.set_window_placement(window, placed.clone());
    let main = state.surfaces()[0];
    state.set_window_placement(main, placed.clone());

    let saved = postcard::to_stdvec(&state).expect("the state serialises");
    let restored: DockState = postcard::from_bytes(&saved).expect("the state reads back");
    assert_eq!(
        restored.window_placement(window),
        Some(placed),
        "a floating window's host placement is saved with the layout"
    );
    assert_eq!(
        restored.window_placement(main),
        None,
        "a surface that is not a window keeps no placement"
    );
}
