use super::*;

/// A command waits behind one deferred before it that needs any block it needs, not only the
/// block that one is about: a move into a folder goes before a command on that folder.
#[test]
fn a_command_on_a_block_waits_behind_a_deferred_move_into_it() {
    let (moved, folder) = (Uuid::new_v4(), Uuid::new_v4());
    let mut served = Served::default();
    served.holding.borrow_mut().insert(moved, 1);
    let deferred = served.defer(Command::SetParent {
        block: moved,
        parent: BlockParent::Block(folder),
    });
    assert!(
        deferred.is_none(),
        "the move waits for the lane to let go of its block"
    );
    assert!(
        served.defer(Command::WatchVersion(folder)).is_none(),
        "a command on the folder the move goes into waits behind it"
    );
    assert!(
        served.runnable().is_empty(),
        "nothing runs while the lane holds the moved block"
    );

    served.holding.borrow_mut().clear();
    let order: Vec<&'static str> = served.runnable().iter().map(Command::name).collect();
    assert_eq!(order, ["set parent", "watch version"]);
}
