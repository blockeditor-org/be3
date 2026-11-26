use super::*;

/// A message the server took well after its author wrote it says when it arrived: it was
/// written offline.
#[test]
fn a_message_sent_offline_shows_when_it_arrived() {
    let mut chat = Harness::new(&[(ROSA, "Anyone around?")]);
    let late = chat.arrive(
        ROSA,
        &message(MORNING - 60 * MINUTE, "Wrote this on the plane"),
    );
    let row = chat.row(late);
    assert!(row.contains("sent offline, arrived"), "{row}");
    chat.snapshot("a_message_sent_offline_shows_when_it_arrived");
}
