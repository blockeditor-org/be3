use be_sdk_type_guest::editor::FromEditor;

use super::*;

#[test]
fn an_edit_made_offline_is_lacking_until_the_drain_publishes_it() {
    let mut harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let address = harness.address();
    open(block, CounterContent::CONTENT_TYPE);
    let mut editor = Editor::<CounterContent>::attach(block);
    editor.wait("was sent the counter", |_| true);
    editor.send(FromEditor::FollowLacking);

    editor.operate(Counter::add(2));
    editor.until("the server took the edit made online", |editor| {
        editor.mirror.pending() == 0 && editor.mirror.lacking().is_empty()
    });
    flush();

    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    editor.operate(Counter::add(3));
    editor.until("the edit made offline is lacking", |editor| {
        editor.mirror.pending() == 0 && editor.mirror.lacking().len() == 1
    });
    flush();
    editor.pump();
    assert_eq!(
        editor.mirror.lacking().len(),
        1,
        "a seal kept on this device still lacks the server"
    );

    harness.start_server(&address);
    retry_now();
    wait_until("reconnected and sent what waited", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    editor.until("the drain published the edit made offline", |editor| {
        editor.mirror.lacking().is_empty()
    });
    assert_eq!(count_of(block), Some(5));
}
