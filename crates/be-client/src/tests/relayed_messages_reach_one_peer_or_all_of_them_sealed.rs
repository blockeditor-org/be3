use super::*;

use be_protocol::{ClientId, ServerMessage};
use counter_types::CounterContent;

async fn heard(
    peer: &Peer<MemoryStore>,
    events: &mut tokio::sync::broadcast::Receiver<ServerMessage>,
    block: Uuid,
    count: usize,
) -> Vec<(ClientId, Vec<u8>, Vec<u8>)> {
    let mut heard = Vec::new();
    let reached = tokio::time::timeout(PATIENCE, async {
        while heard.len() < count {
            if let ServerMessage::Relayed {
                block: relayed,
                from,
                payload,
            } = events.recv().await.unwrap()
                && relayed == block
            {
                let plain = peer.unseal(&payload).unwrap();
                heard.push((from, payload, plain));
            }
        }
    })
    .await;
    assert!(reached.is_ok(), "the relayed messages never arrived");
    heard
}

#[tokio::test]
async fn relayed_messages_reach_one_peer_or_all_of_them_sealed() {
    let harness = Harness::start().await;
    let phone = harness.owner("phone@example.com").await;
    let block = phone
        .create::<CounterContent>(BlockParent::Root)
        .await
        .unwrap();
    let laptop = harness.second(&phone).await;
    let tablet = harness.second(&phone).await;
    let mut on_phone = phone.connection().subscribe();
    let mut on_laptop = laptop.connection().subscribe();
    let mut on_tablet = tablet.connection().subscribe();
    let (phone_id, _) = phone.join_session(block).unwrap().await.unwrap();
    let (laptop_id, _) = laptop.join_session(block).unwrap().await.unwrap();
    tablet.join_session(block).unwrap().await.unwrap();

    let to_laptop = phone
        .relay(block, Some(laptop_id), b"to the laptop")
        .await
        .unwrap();
    let to_everyone = phone.relay(block, None, b"to everyone").await.unwrap();
    to_laptop.await.unwrap();
    to_everyone.await.unwrap();

    let laptop_heard = heard(&laptop, &mut on_laptop, block, 2).await;
    let tablet_heard = heard(&tablet, &mut on_tablet, block, 1).await;
    for (from, sealed, plain) in laptop_heard.iter().chain(&tablet_heard) {
        assert_eq!(*from, phone_id);
        assert_ne!(sealed, plain, "a relayed message crosses the server sealed");
    }
    let plain = |heard: &[(ClientId, Vec<u8>, Vec<u8>)]| {
        heard
            .iter()
            .map(|(.., plain)| plain.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        plain(&laptop_heard),
        [b"to the laptop".to_vec(), b"to everyone".to_vec()],
        "a peer hears what was sent to it in the order it was sent"
    );
    assert_eq!(plain(&tablet_heard), [b"to everyone".to_vec()]);
    while let Ok(event) = on_phone.try_recv() {
        assert!(
            !matches!(event, ServerMessage::Relayed { .. }),
            "a peer never hears itself"
        );
    }

    harness.stop().await;
}
