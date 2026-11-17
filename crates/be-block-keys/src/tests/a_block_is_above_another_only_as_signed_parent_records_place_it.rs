use super::*;

/// A parent record is made by a member with Edit or above, and `Authority::Inherited` counts
/// only along the chain of blocks its signed parent records place the block under, each as of
/// the head the record names, each log checked from the root the placement under it names: so
/// the server can neither put a block under an ancestor someone else manages, nor serve a
/// made-up log for a real one, nor count a block above that it lay under only at another time
/// (plans/signed_access.md, step 4).
#[test]
fn a_block_is_above_another_only_as_signed_parent_records_place_it() {
    let alice = RecoveryPhrase::generate().secret();
    let bob = RecoveryPhrase::generate().secret();
    let carol = RecoveryPhrase::generate().secret();
    let (alice_id, bob_id, carol_id) = ([1; 16], [2; 16], [3; 16]);
    let (folder, middle, note, elsewhere) = ([10; 16], [11; 16], [12; 16], [13; 16]);
    let none = |_: &Asked| None;

    // Alice's folder, which she grants Bob Manage on.
    let folder_root = Entry::root(&alice, alice_id, LogKind::Block, folder);
    let bob_manages = Entry::sign(
        &alice,
        alice_id,
        folder,
        Some(folder_root.hash()),
        false,
        Record::Grant(Grant {
            grantee: bob_id,
            identity: bob.identity(),
            level: Level::Manage,
            authority: Authority::Log,
        }),
    );
    let folder_log = vec![folder_root.clone(), bob_manages.clone()];
    let place = |by: &RecoverySecret, signer, log, previous, placement, authority| {
        Entry::sign(
            by,
            signer,
            log,
            Some(previous),
            false,
            Record::Parent(Parent {
                placement,
                authority,
            }),
        )
    };
    let under = |block: [u8; 16], root: &Entry| Placement::Block {
        block,
        root: root.hash(),
    };

    // A folder in the middle, and Alice's note inside it.
    let middle_root = Entry::root(&alice, alice_id, LogKind::Block, middle);
    let middle_placed = place(
        &alice,
        alice_id,
        middle,
        middle_root.hash(),
        under(folder, &folder_root),
        Authority::Log,
    );
    let middle_log = vec![middle_root.clone(), middle_placed.clone()];
    let note_root = Entry::root(&alice, alice_id, LogKind::Block, note);
    let note_placed = place(
        &alice,
        alice_id,
        note,
        note_root.hash(),
        under(middle, &middle_root),
        Authority::Log,
    );
    // Bob grants Carol the note, as a manager of the folder two levels up.
    let from_folder = Authority::Inherited(vec![
        Link {
            block: middle,
            head: middle_placed.hash(),
        },
        Link {
            block: folder,
            head: bob_manages.hash(),
        },
    ]);
    let to_carol = Entry::sign(
        &bob,
        bob_id,
        note,
        Some(note_placed.hash()),
        false,
        Record::Grant(Grant {
            grantee: carol_id,
            identity: carol.identity(),
            level: Level::View,
            authority: from_folder.clone(),
        }),
    );
    let logs_of = |logs: Vec<([u8; 16], Vec<Entry>)>| {
        move |_: LogKind, block: [u8; 16]| {
            logs.iter()
                .find(|(id, _)| *id == block)
                .map(|(_, entries)| entries.iter().cloned().map(Step::from).collect())
        }
    };
    let check_note = |log: &dyn Fn(LogKind, [u8; 16]) -> Option<Vec<Step>>, entries: &[Entry]| {
        let logs = Logs::new([0; 16], None, log);
        let outside = |asked: &Asked| answer(&logs, note, asked);
        check_log(LogKind::Block, note, note_root.hash(), entries, &outside)
    };
    let honest = logs_of(vec![
        (folder, folder_log.clone()),
        (middle, middle_log.clone()),
    ]);
    let checked = check_note(
        &honest,
        &[note_root.clone(), note_placed.clone(), to_carol.clone()],
    )
    .expect("a manager of a block its parent records place it under grants it");
    assert_eq!(
        checked.grants.get(&carol_id).map(|(level, _)| *level),
        Some(Level::View)
    );
    assert_eq!(checked.placement(), Some(under(middle, &middle_root)));

    // The same folder, served as a log rooted at a creator the server controls: the placement
    // names the real root, so it does not check.
    let forged_root = Entry::root(&bob, bob_id, LogKind::Block, folder);
    let forged = logs_of(vec![
        (folder, vec![forged_root]),
        (middle, middle_log.clone()),
    ]);
    assert!(
        check_note(
            &forged,
            &[note_root.clone(), note_placed.clone(), to_carol.clone()]
        )
        .is_err(),
        "a made-up log for a real ancestor proves no one's Manage"
    );

    // Placed elsewhere, the folder is not above it, whatever the server's tree says.
    let elsewhere_root = Entry::root(&alice, alice_id, LogKind::Block, elsewhere);
    let placed_elsewhere = place(
        &alice,
        alice_id,
        note,
        note_root.hash(),
        under(elsewhere, &elsewhere_root),
        Authority::Log,
    );
    let granted_elsewhere = Entry::sign(
        &bob,
        bob_id,
        note,
        Some(placed_elsewhere.hash()),
        false,
        Record::Grant(Grant {
            grantee: carol_id,
            identity: carol.identity(),
            level: Level::View,
            authority: from_folder.clone(),
        }),
    );
    let around = logs_of(vec![
        (folder, folder_log.clone()),
        (middle, middle_log.clone()),
        (elsewhere, vec![elsewhere_root.clone()]),
    ]);
    assert!(
        check_note(
            &around,
            &[note_root.clone(), placed_elsewhere, granted_elsewhere]
        )
        .is_err(),
        "a manager of a block the note is not placed under grants nothing"
    );

    // Moved away later, a grant made while it lay under the folder still checks.
    let moved = place(
        &alice,
        alice_id,
        note,
        to_carol.hash(),
        under(elsewhere, &elsewhere_root),
        Authority::Log,
    );
    assert!(
        check_note(
            &around,
            &[note_root.clone(), note_placed.clone(), to_carol, moved]
        )
        .is_ok(),
        "a grant is checked where the block lay at its place"
    );

    // The middle folder, later moved out of the folder: a chain naming its head from after
    // the move does not reach the folder, though the middle once lay under it.
    let middle_moved = place(
        &alice,
        alice_id,
        middle,
        middle_placed.hash(),
        under(elsewhere, &elsewhere_root),
        Authority::Log,
    );
    let stale_chain = Authority::Inherited(vec![
        Link {
            block: middle,
            head: middle_moved.hash(),
        },
        Link {
            block: folder,
            head: bob_manages.hash(),
        },
    ]);
    let granted_after = Entry::sign(
        &bob,
        bob_id,
        note,
        Some(note_placed.hash()),
        false,
        Record::Grant(Grant {
            grantee: carol_id,
            identity: carol.identity(),
            level: Level::View,
            authority: stale_chain,
        }),
    );
    let moved_logs = logs_of(vec![
        (folder, folder_log.clone()),
        (middle, vec![middle_root.clone(), middle_placed.clone(), middle_moved]),
        (elsewhere, vec![elsewhere_root.clone()]),
    ]);
    assert!(
        check_note(
            &moved_logs,
            &[note_root.clone(), note_placed.clone(), granted_after]
        )
        .is_err(),
        "a block lain under once, but not as of the head the chain names, is not above"
    );

    // A parent record needs Edit or above: Carol, who views it, moves nothing; Bob moves it
    // as an editor from above.
    let viewer = Entry::sign(
        &alice,
        alice_id,
        note,
        Some(note_placed.hash()),
        false,
        Record::Grant(Grant {
            grantee: carol_id,
            identity: carol.identity(),
            level: Level::View,
            authority: Authority::Log,
        }),
    );
    let carol_moves = place(
        &carol,
        carol_id,
        note,
        viewer.hash(),
        Placement::Detached,
        Authority::Log,
    );
    assert!(
        check_note(
            &honest,
            &[note_root.clone(), note_placed.clone(), viewer, carol_moves]
        )
        .is_err(),
        "one who only views a block moves it nowhere"
    );
    let bob_moves = place(
        &bob,
        bob_id,
        note,
        note_placed.hash(),
        Placement::Detached,
        from_folder.clone(),
    );
    assert_eq!(
        check_note(
            &honest,
            &[note_root.clone(), note_placed.clone(), bob_moves]
        )
        .unwrap()
        .placement(),
        Some(Placement::Detached),
        "an editor from above moves it"
    );

    // No block lies under itself, and an organization's log places nothing.
    let under_itself = place(
        &alice,
        alice_id,
        note,
        note_root.hash(),
        under(note, &note_root),
        Authority::Log,
    );
    assert!(
        check_log(
            LogKind::Block,
            note,
            note_root.hash(),
            &[note_root.clone(), under_itself],
            &none
        )
        .is_err()
    );
    let organization = [20; 16];
    let organization_root = Entry::root(&alice, alice_id, LogKind::Organization, organization);
    let placed = place(
        &alice,
        alice_id,
        organization,
        organization_root.hash(),
        Placement::Top,
        Authority::Log,
    );
    assert!(
        check_log(
            LogKind::Organization,
            organization,
            organization_root.hash(),
            &[organization_root.clone(), placed],
            &none
        )
        .is_err()
    );
}
