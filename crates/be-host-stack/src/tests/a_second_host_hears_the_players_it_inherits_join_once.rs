use super::*;

use be_block_client::Event;
use be_sdk_api::blocks::{JoinedMember, MemberEvent, MemberSaid};

/// The next message on `block`'s host channel that `events` hear.
async fn next_hosted(
    events: &mut tokio::sync::broadcast::Receiver<Event>,
    block: Uuid,
) -> (u64, Vec<u8>) {
    loop {
        match events.recv().await {
            Ok(Event::Hosted {
                block: of,
                from,
                payload,
                ..
            }) if of == block => return (from, payload),
            Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(closed) => panic!("the events ended: {closed}"),
        }
    }
}

/// Joins `block`'s session as `host`, an editor, claiming its lease if `claim`, and answers
/// what it hears there.
fn join_as_editor(
    host: &Member,
    block: Uuid,
    claim: bool,
) -> tokio::sync::broadcast::Receiver<Event> {
    let peer = &host.peer;
    let organization = peer.organization();
    host.runtime.block_on(async {
        let events = peer.connection().subscribe();
        peer.refresh_keys(None)
            .await
            .expect("the host's keys arrive");
        peer.connection()
            .join(
                organization,
                block,
                be_block_store::Hash::from_bytes([3; 32]),
            )
            .await
            .expect("the host joins");
        if claim {
            peer.connection()
                .claim(block)
                .await
                .expect("the host claims the lease");
        }
        events
    })
}

/// When the lease passes to this device, its host component inherits the players another
/// device hosted: each comes to it only by a handshake of its own, and the component hears it
/// join once, with what it says after. What a player sealed on the channel to the host before
/// reaches this one first (the server routes to whoever holds the lease) and is no handshake,
/// so the component hears nothing of it (guides/blocks/hosted.md, "Hosting").
#[test]
fn a_second_host_hears_the_players_it_inherits_join_once() {
    let harness = Harness::start();
    let personal = harness.connect_personal();
    let first = Member::join(&harness, personal, "first@example.com", "First");
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
    set_access(block, first.account(), be_block_graph::Access::Edit, |_| {});
    set_access(block, player.account(), be_block_graph::Access::Join, |_| {});
    flush_and_send();
    assert_eq!(status().error, None);
    let mut first_events = join_as_editor(&first, block, true);

    // The player joins and is admitted by the first host.
    let peer = &player.peer;
    let mut events = peer.connection().subscribe();
    let organization = peer.organization();
    let (member, joining) = player.runtime.block_on(async {
        // A Join member takes the log's root as served once a contact it verified signed a
        // grant to it there (guides/blocks/hosted.md, "Accepted limits").
        peer.refresh_contacts()
            .await
            .expect("the player's contacts read");
        peer.refresh_keys(None)
            .await
            .expect("the player's keys read");
        let joined = peer
            .connection()
            .join(
                organization,
                block,
                be_block_store::Hash::from_bytes([0; 32]),
            )
            .await
            .expect("the player joins");
        let host = joined.host.expect("the first host hosts").account;
        assert_eq!(host, first.account());
        let (joining, hello) = peer
            .join_host(block, host)
            .await
            .expect("the player checks the host");
        peer.connection()
            .send_host(block, None, hello)
            .await
            .expect("the handshake goes");
        (joined.client, joining)
    });
    let admitting = first.runtime.block_on(async {
        let (from, hello) = next_hosted(&mut first_events, block).await;
        assert_eq!(from, member);
        let (admitting, answer) = first
            .peer
            .admit(block, &hello)
            .await
            .expect("the player checks");
        first
            .peer
            .connection()
            .send_host(block, Some(member), answer)
            .await
            .expect("the answer goes");
        admitting
    });
    let mut old = player.runtime.block_on(async {
        let reply = next_hosted(&mut events, block).await.1;
        let mut channel = joining.finish(&reply).expect("the first host answers");
        peer.connection()
            .send_host(block, None, channel.seal(&[]))
            .await
            .expect("the first channel message goes");
        channel
    });
    first.runtime.block_on(async {
        let (_, confirming) = next_hosted(&mut first_events, block).await;
        admitting
            .confirm(&confirming)
            .expect("the first host admits the player");
    });

    // The first host leaves, and this device takes the lease as its component starts.
    first.runtime.block_on(async {
        first
            .peer
            .connection()
            .leave_session(block)
            .await
            .expect("the first host leaves");
    });
    open(block, CounterContent::CONTENT_TYPE);
    wait_until("joined the world's session", |shared| {
        shared.session_hosts.get(&block) == Some(&None)
    });
    restart_host(block);
    wait_until("hosts the world", |shared| shared.hosted.contains(&block));
    let host = player.runtime.block_on(async {
        loop {
            match events.recv().await {
                Ok(Event::Host {
                    block: of,
                    host: Some(host),
                    ..
                }) if of == block && host.account != first.account() => return host.account,
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(closed) => panic!("the events ended: {closed}"),
            }
        }
    });

    // Sealed on the old channel before the player heard, it reaches the new host first.
    player.runtime.block_on(async {
        peer.connection()
            .send_host(block, None, old.seal(b"to the first host"))
            .await
            .expect("the stale message goes");
        let (joining, hello) = peer
            .join_host(block, host)
            .await
            .expect("the player checks the new host");
        peer.connection()
            .send_host(block, None, hello)
            .await
            .expect("the handshake goes");
        let reply = next_hosted(&mut events, block).await.1;
        let mut channel = joining.finish(&reply).expect("the new host answers");
        peer.connection()
            .send_host(block, None, channel.seal(b"here"))
            .await
            .expect("the first channel message goes");
    });
    let joined = MemberEvent::Joined(JoinedMember {
        member,
        account: player.account().into_bytes(),
        level: be_sdk_api::AccessLevel::Join,
    });
    let heard = take_member_events_until("heard the player", block, |events| {
        events.contains(&joined)
    });
    assert_eq!(
        heard,
        vec![
            joined,
            MemberEvent::Said(MemberSaid {
                member,
                payload: b"here".to_vec(),
            }),
        ],
        "the component hears the player join once, then what it says"
    );
}
