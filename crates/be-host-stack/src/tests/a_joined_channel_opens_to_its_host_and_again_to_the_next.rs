use super::*;

use be_block_client::{Admitted, Event};
use be_sdk_api::blocks::ChannelEvent;

/// What `block`'s Join editors have yet to hear, once `ready` holds of it.
fn channel_until(what: &str, block: Uuid, ready: impl Fn(usize) -> bool) -> Vec<ChannelEvent> {
    wait_until(what, |shared| ready(shared.channel_news.len(block)));
    take_channel_events(block)
}

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

/// An editor here that joins a hosted block as a Join member (`want_channel`;
/// plans/hosted_blocks.md, "Hosting") handshakes with whoever holds its lease: it hears the
/// channel open to that account and what the host says, and what it tells the host arrives in
/// order. A message that does not open breaks the channel, and a stray one of the old channel
/// does not end the new handshake. When the host leaves and another account's device claims the
/// lease, the channel closes and is made again with the new host.
#[test]
fn a_joined_channel_opens_to_its_host_and_again_to_the_next() {
    let harness = Harness::start();
    let personal = harness.connect_personal();
    let first = Member::join(&harness, personal, "first@example.com", "First");
    let second = Member::join(&harness, personal, "second@example.com", "Second");
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
    for host in [&first, &second] {
        set_access(block, host.account(), be_block_graph::Access::Edit, |_| {});
    }
    flush_and_send();
    assert_eq!(status().error, None);

    // The first claims the lease and hosts, and the second stands by: a device hosts only by a
    // claim, as its host component starts (plans/hosted_blocks.md, step 4d2).
    let mut first_events = join_as_editor(&first, block, true);
    let mut second_events = join_as_editor(&second, block, false);
    let joiner = Owner::Editor(1);
    want_channel(joiner, Some(block));
    let (member, mut admitted) = admit(&first, &mut first_events, block);
    first.runtime.block_on(async {
        first
            .peer
            .connection()
            .send_host(
                block,
                Some(member),
                admitted.channel.seal(b"you see a door"),
            )
            .await
            .expect("the host's message goes");
    });
    let heard = channel_until("heard the first host", block, |count| count >= 2);
    assert_eq!(
        heard,
        vec![
            ChannelEvent::Open(first.account().into_bytes()),
            ChannelEvent::Said(b"you see a door".to_vec()),
        ]
    );
    assert_eq!(channel_host(block), Some(first.account()));

    tell_host(block, b"open the door".to_vec());
    tell_host(block, b"go through".to_vec());
    let told = first.runtime.block_on(async {
        let mut told = Vec::new();
        while told.len() < 2 {
            let (from, payload) = next_hosted(&mut first_events, block).await;
            assert_eq!(from, member);
            told.push(
                admitted
                    .channel
                    .open(&payload)
                    .expect("in order, and sealed"),
            );
        }
        told
    });
    assert_eq!(
        told,
        vec![b"open the door".to_vec(), b"go through".to_vec()]
    );

    // A message that does not open breaks the channel, and the member handshakes again. A
    // stray message of the old channel reaching it before the host's answer leaves that
    // handshake waiting for its answer, which then opens the channel again.
    first.runtime.block_on(async {
        let peer = &first.peer;
        peer.connection()
            .send_host(block, Some(member), vec![7; 40])
            .await
            .expect("the garbled message goes");
        let (from, again) = next_hosted(&mut first_events, block).await;
        assert_eq!(from, member, "the member handshakes again");
        peer.connection()
            .send_host(block, Some(member), admitted.channel.seal(b"stray"))
            .await
            .expect("the stray message goes");
        let (admitting, answer) = peer.admit(block, &again).await.expect("the member checks");
        peer.connection()
            .send_host(block, Some(member), answer)
            .await
            .expect("the answer goes");
        let (_, confirming) = next_hosted(&mut first_events, block).await;
        admitting
            .confirm(&confirming)
            .expect("the new channel opens");
    });
    let heard = channel_until("heard the channel made again", block, |count| count >= 2);
    assert_eq!(
        heard,
        vec![
            ChannelEvent::Closed,
            ChannelEvent::Open(first.account().into_bytes()),
        ],
        "a stray message of the old channel did not end the new handshake"
    );

    first.runtime.block_on(async {
        first
            .peer
            .connection()
            .leave_session(block)
            .await
            .expect("the first host leaves");
    });
    // The server says no one hosts once the first host left (the second has not claimed), and
    // the second hears it: only then is this device waited on, so a slow server does not count
    // against its wait.
    second.runtime.block_on(async {
        loop {
            match second_events.recv().await {
                Ok(Event::Host {
                    block: of,
                    host: None,
                    ..
                }) if of == block => break,
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                Err(closed) => panic!("the events ended: {closed}"),
            }
        }
    });
    let heard = channel_until("heard the first host go", block, |count| count >= 1);
    assert_eq!(
        heard.last(),
        Some(&ChannelEvent::Closed),
        "the channel to the first host closed: {heard:?}"
    );
    assert_eq!(channel_host(block), None);

    second.runtime.block_on(async {
        second
            .peer
            .connection()
            .claim(block)
            .await
            .expect("the second host claims the lease");
    });
    admit(&second, &mut second_events, block);
    let heard = channel_until("heard the second host", block, |count| count >= 1);
    assert_eq!(
        heard,
        vec![ChannelEvent::Open(second.account().into_bytes())]
    );
    assert_eq!(channel_host(block), Some(second.account()));

    want_channel(joiner, None);
    wait_until("left the channel", |shared| {
        !shared.channel_hosts.contains_key(&block)
    });
}
