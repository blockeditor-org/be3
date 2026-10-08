use super::*;
use beui::Event;

#[test]
fn paused_edits_wait_for_a_send_and_carry_presence() {
    let mut test = alone(WIDE, Page::Collaboration);
    test.click("demo.collaboration.pause");
    test.frame(Vec::new());

    let left = test.rect_of("demo.collaboration.left");
    test.click_at(pos2(left.right() - 8.0, left.top() + 150.0));
    test.frame(vec![Event::Text(", left was here".to_owned())]);
    let right = test.rect_of("demo.collaboration.right");
    test.click_at(pos2(right.right() - 8.0, right.top() + 30.0));
    test.frame(vec![Event::Text(" from the right".to_owned())]);
    test.frame(Vec::new());
    let root = test.document().root().expect("the demo built a root");
    assert_eq!(
        showing(
            test.document(),
            root,
            "In flight: 1 to the right, 1 to the left."
        ),
        1,
        "the right side's edit waits while the network is paused"
    );

    test.click("demo.collaboration.one_left");
    test.frame(Vec::new());
    assert_eq!(
        showing(
            test.document(),
            root,
            "In flight: 2 to the right, 0 to the left."
        ),
        1,
        "sending delivers the right side's edit"
    );
    test.snapshot("collaboration_paused");
}
