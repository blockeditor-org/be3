use super::*;

use be_sdk_api::blocks::MemberEvent;

/// A member whose handshake was checked before the signed logs stopped letting it in, while a
/// check of the members ran beside its answer and skipped it, is checked again once answered
/// and dropped (`hosting::answer`'s `missed`), with the server's word that it left held back,
/// so only the host's own check takes it out (guides/blocks/hosted.md, "Hosting").
#[test]
fn an_admit_a_recheck_ran_beside_is_checked_again() {
    let harness = Harness::start();
    let personal = harness.connect_personal();
    let viewer = Member::join(&harness, personal, "viewer@example.com", "Viewer");
    let block = Uuid::new_v4();
    let key = block.as_u128();
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
        viewer.account(),
        be_block_graph::Access::View,
        |_| {},
    );
    flush_and_send();
    assert_eq!(status().error, None);
    wait_until("joined the world's session", |shared| {
        shared.session_hosts.contains_key(&block)
    });
    restart_host(block);
    wait_until("hosts the world", |shared| shared.hosted.contains(&block));

    // The viewer's handshake is checked and let in, and its answer held before it goes.
    let peer = &viewer.peer;
    let organization = peer.organization();
    be_lib_test_pause::hold(crate::hosting::ANSWER_PAUSE, key);
    let (member, _joining) = viewer.runtime.block_on(async {
        peer.refresh_keys(None)
            .await
            .expect("the viewer's keys read");
        let joined = peer
            .connection()
            .join(
                organization,
                block,
                be_block_store::Hash::from_bytes([3; 32]),
            )
            .await
            .expect("the viewer joins");
        let host = joined.host.expect("this device hosts").account;
        let (joining, first) = peer
            .join_host(block, host)
            .await
            .expect("the viewer checks the host");
        peer.connection()
            .send_host(block, None, first)
            .await
            .expect("the handshake goes");
        (joined.client, joining)
    });
    viewer.runtime.block_on(be_lib_test_pause::reached(
        crate::hosting::ANSWER_PAUSE,
        key,
        1,
    ));

    // Banned, the server takes the viewer out, and its word of that is held back: only the
    // host's own check of the members, run on its ban while the answer is held, finds the
    // viewer no longer let in, and it has no admitted member to check then.
    be_lib_test_pause::hold(crate::hosting::LEAVE_PAUSE, key);
    let (banned, done) = std::sync::mpsc::channel();
    ban(block, viewer.account(), move |result| {
        let _ = banned.send(result);
    });
    done.recv()
        .expect("the worker answered")
        .expect("the viewer is banned");
    wait_until("checked the members beside the answer", |shared| {
        !crate::hosting::rechecking(shared)
    });

    // Let go, the answer sees a check ran beside it and asks for another, which drops the
    // viewer.
    be_lib_test_pause::release(crate::hosting::ANSWER_PAUSE, key);
    wait_until("checked the viewer again and dropped it", |shared| {
        !crate::hosting::rechecking(shared)
            && crate::hosting::member_state(shared, block, member).is_none()
    });
    be_lib_test_pause::release(crate::hosting::LEAVE_PAUSE, key);
    let queued = crate::with_shared(|shared| crate::hosting::queued(shared, block))
        .expect("the stack is running");
    assert!(
        !queued
            .iter()
            .any(|event| matches!(event, MemberEvent::Joined(joined) if joined.member == member)),
        "the viewer the logs bar is never admitted: {queued:?}"
    );
}
