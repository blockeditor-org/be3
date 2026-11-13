use super::*;
use be_block::ChildChange;
use be_type_chat::{ChatContent, Message};
use be_type_workspace::WorkspaceContent;

use crate::search::SearchAsk;

/// Waits until a search for `text` answers exactly `blocks`, every queued edit indexed.
fn finds(text: &str, blocks: &[Uuid]) {
    let ask = SearchAsk {
        text: text.to_owned(),
        within: None,
    };
    want_searches(Owner::Host, vec![ask.clone()]);
    crate::wait_until(|shared| {
        let answer = shared.search.answer(&ask)?;
        let found: Vec<Uuid> = answer.hits.iter().map(|hit| hit.block).collect();
        (answer.complete && found == blocks).then_some(())
    });
}

fn parts_read() -> u64 {
    crate::wait_until(|shared| Some(shared.search.parts_read))
}

#[test]
fn a_chat_is_reindexed_without_reading_its_closed_segments_again() {
    let harness = Harness::start();
    harness.connect();
    let workspace = Uuid::new_v4();
    create(
        workspace,
        WorkspaceContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block::BlockMetadata::named("Team"),
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
        be_block::BlockMetadata::named("Lunch"),
        Some(ChatContent::default().encode()),
        false,
    );
    assert_eq!(
        change_child(
            workspace,
            WorkspaceContent::CONTENT_TYPE,
            ChildChange::Add(chat)
        ),
        Some(true)
    );
    open(chat, ChatContent::CONTENT_TYPE);
    let mut editor = Editor::<ChatContent>::attach(chat);
    editor.wait("was sent the chat", |_| true);

    // About 120 KiB of messages, the oldest of which close into a segment.
    let author = Uuid::new_v4();
    let filler = "a line of chat ".repeat(70);
    let mut sent = Vec::new();
    for index in 0..120 {
        let (id, send) = editor.content().send(
            chat,
            Message::new(author, index, format!("word{index} {filler}")),
        );
        editor.operate(send.into());
        sent.push(id);
    }
    let closing = editor
        .content()
        .closing()
        .expect("a chat this long closes a segment");
    editor.operate(closing);
    editor.wait("closed a segment", |chat| chat.segments().len() == 1);
    flush();
    finds("word3 ", &[chat]);
    finds("word119 ", &[chat]);
    let read = parts_read();
    assert!(read > 0, "the segment's text was read once");

    let (_, send) = editor
        .content()
        .send(chat, Message::new(author, 200, "word200"));
    editor.operate(send.into());
    flush();
    finds("word200 ", &[chat]);
    assert_eq!(
        parts_read(),
        read,
        "a new message reindexes the chat without reading its segment again"
    );

    // The segment's entry is removed (a merge, say) after the indexer read what the index held
    // and before it wrote: its put without text is one the index has no text for, so the
    // index asks for it, and the chat is queued to be read whole again, parts and all.
    let index = node(workspace)
        .and_then(|node| node.metadata.get(&be_block::SEARCH_INDEX))
        .map(|be_block::be_block_model::BlockRef(index)| index)
        .expect("the workspace names its index");
    // This writes the index as another device's indexer would, so this device's starts nothing
    // meanwhile and finishes what it was writing: a batch of its own between the two writes
    // would put the segment's text back, or seal over one of them.
    crate::with_shared_mut(|shared| shared.search.off = true);
    crate::wait_until(|shared| {
        shared
            .search
            .busy_with(crate::search::wants_index(shared))
            .is_none()
            .then_some(())
    });
    let peer = crate::with_shared(|shared| shared.peer.clone())
        .flatten()
        .expect("the host has a peer");
    let runtime = runtime();
    let needs_text = runtime.block_on(async {
        let segment = crate::search::entries(&peer, index, chat)
            .await
            .into_iter()
            .find(|entry| entry.source.is_some() && !entry.is_removed())
            .expect("the index holds the segment, from its part");
        let later = |by: i64| be_lib_search::Commit {
            time: segment.commit.time + by,
            id: segment.commit.id,
        };
        let removed = be_lib_search::Batch {
            removed: vec![(segment.doc.clone(), later(1))],
            ..be_lib_search::Batch::default()
        };
        crate::search::write(&peer, index, removed)
            .await
            .expect("the removal is written");
        let unchanged = be_lib_search::Batch {
            put: vec![be_lib_search::Put {
                doc: segment.doc.clone(),
                commit: later(2),
                source: segment.source,
                text: None,
            }],
            ..be_lib_search::Batch::default()
        };
        crate::search::write(&peer, index, unchanged)
            .await
            .expect("the put is written")
    });
    crate::with_shared_mut(|shared| shared.search.off = false);
    let needs_text = needs_text.expect("the index has a head");
    assert_eq!(needs_text.len(), 1, "the index has no text for the segment");
    let read = parts_read();
    crate::reread_search(
        needs_text
            .iter()
            .filter_map(|doc| Uuid::from_slice(&doc.block).ok()),
    );
    crate::wait_until(|shared| (shared.search.parts_read > read).then_some(()));
    // In the race the chat's head is newer than the removal; here the removal was stamped
    // after the head, so the chat moves on once more and its segment is back.
    let (_, send) = editor
        .content()
        .send(chat, Message::new(author, 201, "word201"));
    editor.operate(send.into());
    flush();
    finds("word201 ", &[chat]);
    finds("word4 ", &[chat]);

    // A deleted message leaves search, whether recent or in the closed segment.
    let delete_recent = editor.content().delete(sent[119], author, 300);
    editor.operate(delete_recent.into());
    let delete_closed = editor.content().delete(sent[3], author, 301);
    editor.operate(delete_closed.into());
    flush();
    finds("word119 ", &[]);
    finds("word3 ", &[]);
    finds("word4 ", &[chat]);
}
