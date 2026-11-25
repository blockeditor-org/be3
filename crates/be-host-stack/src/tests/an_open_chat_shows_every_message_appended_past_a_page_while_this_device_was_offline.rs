use super::*;
use be_block_content::ChildChange;
use be_type_chat::{ChatContent, ChatEntry};
use be_type_workspace::WorkspaceContent;

fn said(body: &str) -> ChatEntry {
    ChatEntry::Message {
        time: 0,
        body: body.to_owned(),
        reply_to: None,
        attachments: Vec::new(),
    }
}

/// An open chat whose log another device appended more than a server page to while this device
/// was offline shows every message once it reconnects, each once and in order: following the
/// log again (`Replicas::follow_logs`) reads on from the kept head a page at a time, so the
/// editor holds the latest of them, and pages back to the rest.
#[test]
fn an_open_chat_shows_every_message_appended_past_a_page_while_this_device_was_offline() {
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
    let (_, append) = editor
        .mirror
        .append(said("Before").encode(), Default::default());
    editor.send(append);
    editor.until("heard the first message", |editor| {
        editor.messages() == ["Before"]
    });
    wait_until("sent what it holds", |shared| {
        shared.connected && shared.waiting == 0
    });

    harness.relay().offline(true);
    wait_until("noticed the server was gone", |shared| !shared.connected);
    let other = Device::past_the_relay(&harness);
    let peer = std::sync::Arc::clone(&other.peer.borrow());
    let away: Vec<String> = (0..be_server::log::MAX_PAGE_ENTRIES + 10)
        .map(|index| format!("Away {index}"))
        .collect();
    other.runtime.block_on(async {
        for body in &away {
            let appended = peer
                .append(chat, "be3.chat", &said(body).encode(), Default::default())
                .await
                .expect("the other device appends");
            assert!(
                matches!(appended, be_block_client::Appended::Logged { .. }),
                "the server took {body:?}: {appended:?}"
            );
        }
    });
    assert_eq!(editor.messages(), ["Before"]);

    harness.relay().offline(false);
    retry_now();
    wait_until("reconnected", |shared| shared.connected);
    let mut shown = vec!["Before".to_owned()];
    shown.extend(away);
    // The editor holds the latest `LOG_HELD` of them, each once and with none left out
    // past the first page; it reads the older ones back as it pages.
    let held = shown.len() - be_sdk_type_guest::editor::LOG_HELD;
    editor.until("held the latest messages", |editor| {
        editor.messages() == shown[held..]
    });
    let (_, read) = editor.mirror.read_log(0, held as u32);
    editor.send(read);
    editor.until("showed every message once", |editor| {
        editor.messages() == shown
    });
}
