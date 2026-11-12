use be_block::{BlockContent, LiveEdit, Merge};
use be_block_commit::MergeResult;
use be_block_model::{Edit, ObjectId};
use uuid::Uuid;

use crate::{ChatContent, Message};

mod a_chat_decoded_from_bytes_alone_keeps_its_segments_when_saved_again;
mod a_close_whose_index_entry_does_not_match_its_bytes_is_ignored;
mod a_closed_segment_saves_as_a_part_and_loads_back_without_being_read;
mod a_merged_index_keeps_the_entries_in_time_order;
mod a_message_hash_covers_what_it_signs_and_a_sent_message_notes_the_last_one_seen;
mod a_message_sent_in_a_live_session_lands_after_those_sequenced_before_it;
mod a_reaction_made_offline_to_a_message_moved_into_a_segment_survives_the_merge;
mod a_saved_chat_loads_back_from_its_parts;
mod a_segment_closed_twice_or_over_messages_that_changed_closes_once;
mod a_segment_read_is_checked_against_the_index;
mod a_small_message_before_an_oversized_one_does_not_stall_closing;
mod an_edit_by_someone_other_than_the_author_is_not_shown;
mod an_edit_made_offline_to_a_message_deleted_elsewhere_stays_deleted;
mod deleting_a_message_removes_it_with_its_edit_and_reactions;
mod edits_and_reactions_made_offline_on_both_sides_are_kept_by_message;
mod merging_saved_sides_with_differing_closes_keeps_each_segments_own_part;
mod segments_read_back_that_overlap_weave_into_one_run_in_order;
mod the_same_edit_or_reaction_made_on_two_devices_keeps_the_later_without_a_conflict;
mod two_closed_chats_merge_by_parts_without_reading_a_segment;
mod two_closes_made_apart_merge_into_one_segment_without_a_conflict;
mod two_devices_moving_on_at_once_keep_the_furthest_place;
mod two_replicas_that_each_add_messages_offline_keep_both_sides_in_order;
mod unread_counts_from_the_furthest_message_read_by_order_not_by_claimed_time;
mod unread_counts_reach_back_into_a_closed_segment;

const CHAT: Uuid = Uuid::from_u128(0xc4a7);
const ADA: Uuid = Uuid::from_u128(0xada);
const BO: Uuid = Uuid::from_u128(0xb0);

fn edited(start: &ChatContent, edits: impl IntoIterator<Item = Edit>) -> ChatContent {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit.into());
    }
    content
}

fn sent(chat: &ChatContent, author: Uuid, bodies: &[&str]) -> ChatContent {
    let mut chat = chat.clone();
    for (index, body) in bodies.iter().enumerate() {
        let (_, send) = chat.send(CHAT, Message::new(author, index as i64, *body));
        chat.apply(&send.into());
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

/// A chat with `count` messages of about a kilobyte each, by `ADA`.
fn long(count: usize) -> ChatContent {
    let body = "a line of chat ".repeat(70);
    let bodies: Vec<&str> = (0..count).map(|_| body.as_str()).collect();
    sent(&ChatContent::default(), ADA, &bodies)
}

/// `chat` with its oldest messages closed into a segment.
fn closed(chat: &ChatContent) -> ChatContent {
    let closing = chat.closing().expect("a chat this long closes a segment");
    let mut chat = chat.clone();
    chat.apply(&closing);
    chat
}
