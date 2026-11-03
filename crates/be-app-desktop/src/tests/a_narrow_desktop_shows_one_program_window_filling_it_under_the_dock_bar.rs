use be_sdk_beui::beui::Vec2;

use super::*;

const PHONE: Vec2 = Vec2::new(360.0, 640.0);
const LONG: &str = "a-report-with-a-name-far-too-long-for-a-phones-bar.txt";

#[test]
fn a_narrow_desktop_shows_one_program_window_filling_it_under_the_dock_bar() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    fixture.test.set_size(PHONE);
    fixture.settle();
    let long = HostWindow {
        title: LONG.to_owned(),
        ..host_window(2, true)
    };
    fixture.show_windows(&[host_window(1, false), long]);

    assert!(
        fixture.test.shown("dock.switch"),
        "a desktop narrower than a tablet shows one window at a time"
    );
    let bar = fixture.test.rect_of("dock.switch");
    let band = fixture.test.rect_of("dock.bar.band.bottom");
    let shown = fixture
        .window_rect(2)
        .expect("the focused window's surface is placed");
    assert!(
        shown.width() >= PHONE.x - 1.0
            && shown.top() >= bar.bottom() - 0.5
            && shown.bottom() <= band.top() + 0.5
            && shown.height() > PHONE.y / 2.0,
        "the window fills the desktop between the dock bar and the bars' band: {shown:?}"
    );
    assert!(
        fixture
            .window_rect(1)
            .is_none_or(|hidden| !hidden.is_positive()),
        "the other window waits behind it"
    );

    let document = fixture.test.document();
    let title = document
        .root()
        .and_then(|root| text_within(&document, root, &format!("Kate \u{2014} {LONG}")))
        .and_then(|title| document.node_rect(title))
        .expect("the bar names the window");
    drop(document);
    assert!(
        title.right() <= bar.left() + 0.5,
        "the long title ends before the switch button, at {title:?} and {bar:?}"
    );
    fixture
        .test
        .snapshot("a_narrow_desktop_shows_one_program_window_filling_it");
}
