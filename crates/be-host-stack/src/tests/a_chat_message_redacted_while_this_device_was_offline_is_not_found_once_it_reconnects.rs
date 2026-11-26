use super::*;
use be_block_content::ChildChange;
use be_type_chat::{ChatContent, ChatEntry};
use be_type_workspace::WorkspaceContent;

use crate::search::SearchAsk;

/// Waits until a search for `text` answers exactly `blocks`, every queued edit indexed.
fn finds(text: &str, blocks: &[Uuid]) {
    let ask = SearchAsk {
        text: text.to_owned(),
        within: None,
    };
    want_searches(Owner::Host, vec![ask.clone()]);
    crate::wait_until(&format!("found {text:?} in what it should"), |shared| {
        let answer = shared.search.answer(&ask)?;
        let found: Vec<Uuid> = answer.hits.iter().map(|hit| hit.block).collect();
        (answer.complete && found == blocks).then_some(())
    });
}

/// A chat message redacted on another device while this one was offline, which heard no push
/// of it, is not found once this device reconnects: it catches up with the redactions of the
/// logs it read (`Peer::catch_up_redactions`), and search drops the message's text.
#[test]
fn a_chat_message_redacted_while_this_device_was_offline_is_not_found_once_it_reconnects() {
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
    for body in ["Pizza at the corner place", "The door code is 4512"] {
        let said = ChatEntry::Message {
            time: 0,
            body: body.to_owned(),
            reply_to: None,
            attachments: Vec::new(),
        };
        let (_, append) = editor.mirror.append(said.encode(), Default::default());
        editor.send(append);
    }
    editor.until("heard both messages", |editor| editor.messages().len() == 2);
    finds("door", &[chat]);
    drop(editor);
    wait_until("sent what it indexed", |shared| {
        shared.connected && shared.waiting == 0
    });

    harness.relay().offline(true);
    wait_until("noticed the server was gone", |shared| !shared.connected);
    let other = Device::past_the_relay(&harness);
    let peer = std::sync::Arc::clone(&other.peer.borrow());
    other
        .runtime
        .block_on(peer.redact(chat, 1))
        .expect("the other device redacts the message");
    finds("door", &[chat]);

    harness.relay().offline(false);
    retry_now();
    wait_until("reconnected", |shared| shared.connected);
    finds("door", &[]);
    finds("pizza", &[chat]);
}
