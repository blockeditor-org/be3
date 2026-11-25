use super::*;
use be_block_content::ChildChange;
use be_type_chat::{ChatContent, ChatEntry};
use be_type_workspace::WorkspaceContent;

/// While this device is away, another appends more messages to an open chat than the server
/// gives in one page of its log (`MAX_PAGE_ENTRIES`). Reconnecting, the device follows the log
/// on from its kept head a page at a time (`Replicas::follow_logs`), and the open chat shows
/// every message up to the last, as many as an editor holds.
#[test]
fn an_open_chat_shows_every_message_appended_past_a_page_while_this_device_was_away() {
    const PAST_A_PAGE: usize = be_server::log::MAX_PAGE_ENTRIES as usize + 5;
    let harness = Harness::relayed();
    harness.connect();
    let workspace = Uuid::new_v4();
    create(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Team"),
        None,
        false,
    );
    wait_until("knew the workspace's type", |_| {
        is_known(WorkspaceContent::CONTENT_TYPE)
    });
    let chat = Uuid::new_v4();
    create(
        chat,
        ChatContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(workspace),
        be_block_content::BlockMetadata::named("Lunch"),
        Some(ChatContent::default().encode()),
        false,
    );
    assert!(made(change_child(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        ChildChange::Add(chat)
    )));
    flush_and_send();
    let mut editor = Editor::<ChatContent>::attach_as(chat, "be3.chat");
    editor.wait("was sent the chat", |_| true);
    let (_, read) = editor.mirror.read_log(0, 100);
    editor.send(read);
    let said = |body: String| ChatEntry::Message {
        time: 0,
        body,
        reply_to: None,
        attachments: Vec::new(),
    };
    let (_, append) = editor
        .mirror
        .append(said("before".to_owned()).encode(), Default::default());
    editor.send(append);
    editor.until("heard the first message", |editor| editor.messages().len() == 1);
    wait_until("sent it", |shared| shared.connected && shared.waiting == 0);

    harness.relay().offline(true);
    wait_until("noticed the server was gone", |shared| !shared.connected);
    let other = Device::past_the_relay(&harness);
    let peer = std::sync::Arc::clone(&other.peer.borrow());
    other.runtime.block_on(async {
        for n in 0..PAST_A_PAGE {
            let appended = peer
                .append(
                    chat,
                    "be3.chat",
                    &said(format!("away {n}")).encode(),
                    Default::default(),
                )
                .await
                .expect("the other device appends");
            assert!(
                matches!(appended, be_block_client::Appended::Logged { .. }),
                "the server took message {n}: {appended:?}"
            );
        }
    });

    harness.relay().offline(false);
    retry_now();
    wait_until("reconnected", |shared| shared.connected);
    // The editor holds the newest `LOG_HELD` entries: the last are those only the second page
    // gives.
    let last = format!("away {}", PAST_A_PAGE - 1);
    editor.until("heard the last message, past the first page", |editor| {
        editor.messages().last() == Some(&last)
    });
    let messages = editor.messages();
    assert_eq!(
        messages.len(),
        be_sdk_type_guest::editor::LOG_HELD,
        "it holds as many as an editor does"
    );
    let first = 1 + PAST_A_PAGE - be_sdk_type_guest::editor::LOG_HELD;
    assert_eq!(
        messages.first(),
        Some(&format!("away {}", first - 1)),
        "every message in order, none lost between the pages"
    );
}
