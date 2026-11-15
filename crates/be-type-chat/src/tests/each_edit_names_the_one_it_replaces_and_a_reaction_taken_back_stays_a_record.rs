use be_block_model::LatestMap;

use super::*;
use crate::{ChatHeader, Deletion, EditSlot, MessageEdit, Reaction, ReactionKey};

/// What someone with Edit writes at `slot` of the edits at `rank`: a record or a removal.
fn edit_at(chat: &ChatContent, slot: &EditSlot, rank: u64, edit: Option<&MessageEdit>) -> ChatContent {
    let edits: LatestMap<EditSlot, MessageEdit> =
        chat.header().field(ObjectId::ROOT, ChatHeader::EDITS);
    let change = ChatHeader::EDITS.put(ObjectId::ROOT, slot, edit, edits.next(slot, 99, BO).ranked(rank));
    edited(chat, [[change].into_iter().collect()])
}

fn reaction_at(
    chat: &ChatContent,
    key: &ReactionKey,
    rank: u64,
    reaction: Option<&Reaction>,
) -> ChatContent {
    let reactions: LatestMap<ReactionKey, Reaction> =
        chat.header().field(ObjectId::ROOT, ChatHeader::REACTIONS);
    let stamp = reactions.next(key, 99, BO).ranked(rank);
    let change = ChatHeader::REACTIONS.put(ObjectId::ROOT, key, reaction, stamp);
    edited(chat, [[change].into_iter().collect()])
}

