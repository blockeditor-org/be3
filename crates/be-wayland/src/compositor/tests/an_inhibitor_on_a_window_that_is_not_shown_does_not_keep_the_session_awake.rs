use super::*;

use std::time::Duration;

const BLANK: Duration = Duration::from_secs(60);

#[test]
fn an_inhibitor_on_a_window_that_is_not_shown_does_not_keep_the_session_awake() {
    let mut harness = Harness::new();
    harness.compositor().set_blank_after(Some(BLANK));
    let hidden = harness.client.toplevel();
    let _inhibitor = harness.client.inhibit(&hidden.surface);
    harness.settle();
    assert!(
        harness
            .compositor()
            .windows()
            .list()
            .get_untracked()
            .is_empty(),
        "the window has nothing to show, so it is not shown"
    );

    harness.context().advance_clock(BLANK);
    harness.frame(Vec::new());
    assert!(harness.compositor().idle());
}
