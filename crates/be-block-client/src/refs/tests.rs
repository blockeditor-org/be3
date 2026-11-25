use serde::{Serialize, de::DeserializeOwned};

use super::*;

mod a_block_this_account_lost_is_forgotten_once_what_was_set_aside_is_discarded;
mod a_discarded_blocks_edits_are_kept_again_only_on_the_servers_word;
mod a_key_current_again_after_a_break_is_timed_by_the_next_break;
mod a_log_fork_is_kept_across_a_reopen_and_the_first_stays;
mod discarding_puts_the_head_back_on_the_servers;
mod every_record_reads_back_with_each_option_none_and_some;
mod what_search_let_go_is_kept_across_a_reopen;

/// Writes `value` as a refs row holds it and reads it back, failing unless it reads back to the
/// same bytes.
#[track_caller]
fn reads_back<T: Serialize + DeserializeOwned>(value: &T) {
    let written = encode(value);
    let read: T = decode(&written).expect("the record reads back");
    assert_eq!(
        encode(&read),
        written,
        "the record reads back as it was written"
    );
}
