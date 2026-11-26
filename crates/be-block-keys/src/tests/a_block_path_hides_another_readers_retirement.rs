use super::*;

/// A retirement's `administrator` bit means nothing in a block's log, so a block's proof path
/// keeps a retirement whole only when it names an account the path proves: a reader below Edit
/// learns nothing of another reader who took a new phrase, though their device says true.
#[test]
fn a_block_path_hides_another_readers_retirement() {
    let alice = RecoveryPhrase::generate().secret();
    let bob = RecoveryPhrase::generate().secret();
    let bob_new = RecoveryPhrase::generate().secret();
    let carol = RecoveryPhrase::generate().secret();
    let (alice_id, bob_id, carol_id) = ([1; 16], [2; 16], [3; 16]);
    let note = [12; 16];
    let grant = |previous: &Entry, grantee, identity| {
        Entry::sign(
            &alice,
            alice_id,
            note,
            Some(previous.hash()),
            false,
            Record::Grant(Grant {
                grantee,
                identity,
                level: Level::View,
                authority: Authority::Log,
            }),
        )
    };
    let root = Entry::root(&alice, alice_id, LogKind::Block, note);
    let bob_views = grant(&root, bob_id, bob.identity());
    let carol_views = grant(&bob_views, carol_id, carol.identity());
    let bob_retires = Entry::retirement(&bob, &bob_new, bob_id, note, carol_views.hash(), true);
    let log = vec![root.clone(), bob_views, carol_views, bob_retires];
    let none = |_: &Asked| None;
    check_log(LogKind::Block, note, root.hash(), &log, &none).unwrap();

    let path = proof_path(LogKind::Block, &log, carol_id, [], false);
    let whole: Vec<bool> = path.iter().map(|step| step.entry().is_some()).collect();
    assert_eq!(whole, [true, false, true, false]);
    let checked = check_log(LogKind::Block, note, root.hash(), &path, &none).unwrap();
    assert!(checked.hides_retirement);
    assert_eq!(
        checked.level_as_of(carol_id, &checked.head),
        Some(Level::View)
    );
}
