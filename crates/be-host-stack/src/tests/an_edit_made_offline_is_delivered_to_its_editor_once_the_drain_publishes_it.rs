use super::*;
use be_sdk_type_guest::editor::ToEditor;

#[test]
fn an_edit_made_offline_is_delivered_to_its_editor_once_the_drain_publishes_it() {
    let mut harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();
    let address = harness.address();
    open(block, CounterContent::CONTENT_TYPE);
    let mut editor = Editor::<CounterContent>::attach(block);
    editor.wait("was sent the counter", |_| true);

    editor.operate(Counter::add(2));
    editor.until("delivered the edit made online", |editor| {
        editor.mirror.delivered() == 1
    });
    flush();

    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    editor.until(
        "was told the server lacks what this device holds",
        |editor| {
            editor
                .received
                .iter()
                .any(|message| matches!(message, ToEditor::Published(false)))
        },
    );
    editor.operate(Counter::add(3));
    editor.until("the edit made offline was confirmed", |editor| {
        editor.mirror.pending() == 0
    });
    flush();
    assert_eq!(
        (editor.mirror.sent(), editor.mirror.delivered()),
        (2, 1),
        "an edit made offline is not delivered while the server lacks it"
    );

    harness.start_server(&address);
    retry_now();
    wait_until("reconnected and sent what waited", |shared| {
        shared.connected && shared.waiting == 0 && !shared.draining
    });
    editor.until("delivered the edit made offline", |editor| {
        editor.mirror.delivered() == 2
    });
    assert_eq!(count_of(block), Some(5));
}
