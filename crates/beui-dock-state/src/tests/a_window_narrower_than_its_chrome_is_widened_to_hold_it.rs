use super::*;

#[test]
fn a_window_narrower_than_its_chrome_is_widened_to_hold_it() {
    let mut state = DockState::new([TabId::new(1)]);
    let window = state.open_window(
        Rect::from_min_size(Pos2::new(40.0, 40.0), Vec2::new(150.0, 100.0)),
        vec![TabId::new(2)],
    );
    assert_eq!(
        state.window_rect(window),
        Some(Rect::from_min_size(
            Pos2::new(40.0, 40.0),
            Vec2::new(MIN_WINDOW_SIZE.x, 100.0)
        )),
        "a window opened too narrow for its bar is framed at the least width, keeping its height"
    );

    state.set_window_rect(
        window,
        Rect::from_min_size(Pos2::new(60.0, 50.0), Vec2::new(90.0, 300.0)),
    );
    assert_eq!(
        state.window_rect(window).map(|rect| rect.width()),
        Some(MIN_WINDOW_SIZE.x),
        "and stays that wide when its tab asks for a narrower one"
    );
}
