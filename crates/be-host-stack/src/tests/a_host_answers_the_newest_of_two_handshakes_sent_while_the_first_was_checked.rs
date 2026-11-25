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

/// A member that sends a second handshake while the host still checks its first (a reconnect,
/// its first message lost to it) waits on the second: the host answers only that one, once the
/// first's check ends, so the member's channel opens with the answer it waits for
/// (guides/blocks/hosted.md, "The channel").
#[test]
fn a_host_answers_the_newest_of_two_handshakes_sent_while_the_first_was_checked() {
    let harness = Harness::start();
    let personal = harness.connect_personal();
    let viewer = Member::join(&harness, personal, "viewer@example.com", "Viewer");
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
    let (member, host) = viewer.runtime.block_on(async {
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
        (
            joined.client,
            joined.host.expect("this device hosts").account,
        )
    });
    let handshake = || {
        viewer.runtime.block_on(async {
            let (joining, first) = peer
                .join_host(block, host)
                .await
                .expect("the viewer checks the host");
            peer.connection()
                .send_host(block, None, first)
                .await
                .expect("the handshake goes");
            joining
        })
    };

    // The first is checked and held there; the second comes while it is.
    be_lib_test_pause::hold(crate::hosting::ANSWER_PAUSE, block.as_u128());
    let _first = handshake();
    viewer.runtime.block_on(be_lib_test_pause::reached(
        crate::hosting::ANSWER_PAUSE,
        block.as_u128(),
        1,
    ));
    let second = handshake();
    wait_until("heard the second handshake", |shared| {
        crate::hosting::member_state(shared, block, member)
            == Some("answering, a newer handshake waiting")
    });
    be_lib_test_pause::release(crate::hosting::ANSWER_PAUSE, block.as_u128());

    // The first answer the member hears is the second's: the first is never answered.
    let mut channel = viewer.runtime.block_on(async {
        let answer = hosted(&mut events, block).await;
        second
            .finish(&answer)
            .expect("the host answers the newest handshake")
    });
    viewer.runtime.block_on(async {
        peer.connection()
            .send_host(block, None, channel.seal(b"here"))
            .await
            .expect("the first channel message goes");
    });
    let joined = MemberEvent::Joined(JoinedMember {
        member,
        account: viewer.account().into_bytes(),
        level: be_sdk_api::AccessLevel::View,
    });
    let heard = take_member_events_until("admitted the viewer", block, |events| {
        events.contains(&joined)
    });
    assert_eq!(
        heard.first(),
        Some(&joined),
        "the member is admitted once, by its newest handshake: {heard:?}"
    );
    assert!(
        !heard.contains(&MemberEvent::Left(member)),
        "no channel of the member broke: {heard:?}"
    );
}
