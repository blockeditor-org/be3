use super::*;

#[test]
fn a_member_is_signed_in_by_an_administrator_and_an_agent_by_its_maker() {
    let admin = RecoveryPhrase::generate().secret();
    let person = RecoveryPhrase::generate().secret();
    let agent = RecoveryPhrase::generate().secret();
    let stranger = RecoveryPhrase::generate().secret();
    let (admin_id, person_id, stranger_id) = ([1; 16], [2; 16], [4; 16]);
    let agent_id = agent_account(person_id, &agent.identity());
    let organization = [9; 16];
    let none = |_: [u8; 16], _: &Authority| None;
    let root = Entry::root(&admin, admin_id, LogKind::Organization, organization);
    let pinned = root.hash();
    let member = |by: &RecoverySecret, signer, previous, account, identity, member, agent| {
        Entry::sign(
            by,
            signer,
            organization,
            Some(previous),
            Record::Member(Member {
                account,
                identity,
                member,
                agent,
            }),
        )
    };
    let check =
        |entries: &[Entry]| check_log(LogKind::Organization, organization, pinned, entries, &none);

    let joined = member(
        &admin,
        admin_id,
        pinned,
        person_id,
        person.identity(),
        true,
        false,
    );
    let checked = check(&[root.clone(), joined.clone()]).unwrap();
    assert_eq!(checked.member(person_id), Some(person.identity()));
    assert_eq!(
        checked.member(admin_id),
        Some(admin.identity()),
        "an administrator is a member"
    );

    let invited = member(
        &person,
        person_id,
        joined.hash(),
        stranger_id,
        stranger.identity(),
        true,
        false,
    );
    assert!(
        check(&[root.clone(), joined.clone(), invited]).is_err(),
        "a member who does not administer signs no person in"
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
    let checked = check(&[root.clone(), joined.clone(), made.clone()]).unwrap();
    assert_eq!(
        checked.members.get(&agent_id).and_then(|named| named.maker),
        Some(person_id)
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
    assert!(
        check(&[root.clone(), joined.clone(), made.clone(), onward]).is_err(),
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
        check(&[root.clone(), joined.clone(), made.clone(), gone])
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
        check(&[root.clone(), joined.clone(), made.clone(), ousted]).is_err(),
        "a member signs out no one but their agents"
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
    assert!(
        check(&[root.clone(), joined.clone(), claimed]).is_err(),
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
        check(&[root.clone(), joined.clone(), stolen]).is_err(),
        "nor one made for other keys than those it names"
    );
    let swapped = member(
        &admin,
        admin_id,
        joined.hash(),
        person_id,
        stranger.identity(),
        true,
        false,
    );
    assert!(
        check(&[root.clone(), joined.clone(), swapped]).is_err(),
        "a member already signed in is not signed in again with other keys"
    );
    let forged = member(
        &stranger,
        stranger_id,
        pinned,
        stranger_id,
        stranger.identity(),
        true,
        false,
    );
    assert!(
        check(&[root, forged]).is_err(),
        "no one signs themselves in"
    );
}
