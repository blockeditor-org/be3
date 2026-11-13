use be_block_content::{BlockContent, LiveEdit, Merge};
use be_block_commit::MergeResult;

use crate::{Boat, BoatContent, BoatKey, BoatSettings, BoatSettingsContent};

mod a_connected_terminal_is_a_reference;
mod keys_pasted_on_two_devices_for_two_accounts_are_both_kept;
mod the_label_filter_is_a_key_and_a_value;

fn edited<C: LiveEdit + Clone>(start: &C, edits: impl IntoIterator<Item = C::Op>) -> C {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}

fn merged<C: Merge>(base: &C, ours: &C, theirs: &C) -> C {
    match C::merge3(base, ours, theirs) {
        MergeResult::Clean(value) => value,
        MergeResult::Conflicted { value, .. } => value,
    }
}

fn key(secret: &str) -> BoatKey {
    BoatKey {
        secret: secret.to_owned(),
        expires: 1_800_000_000,
        login: "sailor".to_owned(),
    }
}
