use super::*;

#[test]
fn a_server_gone_silent_while_contacts_sync_holds_up_no_open_or_edit() {
    let harness = Harness::relayed();
    let guest = Someone::register(
        &harness.directory.join("server"),
        &harness.url,
        "guest@example.com",
        "Guest",
    );
    let personal = harness.connect_personal();
    let block = Uuid::new_v4();
    open(block, CounterContent::CONTENT_TYPE);
    add(block, 4);
    wait_for_count(block, 4);
    flush();
    close(block);
    wait_until("closed the counter", |shared| {
        !shared.blocks.contains_key(&block)
    });
    let code = made_link(None);

    let runtime = runtime();
    let heard = harness.relay().heard();
    harness.relay().stall_at_next_request();
    let peer = runtime.block_on(guest.peer(&harness.url, personal));
    runtime
        .block_on(be_client::contacts::accept_link(
            &peer.connection(),
            &guest.phrase.secret(),
            &guest.me().card(),
            &code,
        ))
        .expect("the guest accepts");
    runtime.block_on(harness.relay().heard_beyond(heard));

    harness.open_until_it_joins(block, CounterContent::CONTENT_TYPE);
    harness.clock.advance(be_types_host::EARLY_OPEN);
    wait_for_count(block, 4);
    add(block, 2);
    wait_for_count(block, 6);
    assert!(
        status().connected,
        "the block opened and took an edit before the connection was given up"
    );

    harness.relay().cut();
    wait_until("reconnected and confirmed the guest", |shared| {
        shared.connected
            && shared
                .contacts
                .iter()
                .any(|contact| contact.account == guest.session.account)
    });
    flush();
    let saved = runtime.block_on(async {
        outside_peer(harness.outside_config().organization(Some(personal)))
            .await
            .open::<CounterContent>(block)
            .await
            .expect("the counter opens from the server")
            .map(|counter| counter.root().value())
    });
    assert_eq!(saved, Some(6));
}
