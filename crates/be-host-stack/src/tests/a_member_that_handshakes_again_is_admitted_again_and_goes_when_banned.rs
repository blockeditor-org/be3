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

/// A member of a hosted block's channel that starts a new handshake over a channel it holds
/// (a reconnect, a message lost) is taken as one: its old channel goes and it is admitted again
/// (plans/hosted_blocks.md, "Hosting"). A viewer another editor bans goes too, though no grant
/// of its changed: the server takes it out of the session, and the host drops its channel.
#[test]
fn a_member_that_handshakes_again_is_admitted_again_and_goes_when_banned() {
    let harness = Harness::start();
    let personal = harness.connect_personal();
    let viewer = Member::join(&harness, personal, "viewer@example.com", "Viewer");
    let editor = Member::join(&harness, personal, "editor@example.com", "Editor");
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
    set_access(block, viewer.account(), be_block_graph::Access::View, |_| {});
    set_access(block, editor.account(), be_block_graph::Access::Edit, |_| {});
    flush_and_send();
    assert_eq!(status().error, None);
    wait_until("held the world's lease", |shared| shared.hosting.holds(block));

    let peer = &viewer.peer;
    let mut events = peer.connection().subscribe();
    let organization = peer.organization();
    let handshake = |events: &mut tokio::sync::broadcast::Receiver<Event>, host: Uuid| {
        viewer.runtime.block_on(async {
            let (joining, first) = peer
                .join_host(block, host)
                .await
                .expect("the viewer checks the host");
            peer.connection()
                .send_host(block, None, first)
                .await
                .expect("the first message goes");
            let mut channel = joining
                .finish(&hosted(events, block).await)
                .expect("the host answers");
            peer.connection()
                .send_host(block, None, channel.seal(b"here"))
                .await
                .expect("the first channel message goes");
            channel
        })
    };
    let (member, host) = viewer.runtime.block_on(async {
        peer.refresh_keys(None).await.expect("the viewer's keys read");
        let joined = peer
            .connection()
            .join(organization, block, be_block_store::Hash::from_bytes([3; 32]))
            .await
            .expect("the viewer joins");
        (joined.client, joined.host.expect("this device hosts").account)
    });
    let _first = handshake(&mut events, host);
    let joined = MemberEvent::Joined(JoinedMember {
        member,
        account: viewer.account().into_bytes(),
        level: be_sdk_api::AccessLevel::View,
    });
    wait_until("admitted the viewer", |shared| {
        shared.hosting.queued(block).contains(&joined)
    });
    take_member_events(block);

    let _second = handshake(&mut events, host);
    wait_until("admitted the viewer again", |shared| {
        shared.hosting.queued(block).contains(&joined)
    });
    let heard = take_member_events(block);
    assert_eq!(
        heard.first(),
        Some(&MemberEvent::Left(member)),
        "the old channel goes before the new one: {heard:?}"
    );

    editor.runtime.block_on(async {
        editor.peer.refresh_keys(None).await.expect("the editor's keys read");
        editor
            .peer
            .ban(block, viewer.account())
            .await
            .expect("the editor bans the viewer");
    });
    wait_until("dropped the banned viewer", |shared| {
        shared.hosting.queued(block).contains(&MemberEvent::Left(member))
    });
}
