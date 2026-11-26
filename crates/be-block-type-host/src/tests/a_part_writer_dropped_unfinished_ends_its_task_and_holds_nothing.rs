use be_type_example_counter::CounterContent;

use super::*;

/// A module drops a part writer it wrote to before it finished it: the host hears the drop
/// and lets go of the writer's feed, so its task ends, and the part is never held. The
/// replica stays open, so only the drop ends it.
#[test]
fn a_part_writer_dropped_unfinished_ends_its_task_and_holds_nothing() {
    with_another_device(|peer, another, shared, _| async move {
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
        write_under(&peer, &shared, &mut replicas, block).await;

        let replica = replicas.replica_op(block).expect("the block is open");
        tell(Writer::Drop(replica));
        while UNWRITTEN.with_borrow(Option::is_none) {
            step(&peer, &shared, &mut replicas).await;
        }
        let unwritten = UNWRITTEN.take().expect("answered");
        assert!(
            unwritten.contains("at least one byte"),
            "a writer finished with nothing written fails: {unwritten}"
        );
        assert!(
            replicas.status(block).is_some(),
            "the replica is still open"
        );
        assert_eq!(
            replicas.writing(),
            0,
            "the dropped writer's feed is let go of, ending its task"
        );
        assert_unheld(&peer, &another, block).await;
    });
}
