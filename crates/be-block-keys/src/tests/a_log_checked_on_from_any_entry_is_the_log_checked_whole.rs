use super::*;

/// However much of a log a device checked before, checking on from there finds what a check from
/// the root finds: through retirements, an opening closed, a share link opened, used and revoked,
/// the retirement of an account granted through it, a Join link opened, the block changing
/// owner, and bans: one its account's own leaving does not lift, one a grant after lifts, and one
/// that stands.
#[test]
fn a_log_checked_on_from_any_entry_is_the_log_checked_whole() {
    let creator = RecoveryPhrase::generate().secret();
    let renewed = RecoveryPhrase::generate().secret();
    let member = RecoveryPhrase::generate().secret();
    let opener = RecoveryPhrase::generate().secret();
    let opener_renewed = RecoveryPhrase::generate().secret();
    let other = RecoveryPhrase::generate().secret();
    let joiner = RecoveryPhrase::generate().secret();
    let (creator_id, member_id, opener_id, other_id, joiner_id) =
        ([1; 16], [2; 16], [3; 16], [4; 16], [5; 16]);
    let block = [7; 16];
    let none = |_: &Asked| None;
    let root = Entry::root(&creator, creator_id, LogKind::Block, block);
    let pinned = root.hash();
    let check = |entries: &[Entry]| check_log(LogKind::Block, block, pinned, entries, &none);
    let mut entries = vec![root];
    // Appends `record` signed by `by` as `signer`, saying whether it lowers anyone as the log so
    // far finds it, and checks the log still checks.
    let append = |entries: &mut Vec<Entry>, by: &RecoverySecret, signer, record: Record| {
        let checked = check(entries).unwrap();
        let lowers = checked.lowers(signer, &record);
        let previous = checked.head;
        entries.push(Entry::sign(
            by,
            signer,
            block,
            Some(previous),
            lowers,
            record,
        ));
        check(entries).unwrap();
    };
    let push = |entries: &mut Vec<Entry>, entry: Entry| {
        entries.push(entry);
        check(entries).unwrap();
    };
    let link = |code: &InviteCode, level, open| {
        Record::ShareLink(ShareLink {
            key: code.share_key(block),
            level,
            admits: Admits::Members,
            open,
            authority: Authority::Log,
        })
    };
    let opening = |level| {
        Record::Opening(Opening {
            level,
            authority: Authority::Log,
        })
    };

    append(
        &mut entries,
        &creator,
        creator_id,
        Record::Grant(Grant {
            grantee: member_id,
            identity: member.identity(),
            level: Level::View,
            authority: Authority::Log,
        }),
    );
    append(&mut entries, &creator, creator_id, opening(Level::View));
    append(&mut entries, &creator, creator_id, opening(Level::None));
    let head = entries.last().unwrap().hash();
    push(
        &mut entries,
        Entry::retirement(&creator, &renewed, creator_id, block, head),
    );
    let code = InviteCode::generate();
    append(
        &mut entries,
        &renewed,
        creator_id,
        link(&code, Level::Edit, true),
    );
    let head = entries.last().unwrap().hash();
    push(
        &mut entries,
        Entry::opened(&code, block, head, opener_id, &opener, Level::Edit),
    );
    let head = entries.last().unwrap().hash();
    push(
        &mut entries,
        Entry::retirement(&opener, &opener_renewed, opener_id, block, head),
    );
    append(
        &mut entries,
        &renewed,
        creator_id,
        link(&code, Level::Edit, false),
    );
    let join = InviteCode::generate();
    append(
        &mut entries,
        &renewed,
        creator_id,
        link(&join, Level::Join, true),
    );
    let head = entries.last().unwrap().hash();
    push(
        &mut entries,
        Entry::opened(&join, block, head, joiner_id, &joiner, Level::Join),
    );
    append(
        &mut entries,
        &renewed,
        creator_id,
        Record::Owner(Owner {
            account: member_id,
            identity: member.identity(),
        }),
    );
    // The member accepts the block offered to it.
    append(
        &mut entries,
        &member,
        member_id,
        Record::Owner(Owner {
            account: member_id,
            identity: member.identity(),
        }),
    );
    append(
        &mut entries,
        &member,
        member_id,
        Record::Grant(Grant {
            grantee: other_id,
            identity: other.identity(),
            level: Level::View,
            authority: Authority::Log,
        }),
    );

    let ban = |account| {
        Record::Ban(Ban {
            account,
            authority: Authority::Log,
        })
    };
    append(&mut entries, &member, member_id, ban(other_id));
    append(
        &mut entries,
        &other,
        other_id,
        Record::Grant(Grant {
            grantee: other_id,
            identity: other.identity(),
            level: Level::None,
            authority: Authority::Log,
        }),
    );
    assert!(check(&entries).unwrap().banned.contains(&other_id));
    append(
        &mut entries,
        &member,
        member_id,
        Record::Grant(Grant {
            grantee: other_id,
            identity: other.identity(),
            level: Level::Join,
            authority: Authority::Log,
        }),
    );
    append(&mut entries, &member, member_id, ban(opener_id));

    let whole = check(&entries).unwrap();
    assert!(whole.last_lowering > 0 && whole.owner == Some(member_id));
    assert!(
        whole
            .holding(LogKind::Block, joiner_id, Level::Join)
            .is_some(),
        "the Join link's opener holds Join"
    );
    assert_eq!(
        whole.banned,
        std::collections::HashSet::from([opener_id]),
        "a grant after a ban lifted it, and the later ban stands"
    );
    for split in 1..=entries.len() {
        let kept = check(&entries[..split]).unwrap();
        assert_eq!(
            kept.resume(LogKind::Block, block, &entries, &none).unwrap(),
            whole,
            "checked on from entry {split}"
        );
    }
}
