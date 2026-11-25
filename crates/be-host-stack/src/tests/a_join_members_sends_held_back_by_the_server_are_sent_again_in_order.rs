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

/// A Join member that tells its host more than the server lets one account send at once
/// (its burst, here a few, then `HOST_MESSAGES_PER_SECOND`) has its sends held back
/// (`slow-down`): each is sent again, the same bytes, before any after it, so the host hears
/// every one, in order, on a channel that never breaks (crates/be-host-stack joining.rs).
#[test]
fn a_join_members_sends_held_back_by_the_server_are_sent_again_in_order() {
    // The handshake and the channel's first message take two of the burst.
    let harness = Harness::holding_host_messages_after(4.0);
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
    want_channel(Owner::Editor(1), Some(block));
    let (member, mut admitted) = admit(&host, &mut events, block);
    wait_until("the channel is open", |shared| {
        shared.channel_hosts.get(&block) == Some(&host.account())
    });

    // Past the burst, so the server holds some back however fast they are sent.
    let told: Vec<Vec<u8>> = (0..12u32).map(|at| at.to_le_bytes().to_vec()).collect();
    for payload in &told {
        tell_host(block, payload.clone());
    }
    let heard = host.runtime.block_on(async {
        let mut heard = Vec::new();
        while heard.len() < told.len() {
            let (from, payload) = next_hosted(&mut events, block).await;
            assert_eq!(from, member);
            heard.push(
                admitted
                    .channel
                    .open(&payload)
                    .expect("in order, and sealed"),
            );
        }
        heard
    });
    assert_eq!(heard, told, "every message arrives once, in order");
    wait_until("the server held some back", |shared| {
        crate::joining::slowed(shared) > 0
    });
}
