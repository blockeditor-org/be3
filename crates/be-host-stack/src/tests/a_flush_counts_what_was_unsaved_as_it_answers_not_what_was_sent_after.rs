use super::*;

/// A flush answers with what is unsaved as the worker answers it: a command sent after the
/// flush, which a window that keeps drawing sends while it is asked to close, does not count,
/// so the close it answers for is not refused over nothing.
#[test]
fn a_flush_counts_what_was_unsaved_as_it_answers_not_what_was_sent_after() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();

    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);
    assert_eq!(flush(), 0);

    let (done, answered) = std::sync::mpsc::channel();
    send(Command::Flush(done));
    send(Command::HoldClock(None));
    assert_eq!(
        answered.recv().expect("the worker answers the flush"),
        0,
        "the command sent after the flush is not counted in its answer"
    );
}
