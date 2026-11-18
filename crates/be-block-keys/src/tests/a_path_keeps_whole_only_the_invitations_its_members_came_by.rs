use super::*;

/// An organization that made many invitations, opened and ended, serves a member below Edit a
/// path whose invitation records are those of the invitation that member came by: the rest are
/// reduced, so the path's whole entries do not grow with every invitation made, and it checks.
#[test]
fn a_path_keeps_whole_only_the_invitations_its_members_came_by() {
    let admin = RecoveryPhrase::generate().secret();
    let reader = RecoveryPhrase::generate().secret();
    let other = RecoveryPhrase::generate().secret();
    let (admin_id, reader_id, other_id) = ([1; 16], [2; 16], [3; 16]);
    let organization = [9; 16];
    let none = |_: &Asked| None;
    let root = Entry::root(&admin, admin_id, LogKind::Organization, organization);
    let pinned = root.hash();
    let mut log = vec![root];
    let invitation = |log: &mut Vec<Entry>, code: &InviteCode, open| {
        let previous = log.last().unwrap().hash();
        log.push(Entry::sign(
            &admin,
            admin_id,
            organization,
            Some(previous),
            false,
            Record::Invitation(Invitation {
                key: code.invitation_key(organization),
                open,
            }),
        ));
    };
    let whole_invitations = |log: &[Entry]| {
        let path = proof_path(LogKind::Organization, log, reader_id, []);
        check_log(LogKind::Organization, organization, pinned, &path, &none)
            .expect("the path checks");
        path.iter()
            .filter_map(Step::entry)
            .filter(|entry| matches!(entry.record, Record::Invitation(_)))
            .count()
    };

    let mine = InviteCode::generate();
    invitation(&mut log, &mine, true);
    let previous = log.last().unwrap().hash();
    log.push(Entry::accepted(
        &mine,
        organization,
        previous,
        reader_id,
        &reader,
    ));
    let theirs = InviteCode::generate();
    invitation(&mut log, &theirs, true);
    let previous = log.last().unwrap().hash();
    log.push(Entry::accepted(
        &theirs,
        organization,
        previous,
        other_id,
        &other,
    ));
    let few = whole_invitations(&log);
    assert_eq!(
        few, 1,
        "the reader's own invitation is whole, another's reduced"
    );

    for at in 0..50 {
        let code = InviteCode::generate();
        invitation(&mut log, &code, true);
        if at % 2 == 0 {
            invitation(&mut log, &code, false);
        }
    }
    assert_eq!(
        whole_invitations(&log),
        few,
        "invitations no member on the path came by are reduced, open or ended"
    );
    let whole = check_log(LogKind::Organization, organization, pinned, &log, &none).unwrap();
    assert!(whole.member(reader_id).is_some() && whole.member(other_id).is_some());
}
