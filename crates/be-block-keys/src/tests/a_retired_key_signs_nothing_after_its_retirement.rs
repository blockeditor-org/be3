use super::*;

#[test]
fn a_retired_key_signs_nothing_after_its_retirement() {
    let old = RecoveryPhrase::generate().secret();
    let new = RecoveryPhrase::generate().secret();
    let thief = RecoveryPhrase::generate().secret();
    let account = [1; 16];
    let block = [7; 16];
    let root = Entry::root(&old, account, LogKind::Block, block);
    let pinned = root.hash();
    let retired = Entry::retirement(&old, &new, account, block, pinned);
    let log = vec![root.clone(), retired.clone()];
    let checked = check_log(LogKind::Block, block, pinned, &log, &|_, _| None).unwrap();
    assert_eq!(checked.head, retired.hash());
    assert_eq!(checked.signers.get(&account), Some(&new.signing()));

    let again = Entry::retirement(&old, &thief, account, block, retired.hash());
    assert!(
        check_log(
            LogKind::Block,
            block,
            pinned,
            &[root.clone(), retired.clone(), again], &|_, _| None)
        .is_err(),
        "the old key retired the account again after its retirement"
    );
    let unendorsed = Entry::retirement(&thief, &thief, account, block, pinned);
    assert!(
        check_log(LogKind::Block, block, pinned, &[root.clone(), unendorsed], &|_, _| None).is_err(),
        "someone retired the account without its key"
    );
    let onward = Entry::retirement(&new, &thief, account, block, retired.hash());
    assert!(
        check_log(
            LogKind::Block,
            block,
            pinned,
            &[root.clone(), retired.clone(), onward], &|_, _| None)
        .is_ok(),
        "the new key retires in turn"
    );
    let misplaced = Entry::retirement(&old, &new, account, block, [0; 32]);
    assert!(
        check_log(LogKind::Block, block, pinned, &[root.clone(), misplaced], &|_, _| None).is_err(),
        "an entry chains to the one before it"
    );
    assert!(
        check_log(LogKind::Organization, block, pinned, &log, &|_, _| None).is_err(),
        "a block's log never checks as an organization's"
    );
    assert!(
        check_log(LogKind::Block, block, [0; 32], &log, &|_, _| None).is_err(),
        "nor from a root other than the pinned one"
    );
}
