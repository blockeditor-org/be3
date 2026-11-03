use be_sdk_beui::be_block::ViewState;
use be_sdk_beui::beui::pos2;

use super::*;

const FIRST: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 600.0));
const SECOND: Rect = Rect::from_min_max(pos2(800.0, 0.0), pos2(1600.0, 600.0));
const STATE: &str = "display-bars-by-monitor";

fn holding(bars: &ViewState) -> EditorViewContent {
    let mut saved = EditorView::document(None);
    let edit = saved.root().set_state(STATE, Some(bars), 1, Uuid::nil());
    saved.apply(&edit);
    saved
}

fn status_down_the_left(fixture: &Fixture) -> bool {
    matches!(
        fixture.shown_on("desktop.bar.status", SECOND)[..],
        [bar] if bar.left() == SECOND.left() && bar.height() > bar.width()
    )
}

fn saved_bars(fixture: &Fixture) -> Option<ViewState> {
    fixture.saved().root().state(STATE).cloned()
}

#[test]
fn bars_of_other_displays_that_cannot_be_read_are_kept_until_reset() {
    let unreadable = ViewState {
        bytes: vec![0xff; 7],
        refs: Vec::new(),
    };
    let mut fixture = Fixture::holding(holding(&unreadable));
    fixture.on_monitors(&[("built-in", FIRST), ("dell", SECOND)]);
    assert!(
        fixture.test.shown("desktop.display-bars.reset"),
        "the desktop says the bars could not be read"
    );
    assert!(
        !status_down_the_left(&fixture),
        "the second display starts from the first display's bars"
    );
    fixture
        .test
        .desk_snapshot("bars_of_other_displays_that_cannot_be_read_are_kept_until_reset", 0);

    fixture.test.click("desktop.display-bars.keep");
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.display-bars.reset"),
        "keeping them closes the notice"
    );
    fixture.display_bar_menu(1, STATUS_BAR, SECOND, &["beside.ends", "beside.left.top"]);
    assert!(status_down_the_left(&fixture), "a bar still moves");
    assert_eq!(
        saved_bars(&fixture),
        Some(unreadable.clone()),
        "but nothing is saved over the bytes it could not read"
    );

    let mut fixture = Fixture::resumed(fixture.saved());
    fixture.on_monitors(&[("built-in", FIRST), ("dell", SECOND)]);
    assert!(
        fixture.test.shown("desktop.display-bars.reset"),
        "the notice comes back while the bytes are kept"
    );
    fixture.test.click("desktop.display-bars.reset");
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.display-bars.reset"),
        "resetting closes the notice"
    );
    fixture.display_bar_menu(1, STATUS_BAR, SECOND, &["beside.ends", "beside.left.top"]);
    assert_ne!(
        saved_bars(&fixture),
        Some(unreadable),
        "from then on the bars are saved"
    );

    let mut fixture = Fixture::resumed(fixture.saved());
    fixture.on_monitors(&[("built-in", FIRST), ("dell", SECOND)]);
    assert!(
        !fixture.test.shown("desktop.display-bars.reset") && status_down_the_left(&fixture),
        "and read back"
    );
}
