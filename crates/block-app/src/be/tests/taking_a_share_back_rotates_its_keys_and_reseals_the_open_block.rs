use super::*;

#[test]
fn taking_a_share_back_rotates_its_keys_and_reseals_the_open_block() {
    let harness = Harness::start();
    let guest = Someone::register(
        &harness.directory.join("server"),
        &harness.url,
        "guest@example.com",
        "Guest",
    );
    let personal = harness.connect_personal();
    let block = Uuid::new_v4();
    written(block, "meet at noon");

    let code = made_link(None);
    let runtime = runtime();
    let peer = runtime.block_on(guest.peer(&harness.url, personal));
    runtime
        .block_on(be_client::contacts::accept_link(
            &peer.connection(),
            &guest.phrase.secret(),
            &guest.me().card(),
            &code,
        ))
        .expect("the guest accepts");
    wait_until("confirmed the guest as a contact", |shared| {
        shared
            .contacts
            .iter()
            .any(|contact| contact.account == guest.session.account && contact.public.is_some())
    });
    set_access(block, guest.session.account, be_graph::Access::Edit);
    flush();
    assert_eq!(runtime.block_on(read_text(&peer, block)), "meet at noon");

    open(block, text_block_types::TextContent::CONTENT_TYPE);
    wait_until("opened the block", |shared| {
        shared.blocks.contains_key(&block)
    });
    let before = node(block).and_then(|node| node.head);
    set_access(block, guest.session.account, be_graph::Access::None);
    flush();
    assert_eq!(status().error, None);
    wait_until("resealed the open block under its new key", |shared| {
        shared.graph.get(block).and_then(|node| node.head) != before
    });

    let mine = runtime
        .block_on(be_client::Peer::connect(
            be_client::PeerConfig::new(
                crate::accounts::socket_url(&harness.url),
                harness.phrase.secret(),
                be_client::Credentials::Token(harness.token.clone()),
            )
            .organization(Some(personal)),
            be_store::MemoryStore::new(),
        ))
        .expect("another device of the owner opens the organization");
    let summary = runtime
        .block_on(mine.summary(block))
        .expect("the owner still reads the block");
    let head = summary.head.expect("the block has a head");
    runtime
        .block_on(mine.fetch(&[head.hash()]))
        .expect("the owner fetches the head");
    let sealed = be_store::ObjectStore::get(mine.commits().vault().store(), head.hash())
        .expect("the store reads")
        .expect("the head is stored");
    assert_eq!(
        be_store::sealed_key(&sealed).ok(),
        summary.key,
        "the open block's head was not sealed again under its new key"
    );
    assert!(
        runtime
            .block_on(mine.key_work())
            .expect("the owner reads its key work")
            .rotations
            .is_empty(),
        "taking the share back left a rotation undone"
    );
    assert!(
        runtime
            .block_on(peer.open::<text_block_types::TextContent>(block))
            .is_err(),
        "the guest still opens a block taken back from it"
    );
    assert!(
        !runtime
            .block_on(peer.try_fetch(head.hash()))
            .expect("the server answers"),
        "the server served the guest what was written after"
    );
}
