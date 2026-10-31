use block_editor_beui::beui::{PointerButton, pos2, vec2};

use super::*;

fn mostly_on(window: Rect, display: Rect, other: Rect) -> bool {
    let shared = |display: Rect| {
        let shared = display.intersect(window);
        shared.width().max(0.0) * shared.height().max(0.0)
    };
    shared(display) > shared(other)
}

#[test]
fn a_window_moved_to_another_display_lists_its_chip_on_that_displays_bar() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(960.0, 640.0));
    let right = Rect::from_min_size(pos2(960.0, 160.0), vec2(640.0, 480.0));
    let mut fixture = Fixture::new();
    fixture.on_two_displays((left, 1.0), (right, 1.0));
    fixture.show_windows(&[host_window(1, true)]);
    let chips = |fixture: &Fixture| fixture.test.desk_rects_of("desktop.windows.1");
    assert!(
        matches!(chips(&fixture)[..], [chip] if left.contains_rect(chip)),
        "a window on the first display has its chip on that display's bar alone: {:?}",
        chips(&fixture)
    );

    fixture
        .test
        .click_with("desktop.windows.1", PointerButton::Secondary);
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.windows.1.display.0"),
        "the menu offers no move to the display the window is on"
    );
    fixture
        .test
        .desk_snapshot("a_window_chips_menu_offers_the_other_display", 0);
    let was = fixture.test.desk_windows()[0];
    fixture.test.click("desktop.windows.1.display.1");
    fixture.settle();
    let moved = fixture.test.desk_windows()[0];
    assert!(
        right.contains_rect(moved),
        "the menu moves the window onto the second display: {moved:?}"
    );
    assert_eq!(moved.size(), was.size(), "at the size it was");
    assert!(
        matches!(chips(&fixture)[..], [chip] if right.contains_rect(chip)),
        "and its chip with it: {:?}",
        chips(&fixture)
    );

    let title = fixture.desk_text("notes-1.txt");
    let grab = pos2(title.max.x + 80.0, title.center().y);
    fixture.test.desk_drag(grab, grab - vec2(700.0, 100.0), 8);
    fixture.settle();
    let dragged = fixture.test.desk_windows()[0];
    assert!(
        mostly_on(dragged, left, right),
        "dragged back, the window is mostly on the first display: {dragged:?}"
    );
    assert!(
        matches!(chips(&fixture)[..], [chip] if left.contains_rect(chip)),
        "and the dock heard where the host put it, so its chip follows: {:?}",
        chips(&fixture)
    );
}
