use super::*;

/// No honest path reduces an administrator record (`proof_path`), and a reduced entry says
/// whether it is one (`Reduced::administers`), so `check_log` refuses a path that reduces one.
/// Else a server could serve an administrator taking a new phrase a log in which their own
/// promotion is reduced: their device would sign that the retirement retires a member
/// (`Retirement::administrator`), and the server would then serve readers a path with the
/// promotion whole and the retirement reduced, on which their stolen old key still grants on the
/// organization's authority.
#[test]
fn a_path_that_reduces_an_administrator_record_is_refused() {
    let alice = RecoveryPhrase::generate().secret();
    let bob = RecoveryPhrase::generate().secret();
    let (alice_id, bob_id) = ([1; 16], [2; 16]);
    let organization = [9; 16];
    let root = Entry::root(&alice, alice_id, LogKind::Organization, organization);
    let bob_administers = Entry::sign(
        &alice,
        alice_id,
        organization,
        Some(root.hash()),
        false,
        Record::Administrator(Administrator {
            account: bob_id,
            identity: bob.identity(),
            administrator: true,
        }),
    );
    let log = [root.clone(), bob_administers.clone()];
    let check = |steps: &[Step]| {
        check_log(
            LogKind::Organization,
            organization,
            root.hash(),
            steps,
            &|_| None,
        )
    };
    let whole: Vec<Step> = log.iter().cloned().map(Step::Entry).collect();
    let checked = check(&whole).unwrap();
    assert_eq!(
        checked.level_as_of(bob_id, &bob_administers.hash()),
        Some(Level::Manage)
    );
    assert!(
        matches!(
            proof_path(LogKind::Organization, &log, [7; 16], [], false).last(),
            Some(Step::Entry(_))
        ),
        "an honest path keeps the promotion whole"
    );

    // What the server serves Bob as he takes a new phrase: his promotion reduced, so the path
    // hides no lowering, reaches the head, and names him no level.
    let forged = vec![
        Step::Entry(root.clone()),
        Step::Reduced(bob_administers.reduced()),
    ];
    assert!(
        check(&forged).is_err(),
        "a path that reduces an administrator record is refused"
    );

    // Nor can the server pass the promotion off as another kind of record: the hash commits to
    // what it is.
    let passed_off = Reduced {
        administers: false,
        ..bob_administers.reduced()
    };
    assert_ne!(passed_off.hash(), bob_administers.hash());
}
