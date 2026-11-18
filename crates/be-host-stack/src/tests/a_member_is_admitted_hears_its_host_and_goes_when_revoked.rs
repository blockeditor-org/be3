use super::*;

use be_sdk_api::blocks::{JoinedMember, MemberEvent, MemberSaid};

/// What `block`'s host component has yet to hear, once `ready` holds of it.
fn queued_until(what: &str, block: Uuid, ready: impl Fn(&[MemberEvent]) -> bool) -> Vec<MemberEvent> {
    wait_until(what, |shared| ready(&crate::hosting::queued(shared, block)));
    take_member_events(block)
}

/// While this device holds a hosted block's lease, a member that speaks on its host channel is
/// admitted once the handshake and its first message check (plans/hosted_blocks.md, "Hosting"):
/// the host component hears it join with its account and level, and what it says; what the
/// component tells it reaches it sealed on its channel; and once its grant is revoked it goes.
#[test]
fn a_member_is_admitted_hears_its_host_and_goes_when_revoked() {
    let harness = Harness::start();
    let personal = harness.connect_personal();
    let player = Member::join(&harness, personal, "player@example.com", "Player");
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
    set_access(block, player.account(), be_block_graph::Access::Join, |_| {});
    flush_and_send();
    assert_eq!(status().error, None);
    wait_until("held the world's lease", |shared| shared.hosted.contains(&block));

    let peer = &player.peer;
    let mut events = peer.connection().subscribe();
    let organization = peer.organization();
    let (mut channel, member) = player.runtime.block_on(async {
        peer.refresh_contacts().await.expect("the contacts read");
        let joined = peer
            .connection()
            .join(organization, block, be_block_store::Hash::from_bytes([3; 32]))
            .await
            .expect("the player joins");
        let host = joined.host.expect("this device hosts");
        let (joining, first) = peer
            .join_host(block, host.account)
            .await
            .expect("the player checks the host");
        peer.connection()
            .send_host(block, None, first)
            .await
            .expect("the first message goes");
        let answer = loop {
            if let be_block_client::Event::Hosted { block: of, payload, .. } =
                events.recv().await.expect("the events go on")
                && of == block
            {
                break payload;
            }
        };
        let mut channel = joining.finish(&answer).expect("the host answers");
        peer.connection()
            .send_host(block, None, channel.seal(b"move north"))
            .await
            .expect("the first channel message goes");
        (channel, joined.client)
    });

    let heard = queued_until("heard the player join", block, |events| events.len() >= 2);
    assert_eq!(
        heard,
        vec![
            MemberEvent::Joined(JoinedMember {
                member,
                account: player.account().into_bytes(),
                level: be_sdk_api::AccessLevel::Join,
            }),
            MemberEvent::Said(MemberSaid {
                member,
                payload: b"move north".to_vec(),
            }),
        ]
    );

    tell_member(block, member, b"you see a door".to_vec());
    let told = player.runtime.block_on(async {
        loop {
            if let be_block_client::Event::Hosted { block: of, payload, .. } =
                events.recv().await.expect("the events go on")
                && of == block
            {
                break channel.open(&payload).expect("the host's message opens");
            }
        }
    });
    assert_eq!(told, b"you see a door");

    set_access(block, player.account(), be_block_graph::Access::None, |_| {});
    flush_and_send();
    let heard = queued_until("heard the player go", block, |events| !events.is_empty());
    assert_eq!(heard, vec![MemberEvent::Left(member)]);
}
