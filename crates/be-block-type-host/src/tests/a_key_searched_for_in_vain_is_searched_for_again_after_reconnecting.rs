use be_block_client::Event;
use be_block_protocol::RelayKind;

use super::*;

/// A search that answered without a key, as one cut short by a dropped connection does, lets go
/// what waited on it; after reconnecting, the server may have the key, so what a peer relays
/// under it waits for a new search and is unsealed rather than let go at once.
#[test]
fn a_key_searched_for_in_vain_is_searched_for_again_after_reconnecting() {
    with_another_device(|peer, phone, shared, mut replicas| async move {
        let block = block(&peer).await;
        replicas
            .open((&peer, &shared), block, Sections::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }

        // A key of the block the phone mints and keeps to itself, so the server does not have
        // it yet.
        let summary = phone
            .summary(block)
            .await
            .expect("the phone reads the block");
        phone
            .learn(&[summary])
            .await
            .expect("the phone learns its keys");
        let scope = phone
            .keys()
            .scope_key(block)
            .expect("the phone holds its scope key");
        let (key, minted) = phone
            .keys()
            .mint_data(block, scope)
            .expect("the phone mints a key");
        let relayed = |text: &[u8]| Event::Relayed {
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
        replicas.heard((&peer, &shared), &relayed(b"before"));
        while replicas.waits_for_keys(block) {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        assert_eq!(replicas.key_searches(), 1);
        assert!(replicas.heard_from_peers().is_empty(), "let go: no key yet");

        be_block_protocol::client::keys::put_keys(&phone.connection(), minted)
            .await
            .expect("the server answers")
            .expect("the server takes the key");
        replicas.resynchronize((&peer, &shared));
        replicas.heard((&peer, &shared), &relayed(b"after"));
        assert!(
            replicas.waits_for_keys(block),
            "after reconnecting, the key is searched for again"
        );
        while replicas.waits_for_keys(block) {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        assert_eq!(replicas.key_searches(), 2);
        assert_eq!(replicas.heard_from_peers(), [b"after".to_vec()]);
    });
}
