use super::*;

/// A key retired in a block's log signs nothing after its retirement, there or below on
/// authority from it. A server that reduces the retirement on the path it serves a reader below
/// would leave the old key looking current there, so `answer` takes no authority from a path
/// that hides a retirement (`Reduced::retires`). An honest path of a log that records below rest
/// on keeps every retirement whole (`proof_path`'s `lowerings`): the old key's grant is refused
/// by it and the new key's checks.
#[test]
fn authority_from_a_path_that_hides_a_retirement_is_refused() {
    let alice = RecoveryPhrase::generate().secret();
    let bob = RecoveryPhrase::generate().secret();
    let bob_new = RecoveryPhrase::generate().secret();
    let carol = RecoveryPhrase::generate().secret();
    let erin = RecoveryPhrase::generate().secret();
    let (alice_id, bob_id, carol_id, erin_id) = ([1; 16], [2; 16], [3; 16], [5; 16]);
    let (folder, note) = ([10; 16], [12; 16]);
    let grant = |by: &RecoverySecret,
                 signer,
                 log,
                 previous: &Entry,
                 grantee,
                 identity,
                 level,
                 authority| {
        Entry::sign(
            by,
            signer,
            log,
            Some(previous.hash()),
            false,
            Record::Grant(Grant {
                grantee,
                identity,
                level,
                authority,
            }),
        )
    };

    // Alice's folder: Bob manages it, and then retires his keys; Erin edits it.
    let folder_root = Entry::root(&alice, alice_id, LogKind::Block, folder);
    let bob_manages = grant(
        &alice,
        alice_id,
        folder,
        &folder_root,
        bob_id,
        bob.identity(),
        Level::Manage,
        Authority::Log,
    );
    let bob_retires = Entry::retirement(&bob, &bob_new, bob_id, folder, bob_manages.hash());
    let erin_edits = grant(
        &alice,
        alice_id,
        folder,
        &bob_retires,
        erin_id,
        erin.identity(),
        Level::Edit,
        Authority::Log,
    );
    let folder_log = vec![
        folder_root.clone(),
        bob_manages,
        bob_retires,
        erin_edits.clone(),
    ];

    // Alice's note in the folder, which Bob grants Carol as a manager of the folder, signing
    // with `key`.
    let note_root = Entry::root(&alice, alice_id, LogKind::Block, note);
    let note_placed = Entry::sign(
        &alice,
        alice_id,
        note,
        Some(note_root.hash()),
        false,
        Record::Parent(Parent {
            placement: Placement::Block {
                block: folder,
                root: folder_root.hash(),
            },
            authority: Authority::Log,
        }),
    );
    let check_note = |folder_steps: Vec<Step>, key: &RecoverySecret| {
        let to_carol = grant(
            key,
            bob_id,
            note,
            &note_placed,
            carol_id,
            carol.identity(),
            Level::View,
            Authority::Inherited(vec![Link {
                block: folder,
                head: erin_edits.hash(),
            }]),
        );
        let log =
            move |_: LogKind, block: [u8; 16]| (block == folder).then(|| folder_steps.clone());
        let logs = Logs::new([0; 16], None, &log);
        let outside = |asked: &Asked| answer(&logs, note, asked);
        check_log(
            LogKind::Block,
            note,
            note_root.hash(),
            &[note_root.clone(), note_placed.clone(), to_carol],
            &outside,
        )
    };

    // The honest path keeps the retirement whole: the new key grants, the old one does not.
    let honest = proof_path(LogKind::Block, &folder_log, carol_id, [bob_id], true);
    assert!(
        honest.iter().any(|step| matches!(step, Step::Reduced(_))),
        "the path reduces Erin's grant"
    );
    assert!(
        check_note(honest.clone(), &bob_new).is_ok(),
        "Bob's new key grants"
    );
    assert!(
        check_note(honest, &bob).is_err(),
        "Bob's retired key grants nothing after its retirement"
    );

    // A server reduces the retirement, so the path alone names Bob's old key: refused.
    let forged: Vec<Step> = proof_path(LogKind::Block, &folder_log, carol_id, [bob_id], true)
        .into_iter()
        .map(|step| match step {
            Step::Entry(entry) if matches!(entry.record, Record::Retirement(_)) => {
                Step::Reduced(entry.reduced())
            }
            step => step,
        })
        .collect();
    let partial = check_log(LogKind::Block, folder, folder_root.hash(), &forged, &|_| {
        None
    })
    .unwrap();
    assert_eq!(
        partial.signers.get(&bob_id),
        Some(&bob.identity().signing),
        "the path alone names Bob's old key"
    );
    assert!(partial.hides_retirement);
    assert!(
        check_note(forged, &bob).is_err(),
        "a retired key's grant is refused on a path that reduces the retirement"
    );
}
