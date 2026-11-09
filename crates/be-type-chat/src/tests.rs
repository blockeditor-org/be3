use be_block::{BlockContent, LiveEdit, Merge};
use be_block_commit::MergeResult;
use be_block_model::{Edit, ObjectId};
use uuid::Uuid;

use crate::{ChatContent, Message};

mod a_message_hash_covers_what_it_signs_and_a_sent_message_notes_the_last_one_seen;
mod a_saved_chat_loads_back_from_its_parts;
mod deleting_a_message_removes_it_with_its_edit_and_reactions;
mod edits_and_reactions_made_offline_on_both_sides_are_kept_by_message;
mod two_replicas_that_each_add_messages_offline_keep_both_sides_in_order;

const CHAT: Uuid = Uuid::from_u128(0xc4a7);
const ADA: Uuid = Uuid::from_u128(0xada);
const BO: Uuid = Uuid::from_u128(0xb0);

fn edited(start: &ChatContent, edits: impl IntoIterator<Item = Edit>) -> ChatContent {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}

fn sent(chat: &ChatContent, author: Uuid, bodies: &[&str]) -> ChatContent {
    let mut chat = chat.clone();
    for (index, body) in bodies.iter().enumerate() {
        let (_, send) = chat.send(CHAT, Message::new(author, index as i64, *body));
        chat.apply(&send);
    }
    chat
}

fn bodies(chat: &ChatContent) -> Vec<String> {
    chat.messages()
        .into_iter()
        .filter_map(|id| chat.body(id))
        .collect()
}

fn merged(base: &ChatContent, ours: &ChatContent, theirs: &ChatContent) -> (ChatContent, usize) {
    match ChatContent::merge3(base, ours, theirs) {
        MergeResult::Clean(value) => (value, 0),
        MergeResult::Conflicted { value, conflicts } => (value, conflicts),
    }
}

fn round_trip(chat: &ChatContent) -> ChatContent {
    ChatContent::decode(&chat.encode()).expect("a chat decodes what it encoded")
}

fn first(chat: &ChatContent) -> ObjectId {
    chat.messages()[0]
}
