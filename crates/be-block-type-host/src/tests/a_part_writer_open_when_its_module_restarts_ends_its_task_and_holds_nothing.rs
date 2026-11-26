use be_type_example_counter::CounterContent;

use super::*;

/// A module restarts while a part writer it wrote to is open: the host lets go of the
/// writer's feed with the module's old connection, so its task ends, and the part is never
/// held.
#[test]
fn a_part_writer_open_when_its_module_restarts_ends_its_task_and_holds_nothing() {
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

        replicas.modules.restart_type(CounterContent::CONTENT_TYPE);
        replicas.pump((&peer, &shared)).await;
        while replicas.joining() {
            step(&peer, &shared, &mut replicas).await;
        }
        replicas.pump((&peer, &shared)).await;
        assert!(replicas.status(block).is_some(), "the replica opened again");
        assert_eq!(
            replicas.writing(),
            0,
            "the writer's feed is let go of with its module, ending its task"
        );
        assert_unheld(&peer, &another, block).await;
    });
}
