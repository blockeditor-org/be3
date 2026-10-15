use super::*;

#[test]
fn a_desktop_started_again_adopts_live_windows_instead_of_placeholders() {
    let mut before = Fixture::new();
    before.settle();
    before.show_windows(&[foot(1), notes(2)]);
    let places = [1, 2].map(|id| before.window_rect(id).expect("the window is placed"));

    let mut again = Fixture::holding(before.saved());
    again.show_windows(&[foot(1), notes(2)]);
    again.settle();
    for (id, place) in [1, 2].into_iter().zip(places) {
        assert!(
            again.placeholder_rect(id).is_none(),
            "a window still open is not shown as a placeholder too"
        );
        assert_eq!(
            again.window_rect(id),
            Some(place),
            "window {id} is back in place"
        );
    }
    assert!(!again.test.shown("desktop.placeholders.open_all"));

    let mut late = Fixture::resumed(before.saved());
    assert!(late.placeholder_rect(1).is_some());
    late.show_windows(&[foot(1), notes(2)]);
    for (id, place) in [1, 2].into_iter().zip(places) {
        assert!(
            late.placeholder_rect(id).is_none(),
            "windows the host still had replace their placeholders once they are listed"
        );
        assert_eq!(late.window_rect(id).map(|rect| rect.min), Some(place.min));
    }
    let placed = late
        .test
        .children()
        .iter()
        .filter(|placement| matches!(placement.content, ChildContent::Window(_)))
        .count();
    assert_eq!(placed, 2, "and no window is shown twice");
}
