use super::*;
use be_block_content::ChildChange;
use be_type_chat::{ChatContent, ChatEntry};
use be_type_workspace::WorkspaceContent;

/// A server that rewrites a block's signed log once this device read it leaves a verdict on
/// the block (guides/blocks/keys.md, "Forks"), told to apps as `BlockInfo::log_doubt` until the
/// person dismisses it on this device, across reconnects; the verdict itself stays kept, and a
/// verdict found later against another head shows again.
#[test]
fn a_log_the_server_rewrote_shows_as_a_doubt_until_dismissed() {
    let mut harness = Harness::start();
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
    for body in ["Pizza", "Tacos"] {
        let (_, message) = editor.mirror.append(said(body).encode(), Default::default());
        editor.send(message);
    }
    editor.until("heard both messages", |editor| editor.messages().len() == 2);
    wait_until("no doubt yet", |held| held.log_doubts.is_empty());

    // The server forgets the log and keeps another in its place.
    rewrite(&harness, chat, 3);
    let (_, message) = editor.mirror.append(said("Sushi").encode(), Default::default());
    editor.send(message);
    wait_until("the rewritten log left a doubt on the chat", |held| {
        held.log_doubts.contains_key(&chat)
    });
    let told = || {
        crate::app_graph::answer(be_sdk_api::blocks::BlockQuery::Block(chat.into_bytes()))
            .into_iter()
            .find(|info| info.block_id == chat.into_bytes())
            .and_then(|info| info.log_doubt)
    };
    let doubt = told().expect("apps are told of the doubt");
    assert_eq!(doubt.source, be_sdk_api::blocks::LogDoubtSource::Server);
    assert_eq!(doubt.ours, 2, "it names the head this device kept");

    // Told again as the connection comes back: the verdict is kept.
    let address = harness.address();
    reconnect(&mut harness, &address);
    wait_until("the doubt is told after reconnecting", |held| {
        held.log_doubts.contains_key(&chat)
    });

    crate::dismiss_log_doubt(chat);
    wait_until("the dismissed doubt is no longer told", |held| {
        held.log_doubts.is_empty()
    });
    assert_eq!(told(), None);
    reconnect(&mut harness, &address);
    assert!(
        crate::with_seen(|seen| seen.log_doubts.is_empty()) == Some(true),
        "a dismissed doubt stays hidden after reconnecting"
    );

    // The server rewrites the log again: a verdict against another head shows, so dismissing
    // never hides what the server does next.
    rewrite(&harness, chat, 5);
    let (_, message) = editor.mirror.append(said("Ramen").encode(), Default::default());
    editor.send(message);
    wait_until("the new fork brings the doubt back", |held| {
        held.log_doubts.contains_key(&chat)
    });
    assert!(told().is_some());
}

/// The server forgets `block`'s log and keeps `entries` made-up ones in its place.
fn rewrite(harness: &Harness, block: Uuid, entries: usize) {
    let database =
        be_block_sqlite_schema::open(harness.directory.join("server").join("metadata.sqlite"))
            .unwrap();
    be_server::log::forget(&database, harness.organization, block).unwrap();
    let entries = (0..entries)
        .map(|index| be_block_protocol::NewEntry {
            id: Uuid::new_v4(),
            payload: format!("made up {index}").into_bytes(),
            signature: vec![1; 64],
        })
        .collect();
    be_server::log::append(
        &database,
        harness.organization,
        block,
        harness.account,
        entries,
        be_block_commit::now_milliseconds(),
    )
    .unwrap();
}

/// Drops the connection and makes it again, as the worker does after the server comes back.
fn reconnect(harness: &mut Harness, address: &str) {
    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    harness.start_server(address);
    retry_now();
    wait_until("reconnected", |shared| shared.connected);
}
