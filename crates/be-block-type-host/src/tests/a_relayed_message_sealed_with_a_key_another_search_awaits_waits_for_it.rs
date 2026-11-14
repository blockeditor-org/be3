use be_block_client::Event;
use be_block_protocol::RelayKind;

use super::*;

/// A peer seals what it relays with its block's key as soon as it has it. When that key reaches
/// this device's search (`Peer::learn`) first, the search is still on its way back as the message
/// arrives: the message waits for keys of its own and is unsealed, not dropped as locked.
#[test]
fn a_relayed_message_sealed_with_a_key_another_search_awaits_waits_for_it() {
    with_another_device(|peer, phone, shared, mut replicas| async move {
        let block = block(&peer).await;
        replicas
            .open((&peer, &shared), block, Sections::CONTENT_TYPE, true)
            .await;
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }

        // A key of the account this device has not fetched: another device made it.
        let made = phone
            .create::<Sections>(BlockParent::Root)
            .await
            .expect("the other device makes a block");
        let key = phone.current_key(made).expect("the block has a key");
        let payload = phone
            .commits()
            .vault()
            .seal(key, b"from the phone")
            .expect("the phone seals with its key");
        // A search for it is on its way already.
        assert!(peer.first_search_for(key));

        replicas.heard(
            (&peer, &shared),
            &Event::Relayed {
                block,
                from: 2,
                kind: RelayKind::Peer,
                payload,
                received: 0,
            },
        );
        assert!(
            replicas.waits_for_keys(block),
            "the message waits for a search of its own: {:?}",
            shared.lock().unwrap().errors
        );
        while replicas.waits_for_keys(block) {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        let errors = std::mem::take(&mut shared.lock().unwrap().errors);
        assert!(errors.is_empty(), "the message was unsealed: {errors:?}");
    });
}
