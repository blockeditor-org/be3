use super::*;

/// After a restart the replica knows only the outbox's last commit, so what the outbox's two
/// kept commits add stays sending until a drain names that last one: a drain of the first
/// leaves the second's edits sending.
#[test]
fn edits_restored_after_a_restart_stay_sending_until_the_outbox_tip_drains() {
    let (first, tip) = (commit(1), commit(2));
    let mut lacking = Lacking::default();
    lacking.kept_unsent(vec![vec![1], vec![2]], tip);
    assert_eq!(sending(&lacking), 2);

    lacking.drained(false, &[first]);
    assert_eq!(sending(&lacking), 2, "the second kept commit has not drained");

    lacking.drained(false, &[tip]);
    assert_eq!(sending(&lacking), 0);
}
