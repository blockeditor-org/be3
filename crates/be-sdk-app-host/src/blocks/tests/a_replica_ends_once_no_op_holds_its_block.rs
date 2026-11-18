use super::*;

#[test]
fn a_replica_ends_once_no_op_holds_its_block() {
    let mut store = Recorded::default();
    let block = Uuid::from_u128(7);
    open_replica(&mut store, 10, block);
    open_replica(&mut store, 11, block);
    assert_eq!(store.blocks.replica(block), Some(11), "frames go to the latest");
    assert_eq!(
        send(&mut store, 10, 20),
        Dispatched {
            reply: Some(Reply::Accepted(20)),
            changed: true,
        },
        "a frame sent to either reaches the block"
    );

    assert!(ended(&mut store, 11));
    assert_eq!(store.blocks.replica(block), Some(10));
    assert!(
        !store.heard.iter().any(|heard| heard.starts_with("ended")),
        "the block is still held: {:?}",
        store.heard
    );
    assert!(ended(&mut store, 10));
    assert_eq!(store.heard.last(), Some(&format!("ended {block}")));
    assert_eq!(store.blocks.replica(block), None);
    assert!(!ended(&mut store, 10), "an op ends once");
    assert_eq!(
        send(&mut store, 10, 21),
        Dispatched {
            reply: Some(Reply::Accepted(21)),
            changed: false,
        },
        "a frame to a replica gone is taken in and dropped"
    );
}
