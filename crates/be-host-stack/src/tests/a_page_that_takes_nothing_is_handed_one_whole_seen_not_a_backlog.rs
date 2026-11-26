use super::*;

/// A page that runs no frames (a hidden tab) takes no update while the worker keeps handing them
/// over: the handover never holds more than it gathers into one whole `Seen`, and the page that
/// comes back reads what the worker has.
#[test]
fn a_page_that_takes_nothing_is_handed_one_whole_seen_not_a_backlog() {
    let harness = Harness::start();
    harness.connect();
    for _ in 0..500 {
        with_shared_mut(|shared| shared.seen.frames_queued += 1);
        let held = stack()
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|stack| stack.handover.len())
            .expect("the stack runs");
        assert!(held <= 64, "the handover holds {held} updates");
    }
    let queued = with_shared(|shared| shared.seen.frames_queued).expect("the stack runs");
    assert_eq!(read_seen(|seen| seen.frames_queued), Some(queued));
}
