use be_block_model::LatestMap;

use super::*;
use crate::{ChatHeader, Counted, Deletion, EditSlot, MessageEdit, Reaction, ReactionKey};

/// What someone with Edit writes at `key` of the edits, a record or a removal.
fn edit_at(chat: &ChatContent, key: &Counted<EditSlot>, edit: Option<&MessageEdit>) -> ChatContent {
    let edits: LatestMap<Counted<EditSlot>, MessageEdit> =
        chat.header().field(ObjectId::ROOT, ChatHeader::EDITS);
    let change = ChatHeader::EDITS.put(ObjectId::ROOT, key, edit, edits.next(key, 99, BO));
    edited(chat, [[change].into_iter().collect()])
}

fn reaction_at(
    chat: &ChatContent,
    key: &Counted<ReactionKey>,
    reaction: Option<&Reaction>,
) -> ChatContent {
    let reactions: LatestMap<Counted<ReactionKey>, Reaction> =
        chat.header().field(ObjectId::ROOT, ChatHeader::REACTIONS);
    let change =
        ChatHeader::REACTIONS.put(ObjectId::ROOT, key, reaction, reactions.next(key, 99, BO));
    edited(chat, [[change].into_iter().collect()])
}

/// Each edit and reaction record counts one past its author's last for its slot and names it,
/// and writing one removes the earlier: a record put back is not the latest, and the latest cut
/// shows, however the records around it are rewritten.
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
    let at = |seq| Counted { slot, seq };

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
    let edits: LatestMap<Counted<EditSlot>, MessageEdit> =
        chat.header().field(ObjectId::ROOT, ChatHeader::EDITS);
    assert_eq!(
        edits.keys().collect::<Vec<_>>(),
        [&at(2)],
        "the earlier edit is removed"
    );

    // Someone with Edit puts the first edit back: the second is still the latest.
    let replayed = edited(&chat, [chat.put_edit(lunch, &once)]);
    assert_eq!(replayed.body(lunch).as_deref(), Some("lunch at two?"));
    assert!(!replayed.edit_head(lunch, ADA).cut);
    // Removing the second as well cuts the latest, which shows.
    let rolled_back = edit_at(&replayed, &at(2), None);
    assert_eq!(rolled_back.body(lunch).as_deref(), Some("lunch at one?"));
    assert!(rolled_back.edit_head(lunch, ADA).cut);
    // So does an edit removed outright.
    let removed = edit_at(&chat, &at(2), None);
    assert_eq!(removed.edit_of(lunch), None);
    assert!(removed.edit_head(lunch, ADA).cut);
    // Its author's next edit counts past the cut and is the latest again.
    let again = removed.next_edit(lunch, ADA, 12, "lunch at three?");
    assert_eq!((again.seq, again.previous), (3, None));
    let healed = edited(&removed, [removed.put_edit(lunch, &again)]);
    assert!(!healed.edit_head(lunch, ADA).cut);
    assert_eq!(healed.body(lunch).as_deref(), Some("lunch at three?"));
    // A record moved to a higher count takes that count, which its signature does not cover.
    let moved = edit_at(&chat, &at(7), Some(&once));
    let head = moved.edit_of(lunch).expect("the moved record is the latest");
    assert_eq!((head.body.as_str(), head.seq), ("lunch at one?", 7));
    assert_ne!(head.key(lunch), once.key(lunch));
    // Someone else's edit is kept in their own slot, and changes nothing.
    let by_bo = edited(&chat, [chat.edit(lunch, BO, 9, "Bo says three")]);
    assert_eq!(by_bo.body(lunch).as_deref(), Some("lunch at two?"));

    // A deletion removed outright is noticed.
    let deleted = edited(&chat, [chat.delete(who, ADA, 10)]);
    assert!(!deleted.deletion_removed(who));
    let deletions: LatestMap<ObjectId, Deletion> = deleted
        .header()
        .field(ObjectId::ROOT, ChatHeader::DELETIONS);
    let removal =
        ChatHeader::DELETIONS.put(ObjectId::ROOT, &who, None, deletions.next(&who, 11, BO));
    let undeleted = edited(&deleted, [[removal].into_iter().collect()]);
    assert!(undeleted.deletion_removed(who));

    // Reactions chain the same way: taking one back stays a record, the kept one put back is
    // not the latest, and removing the taken-back one cuts it.
    let (key, kept) = chat.reaction(lunch, BO, "heart", 8, true);
    let reacted = edited(&chat, [chat.put_reaction(&key, &kept)]);
    assert_eq!(reacted.reactions(lunch), [("heart".to_owned(), vec![BO])]);
    let (_, taken_back) = reacted.reaction(lunch, BO, "heart", 9, false);
    assert_eq!((taken_back.seq, taken_back.previous), (2, Some(kept.key(&key))));
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
    let cut = reaction_at(
        &replayed,
        &Counted {
            slot: key.clone(),
            seq: 2,
        },
        None,
    );
    assert_eq!(cut.reactions(lunch), [("heart".to_owned(), vec![BO])]);
    assert!(cut.reaction_heads(lunch)[0].1.cut, "the taken-back record was cut");
}
