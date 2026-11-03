use be_sdk_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn a_tiny_program_window_is_framed_wide_enough_for_its_tab() {
    let mut fixture = Fixture::new();
    let display = Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 720.0));
    fixture.test.show_on_displays(&[(display, 1.0)]);
    fixture.settle();
    let eyes = HostWindow {
        title: "Choose a colour for the desktop background".to_owned(),
        program_id: "xeyes".to_owned(),
        name: "xeyes".to_owned(),
        size: Vec2::new(150.0, 100.0),
        ..host_window(1, true)
    };
    fixture.show_windows(&[eyes]);
    fixture.settle();
    let shown = fixture.test.desk_windows();
    assert_eq!(shown.len(), 1, "the window is open: {shown:?}");
    let window = shown[0];
    let closes: Vec<Rect> = fixture
        .test
        .test_ids()
        .iter()
        .map(|(id, _)| id.to_owned())
        .filter(|id| id.starts_with("dock.") && id.ends_with(".close"))
        .flat_map(|id| fixture.test.desk_rects_of(&id))
        .collect();
    assert_eq!(
        closes.len(),
        1,
        "the window has one close button: {closes:?}"
    );
    let close = closes[0];
    assert!(
        close.min.x >= window.min.x && close.max.x <= window.max.x,
        "the close button is inside the window: {close:?} in {window:?}"
    );
    fixture
        .test
        .desk_snapshot("a_tiny_program_window_is_framed_wide_enough_for_its_tab", 0);
}
