use super::*;

/// A module appends bytes to its block's signed log and nothing more: the host signs them for
/// the person, as the module's app (`be_block_client::by_app`), so what reaches the server is
/// always signed, and verifies as that app's alone. Bytes a module makes up to pass for another
/// app's entry verify as nobody's there, and the replica is handed each entry with who signed it.
#[test]
fn what_a_module_appends_reaches_the_server_signed_by_the_host_as_its_apps() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = peer
            .create::<Ledger>(BlockParent::Root)
            .await
            .expect("the ledger is made");
        peer.save(block, &Ledger::default(), None)
            .await
            .expect("the empty ledger is published");
        replicas
            .open((&peer, &shared), block, Ledger::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        let editor = 7;
        let mut mirror =
            attach_mirror::<Ledger>(&peer, &shared, &mut replicas, block, editor).await;
        for said in [&b"hello"[..], b"forged"] {
            let (_, message) = mirror.append(said.to_vec(), be_block_content::Along::default());
            replicas.relay_from_editor(block, editor, message.encode(), std::time::Instant::now());
        }
        let mut answered = Vec::new();
        while answered.len() < 2 || mirror.log().count() < 2 {
            step(&peer, &shared, &mut replicas).await;
            receive_frames(&shared, &mut replicas, block, editor, &mut mirror);
            answered.extend(mirror.take_appended());
        }
        assert!(
            answered
                .iter()
                .all(|(_, result)| matches!(result, Ok(appended) if appended.place.is_some())),
            "{answered:?}"
        );
        let heard: Vec<_> = mirror.log().cloned().collect();
        assert_eq!(
            heard
                .iter()
                .map(|item| (item.payload.clone(), item.signer))
                .collect::<Vec<_>>(),
            [
                (
                    Some(b"hello".to_vec()),
                    be_block_content::Signer::Verified
                ),
                (
                    Some(b"forged".to_vec()),
                    be_block_content::Signer::Verified
                ),
            ],
            "the editor hears its entries, signed as its app's"
        );

        let read = peer.read_log(block, 0, 10).await.expect("the log reads");
        assert_eq!(read.entries.len(), 2);
        for logged in &read.entries {
            let entry = &logged.entry;
            let said = logged.opened.as_ref().expect("the entry opens");
            let as_app = |app: &'static str| {
                let peer = Arc::clone(&peer);
                let said = said.clone();
                let entry = entry.clone();
                async move {
                    peer.verify_entry(
                        block,
                        app,
                        entry.author,
                        entry.id,
                        entry.arrival,
                        &said,
                        &entry.signature,
                    )
                    .await
                    .expect("the signer is told")
                }
            };
            assert_eq!(
                as_app("test.ledger").await,
                be_block_content::Signer::Verified,
                "every entry on the server is signed as the module's app"
            );
            assert_eq!(
                as_app("be3.other").await,
                be_block_content::Signer::Bad,
                "an entry verifies as no other app's"
            );
            assert_eq!(entry.author, peer.account());
        }
        let errors = std::mem::take(&mut shared.lock().unwrap().errors);
        assert!(errors.is_empty(), "{errors:?}");
    });
}
