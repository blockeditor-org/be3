use super::*;

#[test]
fn a_member_is_signed_in_by_an_invitation_and_an_agent_by_its_maker() {
    let admin = RecoveryPhrase::generate().secret();
    let person = RecoveryPhrase::generate().secret();
    let agent = RecoveryPhrase::generate().secret();
    let stranger = RecoveryPhrase::generate().secret();
    let (admin_id, person_id, stranger_id) = ([1; 16], [2; 16], [4; 16]);
    let agent_id = agent_account(person_id, &agent.identity());
    let organization = [9; 16];
    let none = |_: &Asked| None;
    let root = Entry::root(&admin, admin_id, LogKind::Organization, organization);
    let pinned = root.hash();
    let member = |by: &RecoverySecret,
                  signer,
                  previous,
                  account,
                  identity: PublicIdentity,
                  member: bool,
                  agent: bool| {
        Entry::sign(
            by,
            signer,
            organization,
            Some(previous),
            false,
            Record::Member(Member {
                account,
                identity: member.then_some(identity),
                member,
                agent,
                invitation: None,
            }),
        )
    };
    let invitation = |by: &RecoverySecret, signer, previous, code: &InviteCode, open| {
        Entry::sign(
            by,
            signer,
            organization,
            Some(previous),
            false,
            Record::Invitation(Invitation {
                key: code.invitation_key(organization),
                open,
            }),
        )
    };
    let check =
        |entries: &[Entry]| check_log(LogKind::Organization, organization, pinned, entries, &none);

    let code = InviteCode::generate();
    let invited = invitation(&admin, admin_id, pinned, &code, true);
    let joined = Entry::accepted(&code, organization, invited.hash(), person_id, &person);
    let checked = check(&[root.clone(), invited.clone(), joined.clone()]).unwrap();
    assert_eq!(checked.member(person_id), Some(person.identity()));
    assert_eq!(
        checked.member(admin_id),
        Some(admin.identity()),
        "an administrator is a member"
    );
    let again = Entry::accepted(&code, organization, joined.hash(), stranger_id, &stranger);
    assert!(
        check(&[root.clone(), invited.clone(), joined.clone(), again]).is_err(),
        "an invitation used once signs no one else in"
    );
    let ended = invitation(&admin, admin_id, invited.hash(), &code, false);
    let late = Entry::accepted(&code, organization, ended.hash(), person_id, &person);
    assert!(
        check(&[root.clone(), invited.clone(), ended, late]).is_err(),
        "an invitation ended signs no one in"
    );
    let other = InviteCode::generate();
    let uninvited = Entry::accepted(&other, organization, pinned, person_id, &person);
    assert!(
        check(&[root.clone(), uninvited]).is_err(),
        "a code no administrator opened as an invitation signs no one in"
    );
    let by_admin = member(
        &admin,
        admin_id,
        pinned,
        person_id,
        person.identity(),
        true,
        false,
    );
    assert!(
        check(&[root.clone(), by_admin]).is_err(),
        "a person is signed in by an invitation alone, not by an administrator"
    );

    let made = member(
        &person,
        person_id,
        joined.hash(),
        agent_id,
        agent.identity(),
        true,
        true,
    );
    let checked = check(&[root.clone(), invited.clone(), joined.clone(), made.clone()]).unwrap();
    assert_eq!(
        checked.members.get(&agent_id).and_then(|named| named.maker),
        Some(person_id)
    );
    let opened_by_member = invitation(&person, person_id, joined.hash(), &other, true);
    assert!(
        check(&[
            root.clone(),
            invited.clone(),
            joined.clone(),
            opened_by_member
        ])
        .is_err(),
        "a member who does not administer invites no one"
    );

    let onward = member(
        &agent,
        agent_id,
        made.hash(),
        agent_account(agent_id, &stranger.identity()),
        stranger.identity(),
        true,
        true,
    );
    let before = [root.clone(), invited.clone(), joined.clone(), made.clone()];
    assert!(
        check(&[before.as_slice(), &[onward]].concat()).is_err(),
        "an agent makes no agent"
    );
    let gone = member(
        &person,
        person_id,
        made.hash(),
        agent_id,
        agent.identity(),
        false,
        true,
    );
    assert!(
        check(&[before.as_slice(), &[gone]].concat())
            .unwrap()
            .member(agent_id)
            .is_none(),
        "an agent's maker signs it out"
    );
    let ousted = member(
        &person,
        person_id,
        made.hash(),
        admin_id,
        admin.identity(),
        false,
        false,
    );
    assert!(
        check(&[before.as_slice(), &[ousted]].concat()).is_err(),
        "a member signs out no one but their agents"
    );
    let removed = member(
        &admin,
        admin_id,
        made.hash(),
        person_id,
        person.identity(),
        false,
        false,
    );
    assert!(
        check(&[before.as_slice(), &[removed]].concat())
            .unwrap()
            .member(person_id)
            .is_none(),
        "an administrator signs a person out"
    );
    let claimed = member(
        &person,
        person_id,
        joined.hash(),
        stranger_id,
        person.identity(),
        true,
        true,
    );
    let joined_log = [root.clone(), invited.clone(), joined.clone()];
    assert!(
        check(&[joined_log.as_slice(), &[claimed]].concat()).is_err(),
        "a member claims no account as its agent but one made for the agent's keys by it"
    );
    let stolen = member(
        &person,
        person_id,
        joined.hash(),
        agent_account(person_id, &stranger.identity()),
        person.identity(),
        true,
        true,
    );
    assert!(
        check(&[joined_log.as_slice(), &[stolen]].concat()).is_err(),
        "nor one made for other keys than those it names"
    );
    let code = InviteCode::generate();
    let second = invitation(&admin, admin_id, joined.hash(), &code, true);
    let swapped = Entry::accepted(&code, organization, second.hash(), person_id, &stranger);
    assert!(
        check(&[joined_log.as_slice(), &[second.clone(), swapped]].concat()).is_err(),
        "a member already signed in is not signed in again with other keys"
    );

    // An invitation lasts only while its inviter administers the organization.
    let deputy_id = [5; 16];
    let deputy = RecoveryPhrase::generate().secret();
    let promoted = Entry::sign(
        &admin,
        admin_id,
        organization,
        Some(pinned),
        false,
        Record::Administrator(Administrator {
            account: deputy_id,
            identity: deputy.identity(),
            administrator: true,
        }),
    );
    let by_deputy = invitation(&deputy, deputy_id, promoted.hash(), &code, true);
    let demoted = Entry::sign(
        &admin,
        admin_id,
        organization,
        Some(by_deputy.hash()),
        true,
        Record::Administrator(Administrator {
            account: deputy_id,
            identity: deputy.identity(),
            administrator: false,
        }),
    );
    let after = [root.clone(), promoted, by_deputy];
    let accepting = |previous| Entry::accepted(&code, organization, previous, person_id, &person);
    let in_time = accepting(after[2].hash());
    assert!(check(&[after.as_slice(), &[in_time]].concat()).is_ok());
    let too_late = accepting(demoted.hash());
    assert!(
        check(&[after.as_slice(), &[demoted.clone(), too_late]].concat()).is_err(),
        "an invitation whose inviter no longer administers signs no one in"
    );
    // Nor once they administer again: it ended with their role.
    let again = Entry::sign(
        &admin,
        admin_id,
        organization,
        Some(demoted.hash()),
        false,
        Record::Administrator(Administrator {
            account: deputy_id,
            identity: deputy.identity(),
            administrator: true,
        }),
    );
    let revived = accepting(again.hash());
    assert!(
        check(&[after.as_slice(), &[demoted, again, revived]].concat()).is_err(),
        "an invitation came back with its inviter's role"
    );

    // The accepter's own keys endorse the record: a holder of the code names no keys it does
    // not hold.
    let code = InviteCode::generate();
    let opened = invitation(&admin, admin_id, pinned, &code, true);
    let endorsed = Entry::accepted(&code, organization, opened.hash(), person_id, &stranger);
    let Record::Member(mut member) = endorsed.record else {
        unreachable!()
    };
    member.identity = Some(person.identity());
    let claimed = Entry::invitation_signed(
        &code.invitation_signing(organization),
        organization,
        opened.hash(),
        person_id,
        Record::Member(member),
    );
    assert!(
        check(&[root.clone(), opened, claimed]).is_err(),
        "a record named keys its accepter does not hold"
    );
}
