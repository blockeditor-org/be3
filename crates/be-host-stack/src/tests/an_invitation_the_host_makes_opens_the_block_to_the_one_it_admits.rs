use super::*;

/// A contact link made with an offer is an invitation (guides/blocks/keys.md, "An invitation"):
/// accepting its contact half grants nothing, and opening its share half, with the same code,
/// opens the block to the one it admits.
#[test]
fn an_invitation_the_host_makes_opens_the_block_to_the_one_it_admits() {
    let harness = Harness::start();
    let guest = Someone::register(
        harness.server.directory(),
        &harness.url,
        "guest@example.com",
        "Guest",
    );
    let personal = harness.connect_personal();
    let block = Uuid::new_v4();
    written(block, "the plan");

    let code = made_link(Some((block, be_block_graph::Access::View)));
    stop();

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
    assert!(
        runtime.block_on(peer.summary(block)).is_err(),
        "the contact half grants nothing"
    );
    let (opened, _) = runtime
        .block_on(be_block_client::share_links::open_share_link(
            &peer.connection(),
            &guest.phrase.secret(),
            peer.account(),
            &code,
            Some((block, be_block_graph::Access::View)),
        ))
        .expect("the guest opens the share half");
    assert_eq!(opened.block, block);
    let peer = runtime.block_on(guest.peer(&harness.url, personal));
    assert_eq!(runtime.block_on(read_text(&peer, block)), "the plan");
}
