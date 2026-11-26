use be_type_example_counter::CounterContent;

use super::*;

/// The module stops again while the replica it restarted rejoins: the replica opened in its
/// place still waits for that rejoin with the edit made meanwhile, and the rejoin's answer
/// gives it every edit once, so the counter reads 7.
#[test]
fn an_edit_made_while_a_restarted_replica_rejoins_waits_on_through_a_second_restart() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = counter_with_an_edit_past_its_seal(&peer, &shared, &mut replicas).await;
        add_while_a_restarted_replica_rejoins(&peer, &shared, &mut replicas, block).await;
        replicas.modules.restart_type(CounterContent::CONTENT_TYPE);
        replicas.pump((&peer, &shared)).await;
        assert_eq!(
            counted(&peer, &shared, &mut replicas, block).await,
            3,
            "opened again, it still waits for the rejoin"
        );

        replicas.hold_rejoins(false);
        step_until_replicas(
            &peer,
            &shared,
            &mut replicas,
            "rejoined and journaled the edit",
            |replicas, _| {
                !replicas.joining()
                    && replicas
                        .status(block)
                        .is_some_and(|status| status.taken >= 1 && status.journaled)
            },
        )
        .await;
        assert_eq!(
            counted(&peer, &shared, &mut replicas, block).await,
            7,
            "each edit reaches the replica once"
        );
    });
}