/// Each edit and reaction record counts one past its slot's rank and names the record it
/// replaces, and is kept ranked by its count: one put back never takes a later one's place,
/// a later one from another device always does, and a slot holding anything but a record at
/// its own count shows its history was altered.
#[test]
fn each_edit_names_the_one_it_replaces_and_a_reaction_taken_back_stays_a_record() {
    let chat = sent(
        &ChatContent::default(),
        ADA,
        &["lunch at noon?", "who is in?"],
    );
    let (lunch, who) = (first(&chat), chat.messages()[1]);
    let slot = EditSlot {
        message: lunch,
        author: ADA,
    };

    let once = chat.next_edit(lunch, ADA, 5, "lunch at one?");
    assert_eq!((once.seq, once.previous), (1, None), "a message's first edit");
    let chat = edited(&chat, [chat.put_edit(lunch, &once)]);
    let twice = chat.next_edit(lunch, ADA, 6, "lunch at two?");
    assert_eq!(
        (twice.seq, twice.previous),
        (2, Some(once.key(lunch))),
        "a later edit counts on and names the one it replaces"
    );
    let chat = edited(&chat, [chat.put_edit(lunch, &twice)]);
    assert_eq!(chat.body(lunch).as_deref(), Some("lunch at two?"));
    assert!(!chat.edit_head(lunch, ADA).cut);

    // Someone with Edit puts the first edit back: it counts lower, so it changes nothing.
    let replayed = edited(&chat, [chat.put_edit(lunch, &once)]);
    assert_eq!(replayed.body(lunch).as_deref(), Some("lunch at two?"));
    assert!(!replayed.edit_head(lunch, ADA).cut);
    // Put above the latest it signs a count it is not kept at, which shows.
    let raised = edit_at(&chat, &slot, 3, Some(&once));
    assert_eq!(raised.body(lunch).as_deref(), Some("lunch at one?"));
    assert!(raised.edit_head(lunch, ADA).cut);
    // So does a removal, which ranks above the latest to take its place; the author's next edit
    // outranks it and heals the slot.
    let removed = edit_at(&chat, &slot, 3, None);
    assert_eq!(removed.edit_of(lunch), None);
    assert!(removed.edit_head(lunch, ADA).cut);
    let again = removed.next_edit(lunch, ADA, 12, "lunch at three?");
    assert_eq!((again.seq, again.previous), (4, None));
    let healed = edited(&removed, [removed.put_edit(lunch, &again)]);
    assert!(!healed.edit_head(lunch, ADA).cut);
    assert_eq!(healed.body(lunch).as_deref(), Some("lunch at three?"));

    // A rank no author reaches shows as altered for good, without overflowing: the author's
    // next edit still lands, at the same rank by a later stamp.
    let junk = edit_at(&chat, &slot, u64::MAX, None);
    let after = junk.next_edit(lunch, ADA, 13, "lunch at four?");
    assert_eq!(after.seq, u64::MAX);
    let after = edited(&junk, [junk.put_edit(lunch, &after)]);
    assert_eq!(after.body(lunch).as_deref(), Some("lunch at four?"));
    assert!(after.edit_head(lunch, ADA).cut, "a slot at the top stays altered");

    // A device offline since the first edit writes its own second: the other device's third
    // outranks it whichever merges first or stamps later, and nothing reads as altered.
    let base = chat.clone();
    let third = base.next_edit(lunch, ADA, 7, "lunch at three, from the laptop");
    let laptop = edited(&base, [base.put_edit(lunch, &third)]);
    let stale = MessageEdit {
        seq: 2,
        previous: Some(once.key(lunch)),
        time: 50,
        body: "lunch at two, from the phone".to_owned(),
        ..twice.clone()
    };
    let phone = edited(&base, [base.put_edit(lunch, &stale)]);
    for (ours, theirs) in [(&laptop, &phone), (&phone, &laptop)] {
        let (joined, _) = merged(&base, ours, theirs);
        assert_eq!(
            joined.body(lunch).as_deref(),
            Some("lunch at three, from the laptop")
        );
        assert!(!joined.edit_head(lunch, ADA).cut);
    }

    // A record filed under another author takes that author, which its signature does not
    // cover.
    let bos = chat.next_edit(lunch, BO, 9, "Bo says three");
    let filed = edit_at(&chat, &slot, 3, Some(&MessageEdit { seq: 3, ..bos.clone() }));
    let head = filed.edit_of(lunch).expect("the filed record is the latest");
    assert_eq!(head.author, ADA);
    assert_ne!(head.key(lunch), MessageEdit { seq: 3, ..bos }.key(lunch));
    // Someone else's own edit is kept in their own slot, and changes nothing.
    let by_bo = edited(&chat, [chat.edit(lunch, BO, 9, "Bo says three")]);
    assert_eq!(by_bo.body(lunch).as_deref(), Some("lunch at two?"));

    // A deletion removed outright is noticed.
    let deleted = edited(&chat, [chat.delete(who, ADA, 10)]);
    assert!(!deleted.deletion_removed(who));
    let deletions: be_block_model::LatestMap<ObjectId, Deletion> = deleted
        .header()
        .field(ObjectId::ROOT, ChatHeader::DELETIONS);
    let removal =
        ChatHeader::DELETIONS.put(ObjectId::ROOT, &who, None, deletions.next(&who, 11, BO));
    let undeleted = edited(&deleted, [[removal].into_iter().collect()]);
    assert!(undeleted.deletion_removed(who));

    // Reactions rank the same way: taking one back stays a record, the kept one put back
    // changes nothing, and removing the taken-back one shows.
    let (key, kept) = chat.reaction(lunch, BO, "heart", 8, true);
    let reacted = edited(&chat, [chat.put_reaction(&key, &kept)]);
    assert_eq!(reacted.reactions(lunch), [("heart".to_owned(), vec![BO])]);
    let (_, taken_back) = reacted.reaction(lunch, BO, "heart", 9, false);
    assert_eq!(
        (taken_back.seq, taken_back.previous),
        (2, Some(kept.key(&key)))
    );
    let unreacted = edited(&reacted, [reacted.put_reaction(&key, &taken_back)]);
    assert!(
        unreacted.reactions(lunch).is_empty(),
        "a reaction taken back is not shown"
    );
    let heads = unreacted.reaction_heads(lunch);
    assert_eq!(heads.len(), 1);
    assert!(
        heads[0].1.record.as_ref().is_some_and(|reaction| !reaction.kept) && !heads[0].1.cut,
        "it stays as a record its author can sign"
    );
    let replayed = edited(&unreacted, [unreacted.put_reaction(&key, &kept)]);
    assert!(replayed.reactions(lunch).is_empty());
    let cut = reaction_at(&unreacted, &key, 3, None);
    assert!(cut.reactions(lunch).is_empty());
    assert!(cut.reaction_heads(lunch)[0].1.cut, "the taken-back record was removed");
}
