use super::*;
use beui::{Event, Key, Modifiers};

fn key(key: Key) -> Vec<Event> {
    [true, false]
        .into_iter()
        .map(|pressed| Event::Key {
            key,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        })
        .collect()
}

#[test]
fn the_runs_toggle_shows_where_each_side_typed_and_deleted() {
    let mut test = alone(WIDE, Page::Collaboration);
    let left = test.rect_of("demo.collaboration.left");
    test.click_at(pos2(left.right() - 8.0, left.top() + 150.0));
    test.frame(vec![Event::Text(", left was here".to_owned())]);
    for event in key(Key::Backspace) {
        test.frame(vec![event]);
    }
    let right = test.rect_of("demo.collaboration.right");
    test.click_at(pos2(right.right() - 8.0, right.top() + 30.0));
    test.frame(vec![Event::Text(" from the right".to_owned())]);
    test.click("demo.collaboration.pause");
    test.frame(Vec::new());
    test.click("demo.collaboration.all_left");
    test.click("demo.collaboration.all_right");
    test.frame(Vec::new());

    let root = test.document().root().expect("the demo built a root");
    assert_eq!(
        showing(test.document(), root, "@0+0"),
        0,
        "runs stay hidden until they are asked for"
    );
    test.click("demo.collaboration.runs");
    test.frame(Vec::new());
    for (label, why) in [
        ("@0+0", "the loaded text starts a run"),
        ("@1+0", "the left side's typing is a run"),
        ("@2+0", "the right side's typing is a run"),
    ] {
        assert_eq!(
            showing(test.document(), root, label),
            2,
            "{why} on both sides"
        );
    }
    test.snapshot("collaboration_runs");
}
