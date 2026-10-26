use super::*;

const CURSOR: Uuid = Uuid::from_u128(0xc0);

#[test]
fn presence_crosses_between_this_peer_and_another() {
    let harness = Harness::start();
    let block = Uuid::new_v4();
    let mut outside = Outside::counter(&harness, block);

    harness.connect();
    open(block, CounterContent::CONTENT_TYPE);
    wait_for_count(block, 0);

    show(block, CURSOR, Some(vec![1]));
    outside.until("saw this peer's cursor", |seen| {
        seen.presence
            .iter()
            .any(|peer| peer.kind == CURSOR && peer.value == [1])
    });

    outside.show(CURSOR, Some(vec![2]));
    wait_until("saw the other peer's cursor", |shared| {
        shared.presence.get(&block).is_some_and(|(_, peers)| {
            peers
                .iter()
                .any(|peer| peer.kind == CURSOR && peer.value == [2])
        })
    });

    show(block, CURSOR, None);
    outside.until("forgot this peer's cursor", |seen| seen.presence.is_empty());
}
