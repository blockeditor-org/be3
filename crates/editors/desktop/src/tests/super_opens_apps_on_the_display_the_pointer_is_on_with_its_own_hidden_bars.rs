use block_editor_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn super_opens_apps_on_the_display_the_pointer_is_on_with_its_own_hidden_bars() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let right = Rect::from_min_size(pos2(800.0, 0.0), vec2(800.0, 600.0));
    let mut fixture = Fixture::with_windows(&[1]);
    fixture.on_two_displays((left, 1.0), (right, 1.0));
    fixture.display_bar_menu(1, APPS_BAR, right, &["hide"]);
    assert!(
        fixture.shown_on("desktop.apps", right).is_empty()
            && fixture.shown_on("desktop.apps", left).len() == 1,
        "the Apps bar is hidden on the second display alone"
    );

    let focus_the_first = |fixture: &mut Fixture| {
        fixture.desk_click("desktop.apps", left);
        fixture.test.key(Key::Escape, Modifiers::NONE);
        fixture.settle();
    };
    focus_the_first(&mut fixture);
    fixture.test.desk_move(right.center());
    assert!(fixture.test.app_tap(Key::Logo));
    fixture.settle();
    let panel = fixture.shown_on("desktop.apps.panel", right);
    let band = fixture.shown_on("dock.bar.band.bottom", right);
    assert!(
        matches!((&panel[..], &band[..]), ([panel], [band]) if (panel.center().x - right.center().x).abs() < 1.0 && (panel.center().y - band.top() / 2.0).abs() < 1.0),
        "Super opens Apps centred over the windows' area of the display the pointer is on: {panel:?} {band:?}"
    );
    fixture
        .test
        .desk_snapshot("apps_opened_by_super_on_the_second_display", 1);
    fixture.desk_click("desktop.apps.bars", right);
    assert_eq!(
        fixture.shown_on("desktop.apps", right).len(),
        1,
        "the panel shows that display's bars again"
    );

    fixture.display_bar_menu(0, WINDOWS_BAR, left, &["hide"]);
    focus_the_first(&mut fixture);
    fixture.test.desk_move(right.center());
    assert!(fixture.test.app_tap(Key::Logo));
    fixture.settle();
    assert!(
        fixture.shown_on("desktop.apps.panel", right).len() == 1
            && fixture.test.desk_rects_of("desktop.apps.bars").is_empty(),
        "a display offers only its own hidden bars"
    );
    focus_the_first(&mut fixture);
    fixture.test.desk_move(left.center());
    assert!(fixture.test.app_tap(Key::Logo));
    fixture.settle();
    assert!(
        fixture.shown_on("desktop.apps.panel", left).len() == 1
            && fixture.shown_on("desktop.apps.bars", left).len() == 1,
        "and the first display offers its own: {:?}",
        fixture.test.desk_rects_of("desktop.apps.panel")
    );
}
