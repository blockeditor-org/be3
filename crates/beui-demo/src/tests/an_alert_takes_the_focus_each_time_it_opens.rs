use super::*;
use beui::Key;

#[test]
fn an_alert_takes_the_focus_each_time_it_opens() {
    let mut test = alone(WIDE, Page::Overlays);
    reveal(&mut test, "demo.discard.open");
    for opening in ["first", "second"] {
        test.click("demo.discard.open");
        test.settle();
        assert!(
            focused(&test, "demo.discard.cancel"),
            "the {opening} time it opens, the alert focuses Keep: it keeps its body while closed, so it asks again"
        );
        press_key(&mut test, Key::Escape);
        test.settle();
        assert!(
            !test.shown("demo.discard.cancel"),
            "Escape closes it the {opening} time"
        );
    }
}
