use super::*;

#[test]
fn a_checkout_shared_with_another_account_is_read_without_the_one_it_came_from() {
    let harness = Harness::start();
    let guest = Someone::register(
        &harness.directory.join("server"),
        &harness.url,
        "guest@example.com",
        "Guest",
    );
    let personal = harness.connect_personal();
    let versioned = start_versioning("hello\n");
    let second = second_checkout(&versioned);
    let note = copy_of(versioned.note, second);

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
    set_access(second, guest.session.account, be_block_graph::Access::View);
    flush();
    assert!(
        runtime.block_on(peer.summary(versioned.note)).is_err(),
        "the guest was shared only the second checkout, not the first"
    );
    assert_eq!(
        runtime.block_on(read_text(&peer, note)),
        "hello\n",
        "the second checkout's copy was filled from the first's snapshot, and reads with its own keys"
    );
}
