use super::*;

/// A rejoin whose answer waits for the replica to settle, which only applying the replica's own
/// entry can do (its seal of it moves the head it reports), takes that entry as the order
/// numbers it rather than park it for after the answer: the seal follows, the answer is let
/// go, and the edit is there once.
#[test]
fn a_held_rejoin_takes_its_own_next_entry_and_lets_go_once_sealed() {
    with_test_types_on(
        be_block_client::Clock::manual(),
        |peer, shared, mut replicas| async move {
            let (block, said) =
                rejoin_held_before_its_own_entry(&peer, &shared, &mut replicas).await;
            let through = replicas.heard_through_of(block).expect("the order was heard");
            for event in &said {
                replicas.heard((&peer, &shared), event);
            }
            replicas.pump((&peer, &shared)).await;
            assert_eq!(
                replicas.heard_through_of(block),
                Some(through + 1),
                "its own next entry is taken while the answer is held"
            );
            replicas.seal_later(block, false);
            step_until_replicas(&peer, &shared, &mut replicas, "let the rejoin go", |replicas, _| {
                !replicas.joining()
            })
            .await;
            let bytes = replicas
                .content((&peer, &shared), block)
                .await
                .expect("the tally is open");
            let tally = Tally::decode(&bytes).expect("the tally decodes");
            assert_eq!(tally.count, 5, "both edits landed, once each");
            let errors = std::mem::take(&mut shared.lock().unwrap().errors);
            assert!(errors.is_empty(), "{errors:?}");
        },
    );
}
