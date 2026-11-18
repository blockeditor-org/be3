use be_block_content::{LogItem, LogSigner};
use uuid::Uuid;

use crate::{ChatEntry, ChatLog};

mod a_message_shows_as_trusted_as_the_least_trusted_entry_it_shows;
mod a_message_shows_its_authors_latest_edit_and_no_one_elses;
mod a_sent_message_lists_what_it_carries_until_it_is_deleted;
mod an_entry_redacted_by_anyone_but_its_author_shows_removed;
mod each_persons_latest_record_of_a_reaction_counts;
mod only_a_messages_author_deletes_it_and_its_redacted_entry_shows_deleted;
mod unread_counts_others_messages_after_the_furthest_place_and_the_readers_own;

const ADA: Uuid = Uuid::from_u128(0xada);
const BO: Uuid = Uuid::from_u128(0xb0);

/// The entry at `seq`, written by `author`.
fn item(seq: u64, author: Uuid, entry: &ChatEntry) -> LogItem {
    signed(seq, author, entry, LogSigner::Verified)
}

fn signed(seq: u64, author: Uuid, entry: &ChatEntry, signer: LogSigner) -> LogItem {
    LogItem {
        seq,
        id: Uuid::from_u64_pair(7, seq),
        author,
        arrival: 1_000 + seq as i64,
        payload: Some(entry.encode()),
        signer,
    }
}

fn message(body: &str) -> ChatEntry {
    ChatEntry::Message {
        time: 0,
        body: body.to_owned(),
        reply_to: None,
        attachments: Vec::new(),
    }
}

fn bodies(log: &ChatLog) -> Vec<Option<String>> {
    log.messages()
        .filter(|shown| shown.deleted.is_none())
        .map(|shown| shown.body.clone())
        .collect()
}
