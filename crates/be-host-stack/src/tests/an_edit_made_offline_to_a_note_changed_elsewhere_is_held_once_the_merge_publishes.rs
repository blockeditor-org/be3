use be_sdk_type_guest::editor::FromEditor;
use be_type_text::TextContent;

use super::*;

#[test]
fn an_edit_made_offline_to_a_note_changed_elsewhere_is_held_once_the_merge_publishes() {
    let mut harness = Harness::start().on_manual_clock();
    harness.connect();
    let address = harness.address();
    let note = Uuid::new_v4();
    written(note, "one\ntwo\nthree\n");
    let other = Device::online(&harness);
    assert_eq!(other.read(note), "one\ntwo\nthree\n");

    harness.stop_server();
    wait_until("tried the server and found it gone", |shared| {
        !shared.connected && shared.error.is_some()
    });
    open(note, TextContent::CONTENT_TYPE);
    let mut editor = Editor::<TextContent>::attach(note);
    editor.wait("was sent the note", |_| true);
    editor.send(FromEditor::FollowLacking);
    type_into(&mut editor, 0, "zero\n");
    editor.until("the edit made offline is lacking", |editor| {
        editor.mirror.pending() == 0 && !editor.mirror.lacking().is_empty()
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
    editor.until("the drain diverged and the merge published the edit", |editor| {
        editor.mirror.lacking().is_empty()
    });
}
