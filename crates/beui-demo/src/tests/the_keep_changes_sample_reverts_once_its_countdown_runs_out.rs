use std::time::Duration;

use super::*;

#[test]
fn the_keep_changes_sample_reverts_once_its_countdown_runs_out() {
    let mut test = alone(WIDE, Page::Overlays);
    let root = test.document().root().expect("the demo built a root");
    test.click("demo.keep_changes.open");
    test.frame(Vec::new());
    assert!(test.shows("keep-changes.0.keep"), "the prompt opens");
    assert_eq!(
        showing(test.document(), root, "Reverting in 15 seconds."),
        1
    );
    test.snapshot("keep_changes");

    test.advance(Duration::from_secs(13));
    assert!(
        test.shows("keep-changes.0"),
        "the prompt waits out the countdown"
    );
    assert_eq!(showing(test.document(), root, "Reverting in 2 seconds."), 1);

    test.advance(Duration::from_secs(2));
    test.frame(Vec::new());
    assert!(!test.shows("keep-changes.0"), "the prompt closes at zero");
    assert_eq!(
        showing(test.document(), root, "Went back to 1920 × 1080 at 60 Hz"),
        1,
        "running out reverts"
    );
}
