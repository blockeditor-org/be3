use be_block_model::LatestMap;

use super::*;
use crate::{ChatHeader, Deletion, MessageEdit};

#[test]
fn each_edit_names_the_one_it_replaces_and_a_reaction_taken_back_stays_a_record() {
    let chat = sent(
        &ChatContent::default(),
        ADA,
        &["lunch at noon?", "who is in?"],
    );
    let (lunch, who) = (first(&chat), chat.messages()[1]);

    let once = chat.next_edit(lunch, ADA, 5, "lunch at one?");
    assert_eq!(once.previous, None, "a message's first edit replaces none");
    let chat = edited(&chat, [chat.put_edit(lunch, &once)]);
    let twice = chat.next_edit(lunch, ADA, 6, "lunch at two?");
    assert_eq!(
        twice.previous,
        Some(once.key(lunch)),
        "a later edit names the one it replaces"
    );
    let chat = edited(&chat, [chat.put_edit(lunch, &twice)]);
    assert_eq!(chat.body(lunch).as_deref(), Some("lunch at two?"));
    assert!(chat.replaced(once.key(lunch)));
    assert!(!chat.replaced(twice.key(lunch)));

    // Someone with Edit puts the first edit back: it is the latest, and it was replaced, even
    // when they remove the record that it was.
    let replayed = edited(&chat, [chat.put_edit(lunch, &once)]);
    let shown = replayed
        .edit_of(lunch)
        .expect("the replayed edit is the latest");
    assert!(replayed.replaced(shown.key(lunch)));
    assert!(!replayed.edit_removed(lunch, ADA));

    // An edit removed outright, or overwritten by someone else's, is noticed.
    let edits: LatestMap<ObjectId, MessageEdit> =
        chat.header().field(ObjectId::ROOT, ChatHeader::EDITS);
    let removal = ChatHeader::EDITS.put(ObjectId::ROOT, &lunch, None, edits.next(&lunch, 9, BO));
    let removed = edited(&chat, [[removal].into_iter().collect()]);
    assert_eq!(removed.edit_of(lunch), None);
    assert!(removed.edit_removed(lunch, ADA));
    let overwritten = edited(&chat, [chat.edit(lunch, BO, 9, "Bo says three")]);
    assert!(overwritten.edit_removed(lunch, ADA));

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

    // Reactions chain the same way: taking one back stays a record, and putting the kept one
    // back over it is a record that was replaced.
    let reacted = edited(&chat, [chat.react(lunch, BO, "heart", 8)]);
    assert_eq!(reacted.reactions(lunch), [("heart".to_owned(), vec![BO])]);
    let (key, kept) = chat.reaction(lunch, BO, "heart", 8, true);
    let reacted = edited(&chat, [chat.put_reaction(&key, &kept)]);
    let (_, taken_back) = reacted.reaction(lunch, BO, "heart", 9, false);
    assert_eq!(taken_back.previous, Some(kept.key(&key)));
    let unreacted = edited(&reacted, [reacted.put_reaction(&key, &taken_back)]);
    assert!(
        unreacted.reactions(lunch).is_empty(),
        "a reaction taken back is not shown"
    );
    assert!(
        unreacted.reaction_records(lunch)[0]
            .1
            .as_ref()
            .is_some_and(|reaction| !reaction.kept),
        "it stays as a record its author can sign"
    );
    let replayed = edited(&unreacted, [unreacted.put_reaction(&key, &kept)]);
    assert_eq!(replayed.reactions(lunch), [("heart".to_owned(), vec![BO])]);
    assert!(
        replayed.replaced(kept.key(&key)),
        "the kept record put back was replaced"
    );
}
