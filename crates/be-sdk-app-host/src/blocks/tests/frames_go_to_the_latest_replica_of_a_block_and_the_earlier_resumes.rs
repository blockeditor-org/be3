use super::*;

#[test]
fn frames_go_to_the_latest_replica_of_a_block_and_the_earlier_resumes() {
    let mut store = Recorded::default();
    let block = Uuid::from_u128(7);
    let (first, second) = (Uuid::from_u128(1), Uuid::from_u128(2));
    open_replica_of(&mut store, 10, block, first);
    open_replica_of(&mut store, 11, block, second);
    assert_eq!(store.blocks.replica(block), Some(11), "frames go to the latest");
    assert_eq!(
        send(&mut store, 10, 20),
        Dispatched {
            reply: Some(Reply::Accepted(20)),
            changed: true,
        },
        "a frame sent from either reaches the block"
    );

    store.heard.clear();
    assert!(ended(&mut store, 11));
    assert_eq!(store.blocks.replica(block), Some(10));
    assert_eq!(
        store.heard,
        [format!("resumed 10 {block} {first}")],
        "the earlier replica starts afresh, of its own content type"
    );

    store.heard.clear();
    open_replica(&mut store, 12, block);
    assert!(ended(&mut store, 10));
    assert!(
        store.heard.iter().all(|heard| !heard.starts_with("resumed")),
        "an earlier replica ending changes nothing for the latest: {:?}",
        store.heard
    );
    assert!(ended(&mut store, 12));
    assert_eq!(store.heard.last(), Some(&format!("ended {block}")));
    assert_eq!(store.blocks.replica(block), None);
    assert!(!ended(&mut store, 12), "an op ends once");
    assert_eq!(
        send(&mut store, 12, 21),
        Dispatched {
            reply: Some(Reply::Accepted(21)),
            changed: false,
        },
        "a frame to a replica gone is taken in and dropped"
    );
}
