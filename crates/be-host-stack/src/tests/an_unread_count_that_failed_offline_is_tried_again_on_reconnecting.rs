use super::*;
use be_block_content::Root;
use be_type_chat::{ChatContent, ChatReads};

fn loads() -> u64 {
    with_shared(|shared| shared.counting.made.0).unwrap_or_default()
}

/// Offline, a chat whose head this device does not hold cannot be loaded to count: it is not
/// tried again while nothing moves, but it is once the device connects again.
#[test]
fn an_unread_count_that_failed_offline_is_tried_again_on_reconnecting() {
    crate::unread::forget();
    let mut harness = Harness::start();
    harness.connect();
    // The chat type's module, which picks places and counts, is started as a chat opens.
    let held = Uuid::new_v4();
    create(
        held,
        ChatContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Held"),
        Some(ChatContent::default().encode()),
        false,
    );
    open(held, ChatContent::CONTENT_TYPE);
    wait_until("knew the chat type", |_| {
        is_known(ChatContent::CONTENT_TYPE)
    });
    flush_and_send();
    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);

    let (parent, chat) = (Uuid::new_v4(), Uuid::new_v4());
    let lacked = be_block_commit::CommitId::from_hash(be_block_store::Hash::of(b"elsewhere"));
    with_shared_mut(|shared| {
        shared
            .graph
            .want(Owner::Editor(u64::MAX - 2), vec![Query::Children(parent)]);
        shared.graph.listed(
            crate::graph::Listing::Children(be_block_graph::BlockParent::Block(parent)),
            vec![Node {
                id: chat,
                content_type: ChatContent::CONTENT_TYPE,
                author: None,
                parent: Some(be_block_graph::BlockParent::Block(parent)),
                access: be_block_graph::Access::Edit,
                references: None,
                metadata: be_block_content::BlockMetadata::default(),
                head: Some(lacked),
                locked: false,
                held: true,
                sets_access: false,
                size: 0,
                tree_size: 0,
                changed: 0,
                entry: None,
                unread: 0,
            }],
            u64::MAX,
        );
    });
    crate::unread::publish(ChatReads::CONTENT_TYPE, Vec::new());
    wait_until("tried to load the chat's head offline", |shared| {
        shared.counting.made.0 >= 1
    });
    flush();
    let offline = loads();

    harness.start_server();
    wait_until("connected", |shared| shared.connected);
    wait_until("tried the chat's head again once connected", |shared| {
        shared.counting.made.0 > offline
    });
    crate::unread::forget();
}
