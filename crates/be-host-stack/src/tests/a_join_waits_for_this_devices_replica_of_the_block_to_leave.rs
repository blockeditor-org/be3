use super::*;

use be_block_client::{Admitted, Event};

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

/// Admits, as `host`, which holds `block`'s lease, the member that handshakes with it: the
/// member's client and the channel to it.
fn admit(
    host: &Member,
    events: &mut tokio::sync::broadcast::Receiver<Event>,
    block: Uuid,
) -> (u64, Admitted) {
    let peer = &host.peer;
    host.runtime.block_on(async {
        let (member, first) = next_hosted(events, block).await;
        let (admitting, answer) = peer.admit(block, &first).await.expect("the member checks");
        peer.connection()
            .send_host(block, Some(member), answer)
            .await
            .expect("the answer goes");
        let (from, confirming) = next_hosted(events, block).await;
        assert_eq!(from, member);
        let (admitted, said) = admitting
            .confirm(&confirming)
            .expect("the member's first message opens");
        assert!(
            said.is_empty(),
            "the first message only confirms the channel"
        );
        (member, admitted)
    })
}

/// An editor here that joins a hosted block as a Join member while this device's replica of the
/// block is open waits for the replica to close and leave before it joins: the server knows one
/// member per connection, so a join beside the replica would take it out of the session, and
/// its leave would take the Join member out after (crates/be-host-stack joining.rs, `step`).
/// Once the replica has left, the join goes and the channel opens.
#[test]
fn a_join_waits_for_this_devices_replica_of_the_block_to_leave() {
    let harness = Harness::start();
    let personal = harness.connect_personal();
    let host = Member::join(&harness, personal, "host@example.com", "Host");
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
    set_access(block, host.account(), be_block_graph::Access::Edit, |_| {});
    flush_and_send();
    assert_eq!(status().error, None);
    let mut events = join_as_editor(&host, block, true);

    open(block, CounterContent::CONTENT_TYPE);
    wait_until("the replica is in the session", |shared| {
        shared.session_hosts.get(&block) == Some(&Some(false))
    });
    want_channel(Owner::Editor(1), Some(block));
    flush();
    wait_until("the join waits, and the replica stays in the session", |shared| {
        assert!(
            crate::joining::unjoined(shared, block),
            "the join waits while the replica is open"
        );
        assert_eq!(
            shared.session_hosts.get(&block),
            Some(&Some(false)),
            "the replica is still in the session"
        );
        true
    });

    close(block);
    let (member, mut admitted) = admit(&host, &mut events, block);
    wait_until("the channel is open", |shared| {
        shared.channel_hosts.get(&block) == Some(&host.account())
    });
    tell_host(block, b"here".to_vec());
    let said = host.runtime.block_on(async {
        let (from, payload) = next_hosted(&mut events, block).await;
        assert_eq!(from, member);
        admitted.channel.open(&payload).expect("in order, and sealed")
    });
    assert_eq!(said, b"here");
}
