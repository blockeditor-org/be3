use super::*;
use crate::noise::{Initiator, Responder, hosted_prologue};

mod a_block_is_above_another_only_as_signed_parent_records_place_it;
mod a_contact_card_opens_only_with_its_code_in_its_role;
mod a_contact_verifies_what_another_signed_with_the_key_on_its_card;
mod a_grant_or_administrator_checks_only_as_one_who_may_make_it_signed_it;
mod a_kept_log_checks_only_what_was_appended_and_refuses_a_rewritten_prefix;
mod a_hosted_handshake_opens_only_between_the_keys_it_names;
mod a_key_change_carries_the_new_signing_key;
mod a_log_checked_on_from_any_entry_is_the_log_checked_whole;
mod a_log_tells_each_level_as_of_each_of_its_entries;
mod a_low_order_public_key_is_sealed_nothing_and_opens_nothing;
mod a_member_is_signed_in_by_an_invitation_and_an_agent_by_its_maker;
mod a_proof_path_proves_one_right_and_still_tells_what_lowered_anyone;
mod a_recovery_phrase_round_trips_through_its_words;
mod a_retired_key_signs_nothing_after_its_retirement;
mod a_sealed_key_opens_only_from_the_account_that_sealed_it;
mod a_share_link_grants_whoever_opens_it_until_it_is_revoked;
mod a_typed_code_is_normalized;
mod an_access_root_checks_only_as_its_creator_signed_it;
mod an_escrow_key_signs_the_next_and_opens_what_is_sealed_to_it;
mod an_invite_code_opens_only_its_own_organizations_key;
mod an_opening_checks_only_as_a_manager_signed_it;
mod an_owner_record_checks_only_as_the_owner_signed_it_and_a_path_cannot_hide_one;
mod any_range_and_any_newer_head_of_a_merkle_tree_verify_with_log_n_hashes;
mod authority_from_far_above_is_checked_once_per_log;
mod pairing_with_a_different_code_fails;
mod pairing_with_the_same_code_shares_a_key;
mod the_hosted_handshake_makes_noises_published_ik_vector;
mod what_an_account_seals_for_itself_opens_only_with_its_secret;

thread_local! {
    /// Signatures verified on this thread (`verify_signature`), so a test can tell how many a
    /// check verified.
    pub(crate) static VERIFIED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The signatures `run` verifies.
fn verified(run: impl FnOnce()) -> usize {
    let before = VERIFIED.with(std::cell::Cell::get);
    run();
    VERIFIED.with(std::cell::Cell::get) - before
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
        .collect()
}
fn key(text: &str) -> [u8; 32] {
    hex(text).try_into().unwrap()
}
