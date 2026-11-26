use super::*;

/// A retirement in an organization's log says whether it retired an administrator
/// (`Retirement::administrator`), which whole-log checkers hold it to, and its reduced form
/// carries that (`Reduced::retires_administrator`). A server that reduces an administrator's
/// retirement on the organization's path it serves a reader below would leave the stolen old key
/// looking current, so `answer` takes no authority from a path that hides one. A path may still
/// hide a member's retirement, whose old key grants nothing on the organization's authority.
#[test]
fn authority_from_an_organization_path_that_hides_an_administrators_retirement_is_refused() {
    let alice = RecoveryPhrase::generate().secret();
    let bob = RecoveryPhrase::generate().secret();
    let bob_new = RecoveryPhrase::generate().secret();
    let carol = RecoveryPhrase::generate().secret();
    let dana = RecoveryPhrase::generate().secret();
    let dana_new = RecoveryPhrase::generate().secret();
    let (alice_id, bob_id, carol_id, dana_id) = ([1; 16], [2; 16], [3; 16], [4; 16]);
    let (organization, note) = ([9; 16], [12; 16]);

    // Alice's organization: Bob administers it and retires his keys; Dana, a member, retires
    // hers.
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
    let code = InviteCode::generate();
    let invited = Entry::sign(
        &alice,
        alice_id,
        organization,
        Some(bob_administers.hash()),
        false,
        Record::Invitation(Invitation {
            key: code.invitation_key(organization),
            open: true,
        }),
    );
    let dana_joins = Entry::accepted(&code, organization, invited.hash(), dana_id, &dana);
    let bob_retires = |administrator| {
        Entry::retirement(
            &bob,
            &bob_new,
            bob_id,
            organization,
            dana_joins.hash(),
            administrator,
        )
    };
    let before = vec![
        root.clone(),
        bob_administers,
        invited,
        dana_joins.clone(),
    ];
    let check = |entries: &[Entry]| {
        check_log(
            LogKind::Organization,
            organization,
            root.hash(),
            entries,
            &|_| None,
        )
    };
    assert!(
        check(&[before.clone(), vec![bob_retires(false)]].concat()).is_err(),
        "an administrator's retirement that says it retires a member is refused"
    );
    let bob_retired = bob_retires(true);
    let dana_retired = Entry::retirement(
        &dana,
        &dana_new,
        dana_id,
        organization,
        bob_retired.hash(),
        false,
    );
    let organization_log = [before, vec![bob_retired.clone(), dana_retired.clone()]].concat();
    assert!(check(&organization_log).is_ok());
    // A path cannot pass Bob's retirement off, reduced, as a member's: its hash commits to the bit.
    let passed_off: Vec<Step> = organization_log
        .iter()
        .map(|entry| match entry.record {
            Record::Retirement(_) if entry.signer == bob_id => Step::Reduced(Reduced {
                retires_administrator: false,
                ..entry.reduced()
            }),
            _ => Step::Entry(entry.clone()),
        })
        .collect();
    assert!(
        check_log(
            LogKind::Organization,
            organization,
            root.hash(),
            &passed_off,
            &|_| None,
        )
        .is_err(),
        "a reduced administrator's retirement that says it retires a member is refused"
    );

    // Alice's note, which Bob grants Carol as an administrator, signing with `key`.
    let note_root = Entry::root(&alice, alice_id, LogKind::Block, note);
    let check_note = |organization_steps: Vec<Step>, key: &RecoverySecret| {
        let to_carol = Entry::sign(
            key,
            bob_id,
            note,
            Some(note_root.hash()),
            false,
            Record::Grant(Grant {
                grantee: carol_id,
                identity: carol.identity(),
                level: Level::View,
                authority: Authority::Organization(dana_retired.hash()),
            }),
        );
        let log = move |kind: LogKind, log: [u8; 16]| {
            (kind == LogKind::Organization && log == organization)
                .then(|| organization_steps.clone())
        };
        let logs = Logs::new(organization, Some(root.hash()), &log);
        let outside = |asked: &Asked| answer(&logs, note, asked);
        check_log(
            LogKind::Block,
            note,
            note_root.hash(),
            &[note_root.clone(), to_carol],
            &outside,
        )
    };

    // The honest path keeps Bob's retirement whole and reduces Dana's: the new key grants, the
    // old one does not.
    let honest = proof_path(
        LogKind::Organization,
        &organization_log,
        carol_id,
        [bob_id],
        false,
    );
    assert_eq!(
        honest.last(),
        Some(&Step::Reduced(dana_retired.reduced())),
        "the path reduces Dana's retirement"
    );
    let partial = check_log(
        LogKind::Organization,
        organization,
        root.hash(),
        &honest,
        &|_| None,
    )
    .unwrap();
    assert!(partial.partial && !partial.hides_retirement);
    assert!(
        check_note(honest.clone(), &bob_new).is_ok(),
        "Bob's new key grants"
    );
    assert!(
        check_note(honest, &bob).is_err(),
        "Bob's retired key grants nothing after its retirement"
    );

    // A server reduces Bob's retirement too, so the path alone names his old key: refused.
    let forged: Vec<Step> = organization_log
        .iter()
        .map(|entry| match entry.record {
            Record::Retirement(_) => Step::Reduced(entry.reduced()),
            _ => Step::Entry(entry.clone()),
        })
        .collect();
    let partial = check_log(
        LogKind::Organization,
        organization,
        root.hash(),
        &forged,
        &|_| None,
    )
    .unwrap();
    assert_eq!(
        partial.signers.get(&bob_id),
        Some(&bob.identity().signing),
        "the path alone names Bob's old key"
    );
    assert!(partial.hides_retirement);
    assert!(
        check_note(forged, &bob).is_err(),
        "a stolen retired administrator's key grants nothing on a path that reduces its retirement"
    );
}
