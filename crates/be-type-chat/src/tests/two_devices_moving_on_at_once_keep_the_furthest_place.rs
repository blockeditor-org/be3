use be_block_model::Document;

use super::*;
use crate::{ChatReads, ChatReadsContent};

const LAPTOP: Uuid = Uuid::from_u128(0x1a);
const PHONE: Uuid = Uuid::from_u128(0x9e);

#[test]
fn two_devices_moving_on_at_once_keep_the_furthest_place() {
    let chat = sent(
        &ChatContent::default(),
        ADA,
        &["m1", "m2", "m3", "m4", "m5", "m6", "m7"],
    );
    let messages = chat.messages();
    let (m3, m5, m7) = (messages[2], messages[4], messages[6]);
    let base: ChatReadsContent = edited_reads(
        &Document::default(),
        [
            ChatReads::set_read(CHAT, LAPTOP, m3, &[]),
            ChatReads::set_read(CHAT, PHONE, m3, &[]),
        ],
    );
    let seen = base.root().places(CHAT);
    // The phone, offline, reads to m5, and the laptop, online, to m7: each drops the places it
    // saw, which were both at m3.
    let phone = ChatReads::set_read(CHAT, PHONE, m5, &seen);
    let laptop = ChatReads::set_read(CHAT, LAPTOP, m7, &seen);

    let furthest = |reads: &ChatReadsContent| {
        let places: Vec<_> = reads
            .root()
            .places(CHAT)
            .into_iter()
            .map(|place| place.message)
            .collect();
        chat.unread(places, BO)
    };
    for order in [[&phone, &laptop], [&laptop, &phone]] {
        let reads = edited_reads(&base, order.into_iter().cloned());
        assert_eq!(
            furthest(&reads),
            0,
            "in either order the laptop's m7 counts"
        );
    }

    let ours = edited_reads(&base, [phone]);
    let theirs = edited_reads(&base, [laptop]);
    match ChatReadsContent::merge3(&base, &ours, &theirs) {
        MergeResult::Clean(merged) => {
            assert_eq!(furthest(&merged), 0, "merged, the laptop's m7 counts");
            assert_eq!(
                merged.root().places(CHAT).len(),
                2,
                "each device's new place is kept"
            );
        }
        MergeResult::Conflicted { .. } => panic!("dropping the same seen places conflicts"),
    }
}

fn edited_reads(
    start: &ChatReadsContent,
    edits: impl IntoIterator<Item = Edit>,
) -> ChatReadsContent {
    let mut reads = start.clone();
    for edit in edits {
        reads.apply(&edit);
    }
    reads
}
