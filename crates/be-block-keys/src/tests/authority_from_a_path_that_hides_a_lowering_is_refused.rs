use super::*;

/// A record signed on authority from a block above is checked against that block's log as the
/// reader is served it, a proof path for a reader who may not view it. A path that reduces an
/// entry that lowered anyone may be hiding the signer's own revocation, so `answer` takes no
/// authority from it: a server that reduces a revoked manager's revocation gets their grant
/// refused. An honest path of a log that records below rest on keeps every lowering whole
/// (`proof_path`'s `lowerings`), so a grant from a manager above still checks for its reader,
/// past a revocation of someone else.
#[test]
fn authority_from_a_path_that_hides_a_lowering_is_refused() {
    let alice = RecoveryPhrase::generate().secret();
    let bob = RecoveryPhrase::generate().secret();
    let carol = RecoveryPhrase::generate().secret();
    let dave = RecoveryPhrase::generate().secret();
    let erin = RecoveryPhrase::generate().secret();
    let (alice_id, bob_id, carol_id, dave_id, erin_id) =
        ([1; 16], [2; 16], [3; 16], [4; 16], [5; 16]);
    let (folder, note) = ([10; 16], [12; 16]);
    let grant = |by: &RecoverySecret, signer, log, previous: &Entry, lowers, grantee, identity, level, authority| {
        Entry::sign(
            by,
            signer,
            log,
            Some(previous.hash()),
            lowers,
            Record::Grant(Grant {
                grantee,
                identity,
                level,
                authority,
            }),
        )
    };

    // Alice's folder: Bob manages it, Dave was granted View and revoked, and Erin edits it.
    let folder_root = Entry::root(&alice, alice_id, LogKind::Block, folder);
    let bob_manages = grant(&alice, alice_id, folder, &folder_root, false, bob_id, bob.identity(), Level::Manage, Authority::Log);
    let dave_views = grant(&alice, alice_id, folder, &bob_manages, false, dave_id, dave.identity(), Level::View, Authority::Log);
    let dave_revoked = grant(&alice, alice_id, folder, &dave_views, true, dave_id, dave.identity(), Level::None, Authority::Log);
    let erin_edits = grant(&alice, alice_id, folder, &dave_revoked, false, erin_id, erin.identity(), Level::Edit, Authority::Log);

    // Alice's note in the folder, which Bob grants Carol as a manager of the folder.
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
    let check_note = |folder_steps: Vec<Step>, head: &Entry| {
        let to_carol = grant(
            &bob,
            bob_id,
            note,
            &note_placed,
            false,
            carol_id,
            carol.identity(),
            Level::View,
            Authority::Inherited(vec![Link {
                block: folder,
                head: head.hash(),
            }]),
        );
        let log = move |_: LogKind, block: [u8; 16]| (block == folder).then(|| folder_steps.clone());
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

    // Carol, who may not view the folder, is served it as a path proving Bob. One that keeps
    // the lowering whole, as the server serves a log records below rest on, proves Bob's grant.
    let folder_log = vec![folder_root.clone(), bob_manages.clone(), dave_views.clone(), dave_revoked.clone(), erin_edits.clone()];
    let honest = proof_path(LogKind::Block, &folder_log, carol_id, [bob_id], true);
    assert!(
        honest.iter().any(|step| matches!(step, Step::Reduced(_))),
        "the path reduces Erin's grant"
    );
    let checked = check_note(honest, &erin_edits)
        .expect("a grant from a manager above checks on an honest path past a revocation");
    assert_eq!(
        checked.grants.get(&carol_id).map(|(level, _)| *level),
        Some(Level::View)
    );

    // A path that reduces the revocation hides a lowering, which could be Bob's own: refused.
    let hiding = proof_path(LogKind::Block, &folder_log, carol_id, [bob_id], false);
    assert!(
        check_note(hiding, &erin_edits).is_err(),
        "no authority is taken from a path that hides a lowering"
    );

    // Alice revokes Bob, and a server reduces the revocation on the path it serves Carol, so Bob
    // would still seem to manage the folder: his grant is refused.
    let bob_revoked = grant(&alice, alice_id, folder, &erin_edits, true, bob_id, bob.identity(), Level::None, Authority::Log);
    let mut forged = proof_path(LogKind::Block, &folder_log, carol_id, [bob_id], true);
    forged.push(Step::Reduced(bob_revoked.reduced()));
    let partial = check_log(LogKind::Block, folder, folder_root.hash(), &forged, &|_| None).unwrap();
    assert_eq!(
        partial.grants.get(&bob_id).map(|(level, _)| *level),
        Some(Level::Manage),
        "the path alone shows Bob managing still"
    );
    assert!(
        check_note(forged, &bob_revoked).is_err(),
        "a revoked manager's grant is refused on a path that reduces the revocation"
    );
}
