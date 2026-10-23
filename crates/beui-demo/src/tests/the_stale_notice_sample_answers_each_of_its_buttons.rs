use super::*;

#[test]
fn the_stale_notice_sample_answers_each_of_its_buttons() {
    let mut test = alone(WIDE, Page::Overlays);
    test.frame(Vec::new());
    for _ in 0..REVEAL_TICKS {
        let bottom = test.size().y;
        if test.shows("stale-notice.try") && test.rect_of("stale-notice.try").bottom() <= bottom {
            break;
        }
        test.scroll(
            pos2(WIDE.x / 2.0, bottom / 2.0),
            Vec2::new(0.0, REVEAL_STEP),
        );
    }
    assert!(test.shows("stale-notice"));
    test.snapshot("a_block_saved_by_another_build");

    for (button, outcome) in [
        ("stale-notice.reset", "Reset to empty"),
        ("stale-notice.delete", "Deleted"),
        ("stale-notice.try", "Opened anyway"),
    ] {
        test.click(button);
        test.frame(Vec::new());
        let root = test.document().root().expect("the demo has a root");
        assert_eq!(showing(test.document(), root, outcome), 1, "{button}");
    }
}
