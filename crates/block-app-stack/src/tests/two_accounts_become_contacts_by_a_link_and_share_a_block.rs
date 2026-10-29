use super::*;

#[test]
fn two_accounts_become_contacts_by_a_link_and_share_a_block() {
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
    let (_, card) = runtime
        .block_on(be_client::contacts::open_link(&peer.connection(), &code))
        .expect("the guest opens the link");
    assert_eq!(card.email, "counter@example.com");
    assert_eq!(card.public, harness.phrase.secret().public());
    runtime
        .block_on(be_client::contacts::accept_link(
            &peer.connection(),
            &guest.phrase.secret(),
            &guest.me().card(),
            &code,
        ))
        .expect("the guest accepts");
    let again = runtime.block_on(be_client::contacts::open_link(&peer.connection(), &code));
    assert!(again.is_err(), "the link opened after it was used");

    wait_until("confirmed the guest as a contact", |shared| {
        shared.contacts.iter().any(|contact| {
            contact.account == guest.session.account
                && contact.public == Some(guest.phrase.secret().public())
        })
    });
    let theirs = runtime
        .block_on(be_client::contacts::contacts(
            &peer.connection(),
            peer.account(),
            &guest.phrase.secret(),
        ))
        .expect("the guest lists its contacts");
    assert_eq!(theirs.len(), 1);
    assert_eq!(theirs[0].public, Some(harness.phrase.secret().public()));

    set_access(block, guest.session.account, be_graph::Access::View);
    flush();
    assert_eq!(status().error, None);
    assert_eq!(runtime.block_on(read_text(&peer, block)), "meet at noon");
}
