use super::*;

#[test]
fn a_programs_dialog_that_resizes_itself_still_points_at_its_windows_tab() {
    let mut fixture = Fixture::new();
    fixture.settle();
    let dialog = |size: Vec2| HostWindow {
        title: "Preferences".to_owned(),
        parent: Some(HostWindowId(1)),
        size,
        ..host_window(2, false)
    };
    fixture.show_windows(&[host_window(1, false), dialog(Vec2::new(320.0, 200.0))]);
    fixture.show_windows(&[host_window(1, false), dialog(Vec2::new(420.0, 260.0))]);

    let placed = fixture.window_rect(2).expect("the dialog is placed");
    assert_eq!(
        (placed.width(), placed.height()),
        (420.0, 260.0),
        "the dialog follows the size its program chose"
    );
    let tab = fixture.test.rect_of(&format!("dock.tab.{}", window_tab(1)));
    assert!(
        placed.top() > tab.bottom()
            && placed.left() <= tab.center().x
            && tab.center().x <= placed.right(),
        "and still hangs from its parent's tab: {placed:?} below {tab:?}"
    );
    fixture
        .test
        .snapshot("a_programs_dialog_that_resizes_itself_still_points_at_its_windows_tab");
}
