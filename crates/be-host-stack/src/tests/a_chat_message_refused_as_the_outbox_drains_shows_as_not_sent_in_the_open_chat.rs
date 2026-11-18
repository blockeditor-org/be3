use super::*;

/// A message kept offline that the server refuses as the outbox drains, its author demoted to
/// viewer meanwhile, reaches the open chat as not sent: the drain's refusal is pushed to the
/// block's replica (`Replicas::not_appended`, `replica.refused`), which tells its editors.
#[test]
fn a_chat_message_refused_as_the_outbox_drains_shows_as_not_sent_in_the_open_chat() {
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
            be_block_client::contacts::Expected::Contact,
            &Default::default(),
        ))
        .expect("the sharer accepts the link");
    runtime
        .block_on(peer.refresh_contacts())
        .expect("the sharer lists its contacts");
    let public = peer
        .verified_identity(harness.account)
        .expect("the sharer verified this account through the link");
    let chat = runtime.block_on(async {
        let chat = peer
            .create::<be_type_chat::ChatContent>(be_block_graph::BlockParent::Root)
            .await
            .unwrap();
        peer.share(chat, harness.account, be_block_graph::Access::Edit, &public)
            .await
            .unwrap();
        chat
    });

    ask_for(vec![Query::Shared]);
    wait_until("listed the shared chat", |held| {
        held.graph
            .get(chat)
            .is_some_and(|node| node.access.can_edit())
    });
    let mut editor = Editor::<be_type_chat::ChatContent>::attach_as(chat, "be3.chat");
    editor.wait("was sent the chat", |_| true);
    let (_, read) = editor.mirror.read_log(0, 100);
    editor.send(read);

    harness.relay().offline(true);
    wait_until("noticed it is offline", |held| !held.connected);
    let message = be_type_chat::ChatEntry::Message {
        time: 0,
        body: "Can I still write here?".to_owned(),
        reply_to: None,
        attachments: Vec::new(),
    };
    let (_, append) = editor.mirror.append(message.encode(), Default::default());
    editor.send(append);
    editor.until("shows the message as sending", |editor| {
        !editor.kept().is_empty()
    });
    runtime
        .block_on(peer.grant(chat, harness.account, be_block_graph::Access::View))
        .expect("the sharer makes this account a viewer");

    harness.relay().offline(false);
    retry_now();
    editor.until("shows the message as not sent", |editor| {
        editor.kept().is_empty() && editor.mirror.refused().count() == 1
    });
    let (_, bytes, why) = editor.mirror.refused().next().unwrap();
    assert_eq!(be_type_chat::ChatEntry::decode(bytes), Some(message));
    assert!(!why.is_empty(), "it says why");
}
