use super::*;

use be_block_client::Event;
use be_sdk_api::blocks::{JoinedMember, MemberEvent};

/// The next message on `block`'s host channel `events` hears.
async fn hosted(events: &mut tokio::sync::broadcast::Receiver<Event>, block: Uuid) -> Vec<u8> {
    loop {
        if let Event::Hosted {
            block: of, payload, ..
        } = events.recv().await.expect("the events go on")
            && of == block
        {
            return payload;
        }
    }
}

/// A check of the members (`hosting::recheck`) that finds one no longer let in, while the
/// member is revoked, taken out and let in again under the same client, account and key before
/// the check acts, drops only the admission it checked: the member admitted again since stays
/// (guides/blocks/hosted.md, "Hosting").
#[test]
fn a_recheck_drops_only_the_admission_it_checked() {
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

    let peer = &viewer.peer;
    let mut events = peer.connection().subscribe();
    let organization = peer.organization();
    // Joins the session and is admitted on a new channel: its client.
    let mut admit = |level: be_sdk_api::AccessLevel| {
        let (member, channel) = viewer.runtime.block_on(async {
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
            let mut channel = joining
                .finish(&hosted(&mut events, block).await)
                .expect("the host answers");
            peer.connection()
                .send_host(block, None, channel.seal(b"here"))
                .await
                .expect("the first channel message goes");
            (joined.client, channel)
        });
        let joined = MemberEvent::Joined(JoinedMember {
            member,
            account: viewer.account().into_bytes(),
            level,
        });
        take_member_events_until("admitted the viewer", block, |events| {
            events.contains(&joined)
        });
        (member, channel)
    };
    let (member, _first) = admit(be_sdk_api::AccessLevel::View);

    // A change of access here starts a check, which takes the viewer to check and is held.
    be_lib_test_pause::hold(crate::hosting::RECHECK_PAUSE, key);
    be_lib_test_pause::hold(crate::hosting::RECHECKED_PAUSE, key);
    set_access(
        block,
        viewer.account(),
        be_block_graph::Access::Edit,
        |_| {},
    );
    flush_and_send();
    viewer.runtime.block_on(be_lib_test_pause::reached(
        crate::hosting::RECHECK_PAUSE,
        key,
        1,
    ));

    // Revoked, the server takes the viewer out; the check finds it no longer let in.
    set_access(
        block,
        viewer.account(),
        be_block_graph::Access::None,
        |_| {},
    );
    flush_and_send();
    take_member_events_until("the viewer left", block, |events| {
        events.contains(&MemberEvent::Left(member))
    });
    be_lib_test_pause::release(crate::hosting::RECHECK_PAUSE, key);
    viewer.runtime.block_on(be_lib_test_pause::reached(
        crate::hosting::RECHECKED_PAUSE,
        key,
        1,
    ));

    // Let in again, it joins and is admitted again under the same client, account and key
    // before the check acts on what it found.
    set_access(
        block,
        viewer.account(),
        be_block_graph::Access::View,
        |_| {},
    );
    flush_and_send();
    let (again, _second) = admit(be_sdk_api::AccessLevel::View);
    assert_eq!(again, member, "the same connection, the same client");
    be_lib_test_pause::release(crate::hosting::RECHECKED_PAUSE, key);

    wait_until("checked the members, the viewer still admitted", |shared| {
        !crate::hosting::rechecking(shared)
            && crate::hosting::member_state(shared, block, member) == Some("admitted")
    });
    wait_until("read what the host component has yet to hear", |shared| {
        let queued = crate::hosting::queued(shared, block);
        assert!(
            !queued.contains(&MemberEvent::Left(member)),
            "the admission made since the check stays: {queued:?}"
        );
        true
    });
}
