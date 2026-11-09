use super::*;

#[test]
fn taking_a_share_back_rotates_its_keys_and_seals_what_follows_under_the_new_one() {
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
        .block_on(be_block_client::contacts::accept_link(
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
    set_access(block, guest.session.account, be_block_graph::Access::Edit);
    flush();
    assert_eq!(runtime.block_on(read_text(&peer, block)), "meet at noon");

    open(block, be_type_text::TextContent::CONTENT_TYPE);
    wait_until("opened the block", |shared| {
        shared.blocks.contains_key(&block)
    });
    let before = node(block).and_then(|node| node.head);
    set_access(block, guest.session.account, be_block_graph::Access::None);
    flush();
    assert_eq!(status().error, None);
    // The open session's forced seal writes only what the content does not hand back as
    // `Part::Reused`, and a text document hands back every part it did not change, so it
    // publishes nothing (decisions/reused_parts_keep_their_key.md). What is written after the
    // rotation is sealed under the new key.
    assert_eq!(
        node(block).and_then(|node| node.head),
        before,
        "the forced seal of unchanged parts published a commit"
    );
    let mut editor = Editor::<be_type_text::TextContent>::attach(block);
    type_into(&mut editor, 0, "then ");
    flush();
    wait_until("sealed the edit made after the rotation", |shared| {
        shared.graph.get(block).and_then(|node| node.head) != before
    });
    drop(editor);

    let mine = runtime
        .block_on(be_block_client::Peer::connect(
            be_block_client::PeerConfig::new(
                crate::accounts::socket_url(&harness.url),
                harness.phrase.secret(),
                be_block_client::Credentials::Token(harness.token.clone()),
            )
            .organization(Some(personal)),
            be_block_store::MemoryStore::new(),
        ))
        .expect("another device of the owner opens the organization");
    let summary = runtime
        .block_on(mine.summary(block))
        .expect("the owner still reads the block");
    let head = summary.head.expect("the block has a head");
    runtime
        .block_on(mine.fetch(&[head.hash()]))
        .expect("the owner fetches the head");
    let sealed = be_block_store::ObjectStore::get(mine.commits().vault().store(), head.hash())
        .expect("the store reads")
        .expect("the head is stored");
    assert_eq!(
        be_block_store::sealed_key(&sealed).ok(),
        summary.key,
        "what was written after the rotation was not sealed under the new key"
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
            .block_on(peer.open::<be_type_text::TextContent>(block))
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
