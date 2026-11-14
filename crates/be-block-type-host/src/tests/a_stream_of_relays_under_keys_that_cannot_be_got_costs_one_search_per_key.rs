use be_block_client::Event;
use be_block_protocol::RelayKind;

use super::*;

/// What a peer relays sealed with a key this device can never get (one the server was never
/// given) costs one search for that key, however many such messages arrive: each waits for that
/// search, then is let go and recorded. A message under another such key that arrived meanwhile
/// waits for a search of its own rather than being let go with them.
#[test]
fn a_stream_of_relays_under_keys_that_cannot_be_got_costs_one_search_per_key() {
    with_another_device(|peer, phone, shared, mut replicas| async move {
        let block = block(&peer).await;
        replicas
            .open((&peer, &shared), block, Sections::CONTENT_TYPE, true)
            .await;
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        shared.lock().unwrap().errors.clear();

        // Kept only on the phone: their keys never reach the server.
        let unsent_key = || async {
            phone
                .keep_creation(
                    Uuid::new_v4(),
                    Sections::CONTENT_TYPE,
                    BlockParent::Root,
                    &be_block_content::BlockMetadata::default(),
                    false,
                )
                .await
                .expect("the phone keeps a block")
                .key
                .expect("the block has a key")
        };
        let (first, second) = (unsent_key().await, unsent_key().await);
        let relayed = |key: Uuid, text: &[u8]| Event::Relayed {
            block,
            from: 2,
            kind: RelayKind::Peer,
            payload: phone
                .commits()
                .vault()
                .seal(key, text)
                .expect("the phone seals with its key"),
            received: 0,
        };

        for _ in 0..5 {
            replicas.heard((&peer, &shared), &relayed(first, b"presence"));
        }
        replicas.heard((&peer, &shared), &relayed(second, b"presence"));
        assert_eq!(replicas.key_searches(), 1, "one search for the first key");
        while replicas.waits_for_keys(block) {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        assert_eq!(
            replicas.key_searches(),
            2,
            "the message under the second key waited for a search of its own"
        );
        assert_eq!(shared.lock().unwrap().errors.len(), 6, "each was let go");

        for _ in 0..3 {
            replicas.heard((&peer, &shared), &relayed(first, b"presence"));
        }
        assert!(!replicas.waits_for_keys(block));
        assert_eq!(
            replicas.key_searches(),
            2,
            "a key searched for is not again"
        );
        assert_eq!(shared.lock().unwrap().errors.len(), 9);
        assert!(replicas.heard_from_peers().is_empty());
    });
}
