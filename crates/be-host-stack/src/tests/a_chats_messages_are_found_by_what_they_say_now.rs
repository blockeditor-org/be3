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

/// Appends `entry` from the chat's editor, with what goes along with it.
fn append(editor: &mut Editor<ChatContent>, entry: &ChatEntry, along: be_block_content::Along) {
    let (_, message) = editor.mirror.append(entry.encode(), along);
    editor.send(message);
}

/// A chat's messages live in its log, and each is found by what it says now: an edit replaces
/// its text, and a deleted message is not found.
#[test]
fn a_chats_messages_are_found_by_what_they_say_now() {
    let harness = Harness::start();
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
    let said = |body: &str| ChatEntry::Message {
        time: 0,
        body: body.to_owned(),
        reply_to: None,
        attachments: Vec::new(),
    };
    append(&mut editor, &said("Pizza at the corner place"), Default::default());
    append(&mut editor, &said("Bring the plumber's number"), Default::default());
    editor.until("heard both messages", |editor| editor.messages().len() == 2);
    finds("pizza", &[chat]);
    finds("plumber", &[chat]);

    append(
        &mut editor,
        &ChatEntry::Edit {
            message: 0,
            time: 0,
            body: "Tacos at the corner place".to_owned(),
        },
        Default::default(),
    );
    finds("tacos", &[chat]);
    finds("pizza", &[]);

    append(
        &mut editor,
        &ChatEntry::Delete {
            message: 1,
            time: 0,
        },
        be_block_content::Along {
            announcement: None,
            redacts: Some(1),
        },
    );
    finds("plumber", &[]);
    finds("tacos", &[chat]);
}
