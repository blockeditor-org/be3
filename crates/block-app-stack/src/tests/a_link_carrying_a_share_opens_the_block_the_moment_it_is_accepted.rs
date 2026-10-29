use super::*;

#[test]
fn a_link_carrying_a_share_opens_the_block_the_moment_it_is_accepted() {
    let harness = Harness::start();
    let guest = Someone::register(
        &harness.directory.join("server"),
        &harness.url,
        "guest@example.com",
        "Guest",
    );
    let personal = harness.connect_personal();
    let block = Uuid::new_v4();
    written(block, "the plan");

    let code = made_link(Some((block, be_graph::Access::View)));
    stop();

    let runtime = runtime();
    let peer = runtime.block_on(guest.peer(&harness.url, personal));
    assert!(
        runtime.block_on(peer.summary(block)).is_err(),
        "the guest saw the block before accepting"
    );
    runtime
        .block_on(be_client::contacts::accept_link(
            &peer.connection(),
            &guest.phrase.secret(),
            &guest.me().card(),
            &code,
        ))
        .expect("the guest accepts");
    assert_eq!(runtime.block_on(read_text(&peer, block)), "the plan");
}
