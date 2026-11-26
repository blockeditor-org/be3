use super::*;

#[test]
fn the_store_refuses_a_call_before_it_is_dispatched() {
    let mut store = Recorded {
        refuse: true,
        ..Recorded::default()
    };
    let dispatched = call(
        &mut store,
        INSTANCE,
        Call::Act {
            reply: reply(4),
            block: Uuid::from_u128(1).into_bytes(),
            command: BlockCommand::SetParent(BlockLocation::Root),
        },
    );
    assert_eq!(
        dispatched,
        Dispatched {
            reply: Some(Reply::Answer(4, Outcome::Err(OpError::Denied))),
            changed: false,
        }
    );
    open_replica(&mut store, 6, Uuid::from_u128(2));
    assert!(store.heard.is_empty(), "{:?}", store.heard);
    assert!(!store.blocks.holds(6), "a refused replica is not held");
}
