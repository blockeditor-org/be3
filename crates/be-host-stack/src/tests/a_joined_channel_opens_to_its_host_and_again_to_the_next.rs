use super::*;

use be_block_client::{Admitted, Event};
use be_sdk_api::blocks::ChannelEvent;

/// What `block`'s Join editors have yet to hear, once `ready` holds of it.
fn channel_until(what: &str, block: Uuid, ready: impl Fn(usize) -> bool) -> Vec<ChannelEvent> {
    wait_until(what, |shared| {
        ready(shared.channel_news.get(&block).map_or(0, Vec::len))
    });
    take_channel_events(block)
}

/// The next message on `block`'s host channel `host` hears.
async fn next_hosted(
    events: &mut tokio::sync::broadcast::Receiver<Event>,
    block: Uuid,
) -> (u64, Vec<u8>) {
    loop {
        if let Event::Hosted {
            block: of,
            from,
            payload,
            ..
        } = events.recv().await.expect("the events go on")
            && of == block
        {
            return (from, payload);
        }
    }
}

/// Holds `block`'s lease as `host` and admits the member that handshakes with it: the member's
/// client and the channel to it.
fn host_and_admit(host: &Member, block: Uuid) -> (u64, Admitted) {
    let peer = &host.peer;
    let organization = peer.organization();
    host.runtime.block_on(async {
        let mut events = peer.connection().subscribe();
        peer.refresh_keys(None).await.expect("the host's keys arrive");
        peer.connection()
            .join(organization, block, be_block_store::Hash::from_bytes([3; 32]))
            .await
            .expect("the host joins");
        peer.connection().claim(block).await.expect("the host claims the lease");
        let (member, first) = next_hosted(&mut events, block).await;
        let (admitting, answer) = peer.admit(block, &first).await.expect("the member checks");
        peer.connection()
            .send_host(block, Some(member), answer)
            .await
            .expect("the answer goes");
        let (from, confirming) = next_hosted(&mut events, block).await;
        assert_eq!(from, member);
        let (admitted, said) = admitting
            .confirm(&confirming)
            .expect("the member's first message opens");
        assert!(said.is_empty(), "the first message only confirms the channel");
        (member, admitted)
    })
}

/// An editor here that joins a hosted block as a Join member (`want_channel`;
/// plans/hosted_blocks.md, "Hosting") handshakes with whoever holds its lease: it hears the
/// channel open to that account and what the host says, and what it tells the host arrives in
/// order. When the host leaves and another account's device takes the lease, the channel
/// closes and is made again with the new host.
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

    let joiner = Owner::Editor(1);
    want_channel(joiner, Some(block));
    let (member, mut admitted) = host_and_admit(&first, block);
    first.runtime.block_on(async {
        first
            .peer
            .connection()
            .send_host(block, Some(member), admitted.channel.seal(b"you see a door"))
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

    let mut events = first.peer.connection().subscribe();
    tell_host(block, b"open the door".to_vec());
    tell_host(block, b"go through".to_vec());
    let told = first.runtime.block_on(async {
        let mut told = Vec::new();
        while told.len() < 2 {
            let (from, payload) = next_hosted(&mut events, block).await;
            assert_eq!(from, member);
            told.push(admitted.channel.open(&payload).expect("in order, and sealed"));
        }
        told
    });
    assert_eq!(told, vec![b"open the door".to_vec(), b"go through".to_vec()]);

    first.runtime.block_on(async {
        first
            .peer
            .connection()
            .leave_session(block)
            .await
            .expect("the first host leaves");
    });
    let heard = channel_until("heard the first host go", block, |count| count >= 1);
    assert_eq!(heard, vec![ChannelEvent::Closed]);
    assert_eq!(channel_host(block), None);

    host_and_admit(&second, block);
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
