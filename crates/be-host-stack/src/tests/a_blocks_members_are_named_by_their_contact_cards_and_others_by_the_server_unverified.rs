use super::*;

#[test]
fn a_blocks_members_are_named_by_their_contact_cards_and_others_by_the_server_unverified() {
    let harness = Harness::start();
    let personal = harness.connect_personal();
    let block = Uuid::new_v4();
    written(block, "Plan");
    flush();

    // Ada is a contact, whose card says "Ada"; the server now says otherwise, which an editor
    // is not shown.
    let ada = Member::join(&harness, personal, "ada@example.com", "Ada");
    let server = be_server::ServerStore::open(&harness.directory.join("server")).unwrap();
    runtime()
        .block_on(server.update_display_name(ada.account(), "Not Ada"))
        .expect("the server renames her");
    // The server renames this account too: it is named by the name this device holds of
    // itself, the one on the card it hands out, never by the server's.
    runtime()
        .block_on(server.update_display_name(harness.account, "Not Counter"))
        .expect("the server renames this account");
    set_access(block, ada.account(), be_block_graph::Access::Manage, |_| {});
    flush_and_send();
    assert_eq!(status().error, None);

    // Teo is Ada's contact, not this account's: she shares the block with him.
    let teo = Someone::register(
        &harness.directory.join("server"),
        &harness.url,
        "teo@example.com",
        "Teo Lind",
    );
    let code = ada
        .runtime
        .block_on(be_block_client::contacts::make_link(
            &ada.peer.connection(),
            &ada.someone.phrase.secret(),
            &ada.someone.me().card(),
            None,
        ))
        .expect("Ada makes a contact link");
    let teo_peer = ada.runtime.block_on(teo.peer(&harness.url, personal));
    ada.runtime
        .block_on(be_block_client::contacts::open_link(
            &teo_peer.connection(),
            &code,
            be_block_client::contacts::Expected::Contact,
        ))
        .expect("Teo opens the link");
    ada.runtime
        .block_on(be_block_client::contacts::accept_link(
            &teo_peer.connection(),
            &teo.phrase.secret(),
            &teo.me().card(),
            &code,
            be_block_client::contacts::Expected::Contact,
        ))
        .expect("Teo accepts");
    ada.runtime
        .block_on(ada.peer.open::<be_type_text::TextContent>(block))
        .expect("Ada opens the block")
        .expect("the block has content");
    ada.runtime
        .block_on(ada.peer.share(
            block,
            teo.session.account,
            be_block_graph::Access::View,
            &teo.phrase.secret().identity(),
        ))
        .expect("Ada shares the block with Teo");

    let (sender, receiver) = std::sync::mpsc::channel();
    list_names(block, move |named| {
        let _ = sender.send(named);
    });
    let named = receiver
        .recv()
        .expect("the worker answered")
        .expect("the server named the members");
    let mut members: Vec<(Uuid, String, bool)> = crate::app_graph::members_of(&named)
        .into_iter()
        .map(|member| (Uuid::from_bytes(member.account), member.name, member.verified))
        .collect();
    members.sort();
    let mut expected = vec![
        (harness.account, "Counter".to_owned(), true),
        (ada.account(), "Ada".to_owned(), true),
        (teo.session.account, "Teo Lind".to_owned(), false),
    ];
    expected.sort();
    assert_eq!(members, expected);
}

