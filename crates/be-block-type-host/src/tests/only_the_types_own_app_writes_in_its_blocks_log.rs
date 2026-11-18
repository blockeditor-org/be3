use super::*;

/// The host signs what a block's module appends as that module's app, so only that app's
/// editors may ask for it: an `Append` from another app's editor, which any app holding the
/// block can open, is refused and reaches neither the module nor the server.
#[test]
fn only_the_types_own_app_writes_in_its_blocks_log() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = peer
            .create::<Tally>(BlockParent::Root)
            .await
            .expect("the tally is made");
        peer.save(block, &Tally::default(), None)
            .await
            .expect("the empty tally is published");
        replicas
            .open((&peer, &shared), block, Tally::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        while replicas.joining() {
            replicas.woken().await;
            replicas.pump((&peer, &shared)).await;
        }
        let other = 8;
        let mut held = attach_mirror::<Tally>(&peer, &shared, &mut replicas, block, other).await;
        let own = 7;
        let mut mirror = attach_mirror::<Tally>(&peer, &shared, &mut replicas, block, own).await;
        let (_, message) = held.append(b"as tally".to_vec());
        replicas.relay_from_app(
            &shared,
            block,
            other,
            "be3.other",
            message.encode(),
            std::time::Instant::now(),
        );
        let (_, message) = mirror.append(b"mine".to_vec());
        replicas.relay_from_app(
            &shared,
            block,
            own,
            "test.tally",
            message.encode(),
            std::time::Instant::now(),
        );
        let mut refused = Vec::new();
        while refused.is_empty() || mirror.log().count() < 1 {
            step(&peer, &shared, &mut replicas).await;
            // Each editor's frames, taken once for both.
            let frames = std::mem::take(&mut shared.lock().unwrap().frames);
            for (to, frame) in frames {
                shared.lock().unwrap().frames.push((to, frame));
                match to {
                    to if to == other => receive_frames(&shared, other, &mut held),
                    to if to == own => receive_frames(&shared, own, &mut mirror),
                    _ => {
                        shared.lock().unwrap().frames.clear();
                    }
                }
            }
            refused.extend(held.take_appended());
        }
        assert!(
            matches!(refused.as_slice(), [(_, Err(_))]),
            "the other app is told no: {refused:?}"
        );
        let read = peer.read_log(block, 0, 10).await.expect("the log reads");
        assert_eq!(
            read.entries
                .iter()
                .map(|logged| logged.bytes().map(<[u8]>::to_vec))
                .collect::<Vec<_>>(),
            [Some(b"mine".to_vec())],
            "only the type's own app's entry reaches the server"
        );
        let errors = std::mem::take(&mut shared.lock().unwrap().errors);
        assert!(errors.is_empty(), "{errors:?}");
    });
}
