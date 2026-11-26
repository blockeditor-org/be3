use super::*;

/// The edit an editor made while a restarted replica rejoined waits for the join; the device
/// goes offline before the join is answered, so no join opens it on the server's journal: the
/// replica is given the edit then, in its made-up session, and shows it.
#[test]
fn an_edit_made_while_a_restarted_replica_rejoins_reaches_it_once_the_device_goes_offline() {
    with_test_types(|peer, shared, mut replicas| async move {
        let block = counter_with_an_edit_past_its_seal(&peer, &shared, &mut replicas).await;
        add_while_a_restarted_replica_rejoins(&peer, &shared, &mut replicas, block).await;
        replicas.disconnected();
        replicas.hold_rejoins(false);
        assert_eq!(
            counted(&peer, &shared, &mut replicas, block).await,
            7,
            "offline, the replica is given the edit it waited for"
        );
    });
}
