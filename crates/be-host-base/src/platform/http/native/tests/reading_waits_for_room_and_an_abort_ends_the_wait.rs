use super::*;

#[test]
fn reading_waits_for_room_and_an_abort_ends_the_wait() {
    let flow = Flow::default();
    for _ in 0..crate::platform::http::CHUNKS_AHEAD {
        assert!(flow.take(), "a chunk within the room is read at once");
    }
    assert_eq!(flow.room().chunks, 0, "the next read has to wait");

    flow.release();
    assert!(flow.take(), "a chunk passed on makes room for one more");

    let waiting = flow.clone();
    let reader = thread::spawn(move || waiting.take());
    flow.abort();
    assert!(
        !reader.join().unwrap(),
        "an abort wakes the waiting read, which stops"
    );
    assert!(!flow.take(), "nothing is read after an abort");
}
