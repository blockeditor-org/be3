use super::*;

/// A device that starts again with no server lists its roots from what it keeps, each with its
/// author: the host finds the account's settings among them by it (`RootSettings::owned`), so a
/// root it could not tell as the account's own would be made again beside it.
#[test]
fn roots_listed_offline_after_a_restart_keep_their_author() {
    let mut harness = Harness::start();
    harness.connect();
    let account = account().expect("signed in");
    let made = Uuid::new_v4();
    ask_for(vec![Query::Roots]);
    create(
        made,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block::BlockMetadata::default(),
        None,
        false,
    );
    flush_and_send();
    // Listed again by the server, which now holds it: the listing the device keeps is the
    // server's, not the one it made up while the creation waited.
    refetch(Query::Roots);
    wait_until("the server listed the root", |shared| {
        shared.graph.ready(Query::Roots) && shared.graph.get(made).is_some()
    });

    harness.stop_server();
    wait_until("noticed the server was gone", |shared| !shared.connected);
    stop();
    harness.connect();
    ask_for(vec![Query::Roots]);
    wait_until("listed the roots from what the device keeps", |shared| {
        shared.graph.ready(Query::Roots)
    });
    assert!(!status().connected);
    let node = query(Query::Roots)
        .into_iter()
        .find(|node| node.id == made)
        .expect("the root is listed offline");
    assert_eq!(node.author, Some(account));
}
