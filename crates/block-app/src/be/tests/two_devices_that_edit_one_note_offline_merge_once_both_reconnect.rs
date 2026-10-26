use text_block_types::TextContent;

use super::*;

#[test]
fn two_devices_that_edit_one_note_offline_merge_once_both_reconnect() {
    let mut harness = Harness::start();
    harness.connect();
    let address = harness.address();
    let note = Uuid::new_v4();
    written(note, "one\ntwo\nthree\n");
    let other = Device::online(&harness);
    let base = other.read(note);
    assert_eq!(base, "one\ntwo\nthree\n");

    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    open(note, TextContent::CONTENT_TYPE);
    type_text(note, 0, "zero\n");
    wait_until("typed offline", |shared| {
        text_of(shared, note).as_deref() == Some("zero\none\ntwo\nthree\n")
    });
    flush();
    other.save_offline(note, "one\ntwo\nTHREE\n");

    harness.start_server(&address);
    other.reconnect(&harness);
    retry_now();
    wait_until("merged what both devices wrote", |shared| {
        shared.connected
            && shared.waiting == 0
            && text_of(shared, note).as_deref() == Some("zero\none\ntwo\nTHREE\n")
    });
    assert_eq!(status().conflicts, 0);
    flush();
    assert_eq!(other.read(note), "zero\none\ntwo\nTHREE\n");
}
