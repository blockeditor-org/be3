use super::*;

/// Who the server says hosts `block`, as `member` hears it on joining its session.
fn host_heard_by(member: &Member, block: Uuid) -> Option<Uuid> {
    let peer = &member.peer;
    member.runtime.block_on(async {
        peer.refresh_keys(None).await.expect("the keys arrive");
        peer.connection()
            .join(
                peer.organization(),
                block,
                be_block_store::Hash::from_bytes([3; 32]),
            )
            .await
            .expect("the member joins")
            .host
            .map(|host| host.account)
    })
}

/// A device whose replica of a hosted block is open (an editor, a child, a preview) holds the
/// session's lease, and seals, but hosts it only once a host component starts and claims it
/// (plans/hosted_blocks.md, step 4d2): until then the server tells members no one hosts. A
/// component that stops gives the claim up while the replica stays open, and once another
/// editor's device claims after it, it hosts no longer.
#[test]
fn a_device_hosts_only_once_its_component_claims_the_lease() {
    let harness = Harness::start();
    let personal = harness.connect_personal();
    let player = Member::join(&harness, personal, "player@example.com", "Player");
    let other = Member::join(&harness, personal, "other@example.com", "Other");
    let block = Uuid::new_v4();
    create(
        block,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("World"),
        None,
        false,
    );
    flush();
    open(block, CounterContent::CONTENT_TYPE);
    set_access(
        block,
        player.account(),
        be_block_graph::Access::Join,
        |_| {},
    );
    set_access(block, other.account(), be_block_graph::Access::Edit, |_| {});
    flush_and_send();
    assert_eq!(status().error, None);
    wait_until("joined the world's session", |shared| {
        shared.session_hosts.contains_key(&block)
    });
    assert!(
        may_host(block),
        "no one hosts, so a component may start here"
    );
    assert_eq!(
        host_heard_by(&player, block),
        None,
        "an open replica alone hosts nothing"
    );

    restart_host(block);
    wait_until("hosts the world", |shared| shared.hosted.contains(&block));
    assert_eq!(
        host_heard_by(&player, block),
        Some(harness.account),
        "the component's claim makes this device the host"
    );

    // The component stops while the replica stays open: the replica joins again, dropping the
    // claim, so no one hosts until a component starts again.
    release_host(block);
    wait_until("hosts no longer once its component stopped", |shared| {
        !shared.hosted.contains(&block) && shared.session_hosts.get(&block) == Some(&None)
    });
    assert_eq!(host_heard_by(&player, block), None);

    // Another editor's device opens the block's replica, running the type's build as this
    // device does.
    let Member { runtime, peer, .. } = other;
    let local = tokio::task::LocalSet::new();
    let session = local.block_on(&runtime, async move {
        be_tool_cli::replica::Session::open(peer, block, super::types::sources().await)
            .await
            .expect("the other editor opens the world")
    });
    let mut outside = Outside {
        runtime,
        local,
        session: Some(session),
    };

    // A component starts here and the other device claims right after, while this device's
    // claim may still be being answered: the later claim wins, and the answer to the earlier
    // one does not bring hosting back.
    restart_host(block);
    wait_until("hosts again", |shared| shared.hosted.contains(&block));
    outside.claim();
    wait_until("hosts no longer", |shared| {
        !shared.hosted.contains(&block) && shared.session_hosts.get(&block) == Some(&Some(false))
    });
    assert!(!may_host(block), "another device hosts");

    // Its component stops, as be-host stops one once another device hosts, while the replica
    // stays open: the claim it made goes, so once the other device leaves, no one hosts, rather
    // than this device by a claim no component stands behind.
    let joins = joins_of(block);
    release_host(block);
    wait_until("joined again with no claim", |shared| {
        shared.session_joins.get(&block).copied().unwrap_or(0) > joins
            && shared.session_hosts.get(&block) == Some(&Some(false))
    });
    outside.leave();
    wait_until("no one hosts once the other device left", |shared| {
        shared.session_hosts.get(&block) == Some(&None) && !shared.hosted.contains(&block)
    });
    assert_eq!(
        host_heard_by(&player, block),
        None,
        "an open replica whose claim went hosts nothing"
    );
}

/// How many times this device's replica of `block` has had its join answered so far.
fn joins_of(block: Uuid) -> u64 {
    crate::wait_until("read the joins", |shared| {
        Some(shared.session_joins.get(&block).copied().unwrap_or(0))
    })
}
