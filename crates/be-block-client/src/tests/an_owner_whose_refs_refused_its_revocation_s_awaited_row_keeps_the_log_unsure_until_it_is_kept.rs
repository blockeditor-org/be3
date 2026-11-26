use super::*;

/// An owner's device revokes a reader, the server takes it, and the refs refuse the row that
/// keeps the revocation awaited. The log's unsure row stays: a restart then still reads the log
/// first rather than judge by the one kept from before. Once the refs take rows again, the next
/// read of the log writes the awaited row, even while the server withholds the revocation, and
/// the unsure row goes: a device restarted after that withholds the log served without it.
#[tokio::test]
async fn an_owner_whose_refs_refused_its_revocation_s_awaited_row_keeps_the_log_unsure_until_it_is_kept()
 {
    let harness = Harness::start().await;
    let admin = harness.owner("admin@example.com").await;
    let bob = harness.member(&admin, "bob@example.com").await;
    let team = workspace(&admin, "Team").await;
    admin
        .grant(team, bob.account(), Access::View)
        .await
        .unwrap();
    admin.tend().await.unwrap();

    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let device = harness.device(&admin, &refs, &store).await;
    device.checked_access(team).await.unwrap();
    refs.refuse(refs::Table::Awaited, true);
    device
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();
    let withheld = withhold_newest(&harness, admin.organization(), Some(team));

    // What a restart now would load, read by another device on the same refs.
    let restarted = harness.offline(&admin, &refs, &store);
    assert!(
        restarted.keys().is_unsure(team).is_some(),
        "a log whose awaited lowering the refs refused is unsure after a restart"
    );
    drop(restarted);

    refs.refuse(refs::Table::Awaited, false);
    assert!(
        matches!(device.checked_access(team).await, Err(ClientError::Withheld(block)) if block == team),
        "a log served without the awaited revocation is withheld"
    );
    drop(device);

    let restarted = harness.device(&admin, &refs, &store).await;
    assert!(
        restarted.keys().is_unsure(team).is_none(),
        "the unsure row went once the awaited row was kept"
    );
    assert!(
        matches!(restarted.checked_access(team).await, Err(ClientError::Withheld(block)) if block == team),
        "a restart withholds the log served without the revocation"
    );
    withheld.serve(&harness);
    restarted.checked_access(team).await.unwrap();
    harness.stop().await;
}
