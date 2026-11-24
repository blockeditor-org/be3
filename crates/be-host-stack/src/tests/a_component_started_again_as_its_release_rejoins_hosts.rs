use super::*;

/// A host component that stops and starts again at once, its release's rejoin still on its way
/// as the new one starts, ends up hosting (plans/hosted_blocks.md, step 4d2): the rejoin drops
/// whatever claim the device had, the join's answer says no one hosts, and as a component runs
/// on the block the device claims again.
#[test]
fn a_component_started_again_as_its_release_rejoins_hosts() {
    let harness = Harness::start();
    harness.connect_personal();
    let block = Uuid::new_v4();
    create(
        block,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("World"),
        None,
        false,
    );
    flush_and_send();
    open(block, CounterContent::CONTENT_TYPE);
    wait_until("joined the world's session", |shared| {
        shared.session_hosts.contains_key(&block)
    });
    restart_host(block);
    wait_until("hosts the world", |shared| shared.hosted.contains(&block));

    // Stopped and started again before the rejoin that drops the claim is answered: the new
    // component finds the device still hosting, so it claims nothing then.
    release_host(block);
    restart_host(block);
    wait_until("hosts again once the rejoin dropped the claim", |shared| {
        shared.hosted.contains(&block) && shared.session_hosts.get(&block) == Some(&Some(true))
    });
    assert!(may_host(block));
}
