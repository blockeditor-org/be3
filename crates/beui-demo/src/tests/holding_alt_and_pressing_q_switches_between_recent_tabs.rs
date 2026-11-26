use super::*;
use beui::{Key, Modifiers};

const TEXT_PAGE: &str = "demo.code.Text styles";
const BUTTONS_PAGE: &str = "demo.code.Buttons";

fn press(key: Key) -> Vec<Input> {
    [true, false]
        .into_iter()
        .map(|pressed| {
            Input::Key(beui::Keystroke {
                key: Some(key),
                pressed,
                ..Default::default()
            })
        })
        .collect()
}

#[test]
fn holding_alt_and_pressing_q_switches_between_recent_tabs() {
    let mut test = demo(WIDE);
    open(&mut test, Page::Text);
    open_beside(&mut test, Page::Buttons);
    assert!(test.shown(BUTTONS_PAGE) && !test.shown(TEXT_PAGE));
    let alt = Modifiers::ALT;

    test.frame(vec![Input::Modifiers(alt)]);
    test.frame(press(Key::Q));
    test.frame(press(Key::Escape));
    test.frame(vec![Input::Modifiers(Modifiers::NONE)]);
    test.frame(Vec::new());
    assert!(!test.shown("dock.switch"), "Escape closes the switcher");
    assert!(
        test.shown(BUTTONS_PAGE),
        "and leaves the page that was shown"
    );

    test.frame(vec![Input::Modifiers(alt)]);
    test.frame(press(Key::Q));
    test.frame(press(Key::Q));
    test.frame(Vec::new());
    assert!(
        test.shown(&format!("dock.switch.{}", tab(Page::Text))),
        "holding Alt and pressing Q lists the tabs over the dock"
    );
    test.snapshot("docking_switching_tabs");
    assert!(
        test.shown(BUTTONS_PAGE),
        "nothing is shown while Alt is held"
    );

    test.frame(vec![Input::Modifiers(Modifiers::NONE)]);
    test.frame(Vec::new());
    assert!(!test.shown("dock.switch"), "letting go of Alt closes it");
    assert!(
        test.shown(TEXT_PAGE) && !test.shown(BUTTONS_PAGE),
        "and shows the tab two back, past the catalog that was clicked in between"
    );
}
