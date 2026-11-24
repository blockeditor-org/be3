use super::*;

/// What the server numbers next while a rejoin is held is taken early only if it is the
/// replica's own submission made on the last entry it heard: an entry that only happens to
/// carry the next number, as one of an order the server started afresh may, waits for the
/// answer, which reads that order.
#[test]
fn a_held_rejoin_takes_no_entry_another_order_numbered_next() {
    with_test_types_on(
        be_block_client::Clock::manual(),
        |peer, shared, mut replicas| async move {
            let (block, said) =
                rejoin_held_before_its_own_entry(&peer, &shared, &mut replicas).await;
            let through = replicas.heard_through_of(block).expect("the order was heard");
            for event in &said {
                let mut event = event.clone();
                if let be_block_client::Event::Sequenced { entry, .. } = &mut event {
                    entry.origin = entry.origin.wrapping_add(1);
                }
                replicas.heard((&peer, &shared), &event);
            }
            replicas.pump((&peer, &shared)).await;
            assert_eq!(
                replicas.heard_through_of(block),
                Some(through),
                "another origin's entry at the next number waits for the answer"
            );
            assert!(replicas.joining(), "and the answer stays held");
        },
    );
}
