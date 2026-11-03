use super::*;

fn act(test: &mut DocumentTest, action: &str) {
    test.click("wm.menu");
    test.settle();
    test.click(&format!("wm.menu.dock.window.{action}"));
    test.settle();
}

#[test]
fn the_window_menu_cycles_a_groups_windows_hides_them_and_moves_one_to_the_other_area() {
    let mut test = alone(WIDE, Page::WindowManager);
    test.settle();
    for window in 1..=3 {
        assert!(test.shows(&format!("wm.window.{window}")));
    }

    test.click("wm.window.1");
    test.settle();
    act(&mut test, "next_of_group");
    act(&mut test, "hide");
    assert!(
        !test.shows("wm.window.2") && test.shows("wm.window.1"),
        "the next window of the notes' group took the focus, so it is the one hidden"
    );

    act(&mut test, "show_desktop");
    assert!(
        !test.shows("wm.window.1") && !test.shows("wm.window.3"),
        "show desktop hides every window"
    );
    act(&mut test, "show_desktop");
    assert!(
        test.shows("wm.window.1") && test.shows("wm.window.3") && !test.shows("wm.window.2"),
        "and a second time brings back what it hid"
    );

    test.click("wm.window.3");
    test.settle();
    act(&mut test, "next_area");
    act(&mut test, "snap_left");
    test.snapshot("the_window_menu_moved_the_sketch_to_the_other_area_and_snapped_it");

    act(&mut test, "close");
    assert!(
        !test.shows("wm.window.3"),
        "close goes through the tab's own close"
    );
}
