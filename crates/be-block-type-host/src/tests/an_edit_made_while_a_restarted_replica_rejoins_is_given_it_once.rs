use super::*;

/// An editor adds 1, which a seal keeps, and 2, which the order holds past it. The type's
/// module stops, and while the replica opened again rejoins its session (its answer held) the
/// editor adds 4. The replica takes none of it until the join opens it on the server's journal,
/// so it submits nothing that opening is then given again: the counter reads 7, not 11.
#[test]
fn an_edit_made_while_a_restarted_replica_rejoins_is_given_it_once() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = counter_with_an_edit_past_its_seal(&peer, &shared, &mut replicas).await;
        add_while_a_restarted_replica_rejoins(&peer, &shared, &mut replicas, block).await;
        // Its module answers what it shows only after what it was sent before.
        assert_eq!(
            counted(&peer, &shared, &mut replicas, block).await,
            3,
            "the replica rejoining is given no edit yet"
        );
        assert_eq!(
            replicas.status(block).map(|status| status.taken),
            Some(0),
            "and took none"
        );
        assert_eq!(
            replicas.submissions_of(block).map(|(submitting, ..)| submitting),
            Some(0),
            "so it submits none"
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
            "each edit reaches the replica opened again once"
        );
    });
}
