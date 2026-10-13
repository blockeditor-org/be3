use super::*;

#[test]
fn the_message_toast_sample_offers_its_actions() {
    let mut test = alone(WIDE, Page::Overlays);
    test.frame(Vec::new());
    for _ in 0..REVEAL_TICKS {
        let bottom = test.size().y;
        if test.shows("toast.3") && test.rect_of("toast.3").bottom() <= bottom {
            break;
        }
        test.scroll_at(
            pos2(WIDE.x / 2.0, bottom / 2.0),
            Vec2::new(0.0, -REVEAL_STEP),
        );
    }
    assert!(test.shows("toast.3.action.reply"));
    assert!(test.shows("toast.3.activate"));
    test.snapshot("toast_with_actions");

    test.click("toast.3.action.reply");
    test.frame(Vec::new());
    let root = test.document().root().expect("the demo has a root");
    assert_eq!(
        showing(test.document(), root, "Chose reply on message 3"),
        1
    );
    assert!(!test.shows("toast.3"), "answering a toast takes it away");
}
