use super::*;
use beui::{Event, Key, Modifiers};

#[test]
fn the_overlay_sample_closes_on_escape_and_a_click_outside() {
    let mut test = demo(WIDE);
    open(&mut test, Page::Layering);

    test.click("demo.overlay.trigger");
    assert!(
        test.shows("demo.overlay.panel"),
        "the trigger opens the overlay"
    );

    test.frame(
        [true, false]
            .into_iter()
            .map(|pressed| Event::Key {
                key: Key::Escape,
                pressed,
                repeat: false,
                modifiers: Modifiers::NONE,
            })
            .collect(),
    );
    test.frame(Vec::new());
    assert!(
        !test.shows("demo.overlay.panel"),
        "Escape closes the overlay"
    );

    test.click("demo.overlay.trigger");
    assert!(
        test.shows("demo.overlay.panel"),
        "the trigger opens it again"
    );
    test.click_at(over_the_page(WIDE));
    test.frame(Vec::new());
    assert!(
        !test.shows("demo.overlay.panel"),
        "a click outside closes the overlay"
    );
}
