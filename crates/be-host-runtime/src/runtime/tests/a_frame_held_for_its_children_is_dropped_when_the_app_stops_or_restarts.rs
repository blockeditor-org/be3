use super::*;

/// A frame held for its children belongs to the session that drew it: a stop or restart drops
/// it, so the next session never shows it.
#[test]
fn a_frame_held_for_its_children_is_dropped_when_the_app_stops_or_restarts() {
    let mut runtime = Runtime::new(&manifest(), 0);
    runtime.hear(vec![placing(1, &[1])]);
    runtime.received(frame(1));
    assert!(
        runtime.held_frame.is_some(),
        "the frame waits for its children"
    );
    runtime.restart();
    assert!(runtime.held_frame.is_none(), "a restart drops it");

    runtime.hear(vec![placing(1, &[1])]);
    runtime.received(frame(1));
    assert!(
        runtime.held_frame.is_some(),
        "the frame waits for its children"
    );
    runtime.stopped("stopped".to_owned());
    assert!(runtime.held_frame.is_none(), "a stop drops it");
}
