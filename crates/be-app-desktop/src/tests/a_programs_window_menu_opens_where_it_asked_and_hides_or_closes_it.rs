use super::*;

#[test]
fn a_programs_window_menu_opens_where_it_asked_and_hides_or_closes_it() {
    let mut fixture = Fixture::with_windows(&[1]);
    let shown = fixture.window_rect(1).expect("the window is shown");
    let asked = |asked: u64| TestWindow {
        menu: Some(WindowMenu {
            asked,
            at: Vec2::new(40.0, 12.0),
        }),
        ..host_window(1, true)
    };
    fixture.show_windows(&[asked(1)]);
    assert!(fixture.test.shown("workspace.window.menu.hide"));
    assert!(fixture.test.shown("workspace.window.menu.close"));
    let menu = fixture.test.rect_of("workspace.window.menu.hide");
    let point = shown.min + Vec2::new(40.0, 12.0);
    assert!(
        (menu.min - point).length() < 12.0,
        "the menu opens at the point the program asked, {menu:?} for {point:?}"
    );
    fixture.test.snapshot("a_programs_window_menu");
    fixture.test.click("workspace.window.menu.hide");
    fixture.settle();
    assert!(fixture.window_rect(1).is_none(), "Hide hides the window");
    fixture.test.click("desktop.windows.1");
    fixture.settle();
    assert!(fixture.window_rect(1).is_some());

    fixture.show_windows(&[asked(1)]);
    assert!(
        !fixture.test.shown("workspace.window.menu.hide"),
        "an ask already answered does not open the menu again"
    );
    fixture.show_windows(&[asked(2)]);
    fixture.test.click("workspace.window.menu.close");
    fixture.settle();
    assert_eq!(
        fixture.closed_windows(),
        [HostWindowId(1)],
        "Close asks the program to close"
    );
}
