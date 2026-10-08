use super::*;

use std::time::Duration;

const BLANK: Duration = Duration::from_secs(600);
const SECOND: Duration = Duration::from_secs(1);

#[test]
fn the_session_idles_after_the_blank_time_and_wakes_on_input() {
    let mut harness = Harness::new();
    harness.context.stop_clock();
    harness.app.set_blank_after(Some(BLANK));
    harness.settle();
    let idle = harness.app.windows().idle();

    harness.context.advance_clock(BLANK - SECOND);
    harness.frame(Vec::new());
    assert!(!harness.app.idle());
    assert_eq!(
        harness.output.as_ref().unwrap().repaint_after,
        SECOND,
        "a frame is asked for when the blank time runs out, not before"
    );

    harness.context.advance_clock(SECOND);
    harness.frame(Vec::new());
    assert!(harness.app.idle());
    assert!(idle.get_untracked(), "the UI hears the session went idle");

    harness.frame(vec![Event::PointerMoved(pos2(5.0, 5.0))]);
    assert!(!harness.app.idle(), "input wakes the session");
    assert!(!idle.get_untracked());

    harness.context.advance_clock(BLANK - SECOND);
    harness.frame(Vec::new());
    assert!(!harness.app.idle(), "the input started the count again");
    harness.context.advance_clock(SECOND);
    harness.frame(Vec::new());
    assert!(harness.app.idle());

    harness.app.set_blank_after(None);
    harness.frame(Vec::new());
    harness.context.advance_clock(BLANK * 10);
    harness.frame(Vec::new());
    assert!(!harness.app.idle(), "never blanking never idles");
}
