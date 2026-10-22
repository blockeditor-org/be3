use super::*;

const SAME: &str = "Both sides hold the same text and styles.";

#[test]
fn rich_text_styled_on_both_sides_converges_once_sent_both_ways() {
    let mut test = alone(WIDE, Page::RichText);
    let margin = pos2(test.size().x - 6.0, test.size().y / 2.0);
    for _ in 0..REVEAL_TICKS {
        if test.rect_of("demo.collaboration.right").bottom() <= test.size().y {
            break;
        }
        test.scroll(margin, Vec2::new(0.0, REVEAL_STEP));
    }
    test.click("demo.collaboration.pause");
    test.frame(Vec::new());

    let left = test.rect_of("demo.collaboration.left");
    select_line(&mut test, pos2(left.left() + 40.0, left.top() + 60.0));
    test.click("demo.collaboration.left.italic");
    test.frame(Vec::new());
    let right = test.rect_of("demo.collaboration.right");
    select_line(&mut test, pos2(right.left() + 40.0, right.top() + 60.0));
    test.click("demo.collaboration.right.strikethrough");
    test.frame(Vec::new());
    let root = test.document().root().expect("the demo built a root");
    assert_eq!(
        showing(test.document(), root, SAME),
        0,
        "each side holds only its own style while the network is paused"
    );

    test.click("demo.collaboration.all_left");
    test.frame(Vec::new());
    test.click("demo.collaboration.all_right");
    test.frame(Vec::new());
    assert_eq!(
        showing(test.document(), root, SAME),
        1,
        "both styles reach both sides"
    );
    test.snapshot("rich_text_collaboration_converged");
}
