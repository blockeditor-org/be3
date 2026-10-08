use super::*;
use beui::{Event, Key, Modifiers};

fn press(test: &mut DocumentTest, key: Key) {
    test.frame(
        [true, false]
            .into_iter()
            .map(|pressed| Event::Key {
                key,
                pressed,
                repeat: false,
                modifiers: Modifiers::NONE,
            })
            .collect(),
    );
    test.frame(Vec::new());
}

#[test]
fn the_keep_changes_sample_keeps_on_enter_and_reverts_on_escape() {
    let mut test = alone(WIDE, Page::Overlays);
    let root = test.document().root().expect("the demo built a root");

    test.click("demo.keep_changes.open");
    test.frame(Vec::new());
    press(&mut test, Key::Enter);
    assert!(!test.shows("keep-changes.0"), "Enter closes the prompt");
    assert_eq!(
        showing(test.document(), root, "Kept 1920 × 1080 at 144 Hz"),
        1,
        "Enter keeps"
    );

    test.click("demo.keep_changes.open");
    test.frame(Vec::new());
    press(&mut test, Key::Escape);
    assert!(!test.shows("keep-changes.0"), "Escape closes the prompt");
    assert_eq!(
        showing(test.document(), root, "Went back to 1920 × 1080 at 60 Hz"),
        1,
        "Escape reverts"
    );
}
