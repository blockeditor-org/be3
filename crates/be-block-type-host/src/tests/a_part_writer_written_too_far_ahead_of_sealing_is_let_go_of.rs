use be_type_example_counter::CounterContent;

use crate::replicas::UNSEALED_WRITES;

use super::*;

/// A module writes to a part writer without waiting for its writes: the host takes them while
/// what it holds unsealed stays within `UNSEALED_WRITES`, refuses the write past it and lets go
/// of the writer, refusing what follows, so the host never holds more of it than that.
#[test]
fn a_part_writer_written_too_far_ahead_of_sealing_is_let_go_of() {
    with_test_types(|peer, shared, _| async move {
        let mut replicas = writing_replicas().await;
        let block = peer
            .create::<CounterContent>(BlockParent::Root)
            .await
            .expect("the counter is made");
        replicas
            .open((&peer, &shared), block, CounterContent::CONTENT_TYPE, true)
            .await
            .expect("the block opens");
        replicas.pump((&peer, &shared)).await;

        let replica = replicas.replica_op(block).expect("the block is open");
        let third = UNSEALED_WRITES / 3 + 1;
        tell(Writer::Open(replica, vec![third, third, third, 1]));
        while WROTE.with_borrow(Vec::len) < 4 {
            step(&peer, &shared, &mut replicas).await;
        }
        let mut wrote = WROTE.take();
        wrote.sort();
        assert_eq!(
            wrote,
            ["Err(Canceled)", "Err(Canceled)", "Ok(())", "Ok(())"],
            "two thirds of the bound are taken unsealed, and the write past it refused"
        );
        assert_eq!(replicas.writing(), 0, "the writer is let go of");
    });
}
