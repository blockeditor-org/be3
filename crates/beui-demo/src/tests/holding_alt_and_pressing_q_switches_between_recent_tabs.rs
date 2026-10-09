use super::*;
use beui::{Event, Key, Modifiers};

const TEXT_PAGE: &str = "demo.code.Type scale";
const BUTTONS_PAGE: &str = "demo.code.Buttons";

fn press(key: Key, modifiers: Modifiers) -> Vec<Event> {
    [true, false]
        .into_iter()
        .map(|pressed| Event::Key {
            key,
            pressed,
            repeat: false,
            modifiers,
        })
        .collect()
}

#[test]
fn holding_alt_and_pressing_q_switches_between_recent_tabs() {
    let mut test = demo(WIDE);
    open(&mut test, Page::Text);
    open(&mut test, Page::Buttons);
    assert!(test.shows(BUTTONS_PAGE) && !test.shows(TEXT_PAGE));
    let alt = Modifiers::ALT;

    test.frame(vec![Event::Modifiers(alt)]);
    test.frame(press(Key::Q, alt));
    test.frame(press(Key::Escape, alt));
    test.frame(vec![Event::Modifiers(Modifiers::NONE)]);
    test.frame(Vec::new());
    assert!(!test.shows("dock.switch"), "Escape closes the switcher");
    assert!(
        test.shows(BUTTONS_PAGE),
        "and leaves the page that was shown"
    );

    test.frame(vec![Event::Modifiers(alt)]);
    test.frame(press(Key::Q, alt));
    test.frame(press(Key::Q, alt));
    test.frame(Vec::new());
    assert!(
        test.shows(&format!("dock.switch.{}", Page::Text.tab().value())),
        "holding Alt and pressing Q lists the tabs over the dock"
    );
    test.snapshot("docking_switching_tabs");
    assert!(
        test.shows(BUTTONS_PAGE),
        "nothing is shown while Alt is held"
    );

    test.frame(vec![Event::Modifiers(Modifiers::NONE)]);
    test.frame(Vec::new());
    assert!(!test.shows("dock.switch"), "letting go of Alt closes it");
    assert!(
        test.shows(TEXT_PAGE) && !test.shows(BUTTONS_PAGE),
        "and shows the tab two back, past the catalog that was clicked in between"
    );
}
