use block_editor_beui::{ChildContent, HostPanel};

use super::*;

#[test]
fn the_desktop_starts_with_nothing_open_but_its_bar() {
    let mut fixture = Fixture::new();
    fixture.settle();

    assert!(fixture.placed_blocks().is_empty(), "no window is open");
    let document = fixture.test.document();
    for bar in [
        "desktop.bar.apps",
        "desktop.bar.windows",
        "desktop.bar.status",
    ] {
        assert!(
            document.find_test_id(bar).is_some(),
            "the desktop shows {bar}"
        );
    }
    let clock = document
        .find_test_id("desktop.clock")
        .and_then(|clock| text_under(&document, clock))
        .expect("the bar shows the time");
    assert!(
        clock.len() == 5 && clock.as_bytes()[2] == b':',
        "the time reads as hours and minutes, not {clock:?}"
    );

    drop(document);
    fixture.test.click("desktop.apps");
    fixture.settle();
    fixture.test.click("desktop.apps.menu");
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.apps.panel"),
        "the Apps panel closes for the menu"
    );
    assert!(
        fixture
            .test
            .placed(|content| *content == ChildContent::Host(HostPanel::AppMenu))
            .is_some(),
        "the Apps panel's menu row places the app's menu in a view it anchored"
    );
}
