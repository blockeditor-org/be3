use super::*;

/// Edits made to a block whose edit access was taken away meanwhile are set aside among the
/// unsent edits, but the block is still on the server and this account can still see it: it
/// stays in the files, unlike one the server refused to make.
#[test]
fn a_block_demoted_to_viewer_stays_listed_with_its_edits_set_aside() {
    let harness = Harness::relayed();
    let sharer = Someone::register(
        &harness.directory.join("server"),
        &harness.url,
        "sharer@example.com",
        "Sharer",
    );
    let personal = harness.connect_personal();
    let code = made_link(None);
    let runtime = runtime();
    let peer = runtime.block_on(sharer.peer(&harness.url, personal));
    runtime
        .block_on(be_block_client::contacts::accept_link(
            &peer.connection(),
            &sharer.phrase.secret(),
            &sharer.me().card(),
            &code,
        ))
        .expect("the sharer accepts the link");
    runtime
        .block_on(peer.refresh_contacts())
        .expect("the sharer lists its contacts");
    let public = peer
        .keys()
        .contact(harness.account)
        .expect("the sharer verified this account through the link");
    let block = runtime.block_on(async {
        let block = peer
            .create::<CounterContent>(be_block_graph::BlockParent::Root)
            .await
            .unwrap();
        peer.share(
            block,
            harness.account,
            be_block_graph::Access::Edit,
            &public,
        )
        .await
        .unwrap();
        block
    });

    ask_for(vec![Query::Shared]);
    wait_until("listed the shared block", |held| {
        held.graph
            .get(block)
            .is_some_and(|node| node.access.can_edit())
    });
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 1);
    wait_for_count(block, 1);
    flush_and_send();

    harness.relay().offline(true);
    wait_until("noticed it is offline", |held| !held.connected);
    add(block, 2);
    wait_for_count(block, 3);
    flush();
    runtime
        .block_on(peer.grant(block, harness.account, be_block_graph::Access::View))
        .expect("the sharer makes this account a viewer");

    harness.relay().offline(false);
    retry_now();
    wait_until("set the edits made meanwhile aside", |held| {
        held.unsent
            .iter()
            .any(|unsent| unsent.block == block && unsent.refused.is_none())
    });
    refetch(Query::Shared);
    wait_until("listed what is shared again", |held| {
        held.graph.ready(Query::Shared)
    });
    assert!(
        crate::wait_until(|held| Some(held.graph.get(block).is_some())),
        "the block this account can still see left the files"
    );
}
