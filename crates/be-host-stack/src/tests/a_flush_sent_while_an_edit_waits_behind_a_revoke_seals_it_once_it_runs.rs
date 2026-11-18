use super::*;

/// An edit made while a revoke of its block is in the lane waits its turn behind it, and a flush
/// sent after the edit runs ahead of it: its seal cannot take the edit. Once the revoke ends and
/// the edit runs, it is sealed as the flush was asked, with no quiet spell waited for.
#[test]
fn a_flush_sent_while_an_edit_waits_behind_a_revoke_seals_it_once_it_runs() {
    let harness = Harness::relayed();
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
            be_block_client::contacts::Expected::Contact,
            &Default::default(),
        ))
        .expect("the guest accepts");
    wait_until("confirmed the guest as a contact", |shared| {
        shared
            .contacts
            .iter()
            .any(|contact| contact.account == guest.session.account && contact.public.is_some())
    });
    set_access(
        block,
        guest.session.account,
        be_block_graph::Access::Edit,
        |_| {},
    );
    flush_and_send();

    let mut editor = Editor::<be_type_text::TextContent>::attach(block);
    editor.wait("was sent the text", |_| true);
    let before = node(block).and_then(|node| node.head);

    // The revoke waits in the lane for the server, holding the block: the edit waits behind it,
    // and the flush, which needs no block, runs ahead of the edit.
    harness.relay().hold_answers(true);
    set_access(
        block,
        guest.session.account,
        be_block_graph::Access::None,
        |_| {},
    );
    type_into(&mut editor, 0, "Still ");
    flush();
    harness.relay().hold_answers(false);

    wait_until("sealed the edit the flush ran ahead of", |shared| {
        shared.unsynced().is_empty()
    });
    wait_until("the edit reached the server", |shared| {
        shared
            .graph
            .get(block)
            .is_some_and(|node| node.head.is_some() && node.head != before)
            && text_of(shared, block).as_deref() == Some("Still meet at noon")
    });
}
