use block_editor_beui::beui::{PointerButton, pos2, vec2};

use super::*;

#[test]
fn a_windows_tab_menu_shows_whole_past_its_window() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.show_windows(&[HostWindow {
        size: Vec2::new(150.0, 90.0),
        ..host_window(1, true)
    }]);
    fixture.test.show_on_displays(&[(
        Rect::from_min_size(pos2(0.0, 0.0), vec2(960.0, 640.0)),
        1.0,
    )]);
    fixture.settle();
    let window = fixture.test.desk_windows()[0];
    let title = fixture.desk_text("Program 1");
    fixture
        .test
        .click_with(title.center(), PointerButton::Secondary);
    fixture.settle();
    let row = fixture
        .test
        .desk_rects_of("dock.tab.pop-out")
        .first()
        .copied()
        .expect("the tab's menu opens");
    assert!(
        !window.contains_rect(row),
        "the menu is wider than the small window and shows past its edge: {row:?} {window:?}"
    );
    fixture
        .test
        .desk_snapshot("a_windows_tab_menu_past_its_window", 0);

    let full = fixture.desk_text("Full screen");
    fixture.test.click(full.center());
    fixture.settle();
    assert!(
        fixture
            .test
            .host_calls(block_ui_test::requested::window)
            .contains(&WindowAction::Fullscreen {
                window: HostWindowId(1),
                fullscreen: true
            }),
        "a click on the menu where it shows past the window reaches it"
    );
}
