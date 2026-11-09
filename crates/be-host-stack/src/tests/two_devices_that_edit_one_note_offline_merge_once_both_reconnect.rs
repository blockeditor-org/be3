use be_type_text::TextContent;

use super::*;

#[test]
fn two_devices_that_edit_one_note_offline_merge_once_both_reconnect() {
    // On the manual clock the host only tries the server again at `retry_now`, so it cannot
    // publish before the other device does and leave that device's drain `Diverged`.
    let mut harness = Harness::start().on_manual_clock();
    harness.connect();
    let address = harness.address();
    let note = Uuid::new_v4();
    written(note, "one\ntwo\nthree\n");
    let other = Device::online(&harness);
    let base = other.read(note);
    assert_eq!(base, "one\ntwo\nthree\n");

    harness.stop_server();
    wait_until("tried the server and found it gone", |shared| {
        !shared.connected && shared.error.is_some()
    });
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
