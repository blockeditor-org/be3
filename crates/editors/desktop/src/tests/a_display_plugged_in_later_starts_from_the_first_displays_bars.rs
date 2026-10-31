use block_editor_beui::beui::{pos2, vec2};

use super::*;

#[test]
fn a_display_plugged_in_later_starts_from_the_first_displays_bars() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let right = Rect::from_min_size(pos2(800.0, 0.0), vec2(800.0, 600.0));
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.bar_menu(APPS_BAR, &["hide"]);
    fixture.bar_menu(STATUS_BAR, &["beside.top"]);
    fixture.on_two_displays((left, 1.0), (right, 1.0));
    assert!(
        fixture.test.desk_rects_of("desktop.bar.apps").is_empty(),
        "the new display hides the Apps bar the first one hides"
    );
    let status = fixture.shown_on("desktop.bar.status", right);
    assert!(
        matches!(status[..], [bar] if bar.top() == right.top()),
        "and puts its Status bar along the top as the first one does: {status:?}"
    );

    fixture.display_bar_menu(1, STATUS_BAR, right, &["beside.bottom"]);
    let first = fixture.shown_on("desktop.bar.status", left);
    assert!(
        matches!(first[..], [bar] if bar.top() == left.top()),
        "from then on the new display keeps its own: {first:?}"
    );
}
