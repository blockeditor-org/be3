use super::*;

/// A device checked the team's log past Bob's revocation, then restarts, and the server serves
/// it the log only up to before the revocation: it takes nothing from that log, judges by what
/// it kept of the earlier one, and gives the team a new epoch rather than write under the key
/// Bob made up.
#[tokio::test]
async fn a_restarted_device_served_a_log_before_a_revocation_it_held_writes_under_no_made_up_key() {
    let harness = Harness::start().await;
    let revoked = revoked(&harness).await;
    let refs = refs::RefsDb::memory();
    let store = MemoryStore::new();
    let before = harness.device(&revoked.carol, &refs, &store).await;
    before.checked_access(revoked.team).await.unwrap();
    drop(before);

    let made_up = plant(&harness, &revoked, None);
    let withheld = withhold_newest(&harness, revoked.admin.organization(), Some(revoked.team));
    let restarted = harness.device(&revoked.carol, &refs, &store).await;
    saves_past(&revoked, &restarted, made_up).await;
    withheld.serve(&harness);

    harness.stop().await;
}
