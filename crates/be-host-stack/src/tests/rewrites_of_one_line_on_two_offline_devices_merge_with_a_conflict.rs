use be_type_text::TextContent;

use super::*;

#[test]
fn rewrites_of_one_line_on_two_offline_devices_merge_with_a_conflict() {
    // On the manual clock the host only tries the server again at `retry_now`, so it cannot
    // publish before the other device does and leave that device's drain `Diverged`.
    let mut harness = Harness::start().on_manual_clock();
    harness.connect();
    let address = harness.address();
    let note = Uuid::new_v4();
    written(note, "one\ntwo\nthree\n");
    let other = Device::online(&harness);
    other.read(note);

    harness.stop_server();
    wait_until("tried the server and found it gone", |shared| {
        !shared.connected && shared.error.is_some()
    });
    open(note, TextContent::CONTENT_TYPE);
    replace(
        note,
        TextContent::CONTENT_TYPE,
        be_type_text::TextBlock::of("one\nours\nthree\n").encode(),
    );
    wait_until("rewrote the line offline", |shared| {
        text_of(shared, note).as_deref() == Some("one\nours\nthree\n")
    });
    flush();
    other.save_offline(note, "one\ntheirs\nthree\n");

    harness.start_server(&address);
    other.reconnect(&harness);
    retry_now();
    wait_until("merged with a conflict", |shared| {
        shared.connected && shared.waiting == 0 && shared.conflicts.contains_key(&note)
    });
    let status = status();
    assert_eq!(status.conflicts, 1);
    assert_eq!(status.conflicted, [note]);
    wait_until("showed both sides", |shared| {
        text_of(shared, note).as_deref()
            == Some("one\n<<<<<<< local\nours\n=======\ntheirs\n>>>>>>> remote\nthree\n")
    });
}
