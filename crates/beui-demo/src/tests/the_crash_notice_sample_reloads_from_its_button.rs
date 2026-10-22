use super::*;

#[test]
fn the_crash_notice_sample_reloads_from_its_button() {
    let mut test = alone(WIDE, Page::Overlays);
    test.frame(Vec::new());
    for _ in 0..REVEAL_TICKS {
        let bottom = test.size().y;
        if test.shows("crash-notice.reload")
            && test.rect_of("crash-notice.reload").bottom() <= bottom
        {
            break;
        }
        test.scroll(
            pos2(WIDE.x / 2.0, bottom / 2.0),
            Vec2::new(0.0, REVEAL_STEP),
        );
    }
    assert!(test.shows("crash-notice"));
    test.snapshot("a_crashed_editor");

    test.click("crash-notice.reload");
    test.frame(Vec::new());

    let root = test.document().root().expect("the demo has a root");
    assert_eq!(
        showing(test.document(), root, "It stopped again after a reload."),
        1
    );
}
