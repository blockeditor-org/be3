use be_block_content::{BlockContent, ContentError};
use be_block_graph::BlockParent;
use be_block_protocol::Listing;
use be_block_store::{ChunkerConfig, MemoryStore};
use be_server::Running;
use uuid::Uuid;

use super::*;
use be_block_graph::Access;
use be_block_keys::{
    Authority, Entry, Grant, Level, LogKind, Record, RecoveryPhrase, Reduced, Step,
};
use be_block_keys::{CarriedKey, EntryHash, Heads, Party, RecoveryPublic};
use be_block_protocol::KeyKind;
use be_block_store::Keyring;
use be_type_image::{ImageContent, ImageHeader};
use be_type_text::{TextBlock, TextContent};

/// Uploads `objects` as `Peer::push` does, without keeping them on the device: asks for URLs,
/// then PUTs each object's bytes.
async fn upload(peer: &Peer<MemoryStore>, objects: Vec<Vec<u8>>) -> Result<(), ClientError> {
    let lengths = objects
        .iter()
        .map(|bytes| be_block_protocol::ObjectLength {
            hash: be_block_store::Hash::of(bytes),
            length: Some(bytes.len() as u64),
        })
        .collect();
    let urls = crate::ask(be_block_protocol::write::<
        Vec<Option<be_block_protocol::UploadUrl>>,
        _,
    >(
        &peer.connection(),
        Some(peer.organization()),
        be_block_protocol::Op::ReserveUploads(lengths),
    ))
    .await?;
    for (url, bytes) in urls.into_iter().zip(objects) {
        let Some(url) = url else {
            continue;
        };
        let headers: Vec<(String, String)> = url
            .headers
            .into_iter()
            .map(|header| (header.name, header.value))
            .collect();
        crate::http::put(&url.url, &headers, bytes)
            .await
            .map_err(|error| ClientError::Disconnected(error.to_string()))?;
    }
    Ok(())
}

mod a_banned_member_is_refused_until_a_grant_lifts_the_ban;
mod a_block_a_shared_one_links_to_shows_only_that_it_exists;
mod a_block_deleted_after_its_folder_was_listed_is_listed_only_as_deleted_offline;
mod a_block_made_offline_keeps_its_edits_when_read_before_it_is_sent;
mod a_block_made_offline_takes_the_access_the_server_will_give_it;
mod a_block_made_offline_under_a_scope_the_server_claims_is_set_aside;
mod a_block_made_under_one_whose_keys_cannot_be_asked_for_is_not_locked;
mod a_block_made_while_an_escrow_key_is_minted_wraps_under_the_one_the_server_holds;
mod a_block_moved_back_into_a_scope_takes_what_was_made_below_it_meanwhile;
mod a_block_moved_out_of_a_listed_folder_is_listed_where_it_went_offline;
mod a_block_moved_out_of_an_owner_only_block_is_escrowed_as_any_shared_one;
mod a_block_moved_to_another_workspace_is_rewrapped_and_given_a_fresh_key;
mod a_block_named_then_written_offline_keeps_the_metadata_it_was_sealed_with;
mod a_block_opened_offline_that_someone_else_made_keeps_their_root;
mod a_block_placed_in_a_folder_after_it_was_listed_is_listed_there_offline;
mod a_block_referencing_too_many_blocks_is_refused;
mod a_block_restored_into_a_scope_rotated_while_it_was_in_the_trash_is_wrapped_under_its_new_key;
mod a_block_s_ownership_moves_and_its_old_owner_grants_no_more;
mod a_block_sent_after_a_new_escrow_key_is_wrapped_under_it;
mod a_block_the_server_places_under_a_contacts_scope_is_wrapped_under_nothing_there;
mod a_block_the_server_refused_to_make_is_not_listed_offline;
mod a_block_the_server_says_is_open_inside_is_sealed_to_no_contact;
mod a_block_this_account_makes_on_another_device_after_a_discard_pins_its_root;
mod a_block_whose_edit_was_set_aside_as_outdated_takes_the_next_one_once_it_is_discarded;
mod a_block_whose_root_the_server_swaps_is_refused;
mod a_change_the_server_refuses_leaves_the_rest_of_the_outbox_to_send;
mod a_claim_moves_the_lease_and_the_old_holders_next_seal_is_rejected;
mod a_connect_nobody_answers_times_out;
mod a_connection_asked_to_close_is_closed_to_everyone_at_once;
mod a_connection_outlives_a_stall_of_this_process;
mod a_contact_connected_again_by_a_new_link_outranks_every_devices_pin;
mod a_contact_is_named_by_the_card_it_verified_not_the_servers_row;
mod a_contact_link_revoked_before_it_is_finished_grants_nothing;
mod a_contact_made_on_one_device_is_read_by_another_from_the_accounts_block;
mod a_contact_taken_and_removed_in_one_pass_stays_removed_whatever_is_replayed;
mod a_contact_the_log_never_names_hands_over_a_key_only_with_edit_from_above;
mod a_contact_the_server_lists_as_granted_with_no_record_is_sealed_nothing;
mod a_contact_whose_key_changes_outran_the_mailbox_is_not_confirmed_until_linked_again;
mod a_contacts_key_the_server_rolls_back_stays_the_newest_one_seen;
mod a_contacts_seal_waits_on_a_log_short_of_what_was_kept;
mod a_creation_another_device_of_the_account_made_first_keeps_its_root;
mod a_creation_discarded_unsent_lets_its_root_go;
mod a_creation_the_server_refuses_is_set_aside_and_nothing_waits_behind_it;
mod a_data_key_a_revoked_member_makes_up_is_never_written_under;
mod a_debug_print_shows_no_key;
mod a_definition_the_server_refuses_leaves_the_block_as_the_server_has_it;
mod a_demoted_administrator_who_seals_a_made_up_escrow_key_installs_nothing;
mod a_demoted_administrator_with_the_server_reads_nothing_written_after_the_demotion;
mod a_demotion_hidden_from_an_organization_path_names_no_administrator;
mod a_device_following_a_tree_hears_another_devices_publish;
mod a_device_reads_every_page_of_its_keys;
mod a_device_shown_a_kind_still_open_seals_the_new_epoch_to_no_contact;
mod a_device_shown_a_log_without_a_revocation_seals_the_new_epoch_to_no_one;
mod a_device_shown_an_administrator_not_yet_demoted_seals_them_no_new_escrow_key;
mod a_device_that_knows_a_head_only_from_its_summary_saves_on_it_offline;
mod a_device_that_read_fewer_key_changes_never_rolls_a_contact_back;
mod a_device_that_starts_offline_reads_and_lists_what_it_saved;
mod a_file_added_to_a_shared_folder_opens_and_saves_for_who_it_was_shared_with;
mod a_folder_listed_again_answers_offline_as_of_the_newer_listing;
mod a_folder_of_blocks_with_large_metadata_reads_in_answers_that_fit_a_frame;
mod a_folder_shared_with_a_contact_opens_for_them_alone;
mod a_grantee_inside_a_workspace_writes_once_an_editor_above_tends_after_a_demotion;
mod a_grantee_who_is_not_a_contact_gets_the_key_by_the_log;
mod a_grantee_who_never_signed_retires_its_old_key_too;
mod a_guest_a_share_link_admits_is_named_only_itself;
mod a_guest_sees_no_members_or_settings_and_leaves_with_its_last_grant;
mod a_host_and_a_member_whose_access_comes_from_above_meet;
mod a_join_member_refuses_a_host_the_server_makes_up;
mod a_joiner_whose_grant_was_revoked_is_refused_by_the_host;
mod a_key_a_contact_seals_after_their_revocation_is_not_taken;
mod a_key_change_reaches_a_contact_offline_and_is_read_from_its_mailbox;
mod a_key_opened_from_escrow_arrives_and_unlocks_its_block;
mod a_key_opened_through_a_chain_longer_than_any_tree_is_outdated;
mod a_key_sealed_after_the_block_was_listed_unlocks_it;
mod a_key_sealed_by_a_contact_verified_later_arrives_as_it_is_verified;
mod a_key_the_server_labels_as_the_organizations_settings_never_seals_another_block;
mod a_key_the_server_or_a_stranger_seals_to_a_member_is_never_used;
mod a_key_the_server_wraps_under_the_settings_key_and_names_current_seals_nothing;
mod a_key_wrapped_under_the_settings_key_seals_no_other_block;
mod a_kind_opened_on_authority_from_above_is_offered_to_contacts_inside;
mod a_kind_opened_then_closed_is_sealed_to_no_contact;
mod a_large_block_saved_after_a_small_change_uploads_only_what_changed;
mod a_large_image_streams_without_downloading_all_of_it;
mod a_link_carrying_access_is_refused_where_nothing_is_shared_outside;
mod a_listed_block_this_device_may_no_longer_know_of_is_left_out_offline;
mod a_local_account_keeps_what_it_makes_as_final_with_no_server;
mod a_local_accounts_log_is_numbered_on_the_device;
mod a_log_appended_offline_is_sent_in_order_once_the_device_reconnects;
mod a_log_the_server_cut_or_forked_is_reported_and_not_taken;
mod a_long_listing_is_read_page_by_page;
mod a_lowering_shown_past_a_paths_last_whole_entry_is_not_forgotten;
mod a_member_served_the_organizations_log_short_of_a_demotion_escrows_and_seals_nothing_to_them;
mod a_member_the_server_adds_is_listed_as_signed_in_by_no_one;
mod a_member_who_roots_a_block_they_do_not_hold_is_taken_only_weakly;
mod a_member_with_view_is_served_no_other_members_grant;
mod a_merge_left_waiting_is_sent_once_the_device_reconnects;
mod a_merge_on_a_local_commit_under_a_made_up_key_is_set_aside_not_uploaded;
mod a_move_refused_to_an_editor_drops_only_the_move;
mod a_new_device_opens_a_company_organization_from_the_pin_in_the_accounts_block;
mod a_new_phrase_leaves_the_old_one_unable_to_read_the_accounts_next_commit;
mod a_notification_opens_only_for_the_blocks_members;
mod a_notification_sealed_under_another_blocks_key_is_refused;
mod a_page_checked_against_a_head_that_moved_meanwhile_is_read_again;
mod a_part_put_for_an_operation_is_saved_by_another_member_without_uploading_it;
mod a_policy_outside_the_organizations_settings_is_not_enforced;
mod a_publish_the_server_refuses_as_invalid_is_set_aside_not_sent_again;
mod a_reader_cannot_read_a_block_moved_out_of_what_they_were_shared;
mod a_reader_granted_by_an_administrator_checks_its_path_through_the_organizations;
mod a_reader_granted_from_above_checks_its_grant_on_the_ancestors_path;
mod a_reader_taken_off_a_workspace_cannot_read_what_is_written_after;
mod a_real_clock_places_an_instant_ahead_of_now_that_far_ahead;
mod a_rebased_journal_reads_back_on_its_base_with_what_came_after;
mod a_record_found_forged_is_not_asked_about_again;
mod a_record_the_old_key_signs_after_a_new_phrase_is_refused;
mod a_record_the_server_replays_after_a_removal_does_not_bring_the_contact_back;
mod a_relay_sealed_under_another_blocks_key_is_refused;
mod a_reordered_reference_list_keeps_its_new_order;
mod a_request_fails_once_a_ping_goes_unanswered;
mod a_request_on_a_link_gone_silent_fails_without_waiting_for_the_idle_ping;
mod a_request_stuck_in_a_full_socket_fails_once_the_server_goes_silent;
mod a_restarted_device_served_a_log_before_a_revocation_it_held_writes_under_no_made_up_key;
mod a_revocation_the_server_makes_is_signed_into_the_log_by_a_manager;
mod a_save_keeps_defined_metadata_it_does_not_know_and_drops_stale_derived_metadata;
mod a_save_under_an_epoch_the_log_outdates_rotates_first;
mod a_saved_record_that_does_not_decode_leaves_the_rest_readable;
mod a_second_administrator_edits_the_organizations_settings;
mod a_share_link_admits_only_members_unless_the_policy_lets_it_admit_guests;
mod a_share_link_grants_view_to_an_account_no_one_granted;
mod a_share_link_opened_after_a_rotation_unlocks_once_an_editor_tends;
mod a_signature_is_verified_by_a_contacts_key_pinned_from_the_server_or_refused;
mod a_stale_card_does_not_make_a_replaced_signing_key_current;
mod a_stale_save_is_rejected_with_the_head_to_merge_against;
mod a_submission_reaches_every_participant_numbered_and_sealed;
mod a_summary_read_before_a_publish_never_brings_back_the_old_head;
mod a_summary_read_before_a_rotation_never_brings_back_the_old_key;
mod a_summary_read_while_a_move_waits_to_be_sent_shows_the_move;
mod a_tail_the_server_makes_up_after_a_proof_path_is_not_kept;
mod a_workspace_made_offline_reaches_the_server_with_its_keys;
mod a_workspace_not_kept_for_offline_use_is_forgotten_but_its_unsent_edits_stay;
mod a_wrap_the_server_seals_to_the_escrow_carries_no_root_anyone_trusts;
mod accepting_an_invitation_signs_the_member_in_with_no_administrator_online;
mod an_administrator_edits_policies_another_administrator_made;
mod an_administrator_made_after_a_demotion_opens_the_settings;
mod an_administrator_reads_a_workspace_nobody_shared_with_them;
mod an_administrators_device_shares_the_settings_with_the_server_again_when_it_connects;
mod an_administrators_new_phrase_leaves_the_server_reading_the_policies;
mod an_artifact_is_a_backref_of_the_block_it_describes;
mod an_edit_made_offline_across_a_demotion_is_sealed_again_under_a_key_the_demoted_never_get;
mod an_edit_made_offline_by_an_author_demoted_since_stays_set_aside;
mod an_edit_made_offline_under_a_made_up_key_is_sealed_again_under_a_key_bob_never_gets;
mod an_edit_the_server_kept_without_answering_settles_once_another_device_builds_on_it;
mod an_editor_rotates_a_workspace_a_reader_was_taken_off;
mod an_entry_signed_with_a_replaced_key_after_its_replacement_answers_replaced;
mod an_escrow_key_a_demoted_administrator_makes_is_refused;
mod an_escrow_key_the_server_puts_in_place_is_refused;
mod an_escrow_secret_is_held_apart_from_content_keys;
mod an_escrow_wrap_from_a_revoked_editor_waits_on_a_log_short_of_what_was_kept;
mod an_exported_artifact_imports_pointing_to_its_imported_source;
mod an_image_round_trips_without_being_re_encoded;
mod an_import_another_device_of_the_account_made_first_stands;
mod an_import_under_the_id_of_a_block_someone_else_made_is_set_aside;
mod an_offline_edit_merges_against_the_head_it_left_after_the_server_pruned_it;
mod an_offline_save_under_a_key_named_between_the_last_lowering_and_the_head_goes_through;
mod an_old_key_the_server_relabels_as_a_new_epoch_opens_nothing;
mod an_older_pin_the_server_sends_again_is_not_taken;
mod an_organization_whose_root_the_server_swaps_is_refused;
mod an_organizations_policies_are_read_by_its_members_and_enforced_by_the_server;
mod an_owner_only_block_is_read_by_its_author_alone;
mod an_owner_only_blocks_keys_are_sealed_to_nobody_key_work_lists;
mod another_blocks_commit_served_as_a_head_is_refused;
mod another_blocks_key_named_as_current_is_refused;
mod contacts_keep_sharing_after_one_of_them_takes_a_new_recovery_phrase;
mod contents_read_together_match_what_each_block_saved;
mod edits_sealed_offline_are_published_when_the_device_reconnects;
mod edits_to_a_block_whose_access_was_lost_wait_aside_until_it_returns;
mod finishing_a_contact_link_again_does_not_undo_a_revocation;
mod history_is_thinned_but_the_head_and_bookmarks_survive;
mod kept_edits_sealed_with_a_key_not_on_the_device_keep_their_origin_until_it_arrives;
mod kept_edits_set_aside_are_bounded_told_once_they_read_and_never_sent;
mod keys_read_before_a_creation_someone_else_made_is_set_aside_keep_their_root;
mod large_files_saved_at_once_through_a_relay_all_arrive;
mod leaving_a_shared_workspace_leaves_its_rotation_to_who_can_edit_it;
mod making_a_block_whose_log_this_device_holds_keeps_its_root;
mod metadata_is_sealed_on_the_server_and_opened_by_members;
mod metadata_that_does_not_decode_is_an_error_rather_than_blank;
mod moving_a_block_to_the_top_follows_who_may_make_workspaces;
mod objects_move_between_devices_and_a_bucket_directly;
mod objects_move_through_the_servers_own_s3_endpoint;
mod references_declared_by_content_reach_the_graph;
mod relayed_messages_reach_one_peer_or_all_of_them_sealed;
mod removing_a_member_rotates_a_workspaces_search_index_with_it;
mod removing_a_member_rotates_the_organizations_settings;
mod removing_a_member_shuts_them_out_and_rotates_what_they_could_read;
mod summaries_naming_a_block_twice_keep_its_newer_head;
mod swapping_ids_replaces_every_written_form_of_each;
mod the_first_page_of_a_long_log_costs_that_page_and_log_n_hashes;
mod the_next_editor_seals_once_the_server_writes_a_lost_holders_leave;
mod the_server_naming_itself_in_key_work_is_sealed_nothing;
mod two_devices_sealing_one_head_keep_only_the_winners_trim;
mod two_peers_of_one_account_read_each_others_blocks;
mod what_a_policy_governs_waits_while_the_server_cannot_read_it;
mod what_is_in_force_at_a_block_not_sent_yet_is_answered_from_this_device;
mod what_is_made_in_a_block_the_server_refused_waits_aside_with_it;
mod whether_a_blocks_key_is_here_is_not_told_offline;

struct Harness {
    server: Running,
    url: String,
}

impl Harness {
    async fn start() -> Self {
        Self::configured(be_server::ServerConfig::OPEN).await
    }

    async fn configured(config: be_server::ServerConfig) -> Self {
        be_lib_build_id::install("test");
        let server = Running::start(config).await;
        Self {
            url: format!("ws://{}", server.address),
            server,
        }
    }

    async fn owner(&self, email: &str) -> Peer<MemoryStore> {
        Peer::connect(
            PeerConfig::new(
                &self.url,
                identity(email),
                Credentials::Token(self.server.register(email, email).await),
            )
            .chunker(ChunkerConfig::SMALL),
            MemoryStore::new(),
        )
        .await
        .unwrap()
    }

    async fn second(&self, owner: &Peer<MemoryStore>) -> Peer<MemoryStore> {
        Peer::connect(
            PeerConfig::new(
                &self.url,
                owner.keys().identity().clone(),
                Credentials::Token(owner.token().to_owned()),
            )
            .organization(Some(owner.organization()))
            .chunker(ChunkerConfig::SMALL),
            MemoryStore::new(),
        )
        .await
        .unwrap()
    }

    async fn member(&self, admin: &Peer<MemoryStore>, email: &str) -> Peer<MemoryStore> {
        self.member_pinning(admin, email, admin.keys().escrow())
            .await
    }

    async fn member_pinning(
        &self,
        admin: &Peer<MemoryStore>,
        email: &str,
        escrow: Option<be_block_protocol::EscrowKey>,
    ) -> Peer<MemoryStore> {
        let own = self.owner(email).await;
        let card = crate::contacts::card(
            admin.account(),
            admin.keys().identity(),
            "Admin",
            "admin@example.com",
        );
        let code = crate::contacts::invite(
            &admin.connection(),
            admin.keys().identity(),
            &card,
            admin.organization(),
            &crate::contacts::Pinned {
                root: admin.keys().organization_root(),
                escrow,
                made: 0,
            },
            &admin.checked_organization().await.unwrap().unwrap(),
        )
        .await
        .unwrap();
        let theirs = crate::contacts::card(own.account(), own.keys().identity(), email, email);
        crate::contacts::accept_link(
            &own.connection(),
            own.keys().identity(),
            &theirs,
            &code,
            crate::contacts::Expected::Invitation,
        )
        .await
        .unwrap();
        admin.finish_links().await.unwrap();
        admin.refresh_contacts().await.unwrap();
        Peer::connect(
            PeerConfig::new(
                &self.url,
                own.keys().identity().clone(),
                Credentials::Token(own.token().to_owned()),
            )
            .organization(Some(admin.organization()))
            .chunker(ChunkerConfig::SMALL),
            MemoryStore::new(),
        )
        .await
        .unwrap()
    }

    fn config(&self, like: &Peer<MemoryStore>, refs: &refs::RefsDb) -> PeerConfig {
        PeerConfig::new(
            &self.url,
            like.keys().identity().clone(),
            Credentials::Token(like.token()),
        )
        .organization(Some(like.organization()))
        .chunker(ChunkerConfig::SMALL)
        .refs(refs.clone())
    }

    async fn device(
        &self,
        like: &Peer<MemoryStore>,
        refs: &refs::RefsDb,
        store: &MemoryStore,
    ) -> Peer<MemoryStore> {
        Peer::connect(self.config(like, refs), store.clone())
            .await
            .unwrap()
    }

    fn offline(
        &self,
        like: &Peer<MemoryStore>,
        refs: &refs::RefsDb,
        store: &MemoryStore,
    ) -> Peer<MemoryStore> {
        Peer::cached(self.config(like, refs), like.account(), store.clone()).unwrap()
    }

    /// A new account's connection to `url` (the server, or a relay in front of it), signed in
    /// with its token, keeping time by `clock`.
    async fn connection(&self, url: &str, clock: &Clock) -> std::sync::Arc<Connection> {
        let token = self
            .server
            .register(&format!("{}@example.com", Uuid::new_v4()), "Connection")
            .await;
        Connection::connect(url, clock, &Credentials::Token(token))
            .await
            .expect("the connection opens signed in")
            .0
    }

    async fn reconnect(&self, peer: &Peer<MemoryStore>, token: String) {
        let (connection, signed_in) =
            Connection::connect(&self.url, &Clock::real(), &Credentials::Token(token))
                .await
                .expect("the server signs the device in");
        peer.attach(connection, signed_in.token).await.unwrap();
    }

    fn database(&self) -> rusqlite::Connection {
        be_block_sqlite_schema::open(self.server.directory.join("metadata.sqlite")).unwrap()
    }

    async fn stop(self) {
        self.server.stop().await;
    }
}

/// Whether `events` already holds `Event::KeysChanged`, without waiting: a key that opens here
/// is told before the call that opened it returns (`KeyBook::on_opened`).
fn told_keys_changed(events: &mut tokio::sync::broadcast::Receiver<crate::Event>) -> bool {
    loop {
        match events.try_recv() {
            Ok(crate::Event::KeysChanged) => return true,
            Ok(_) | Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
            Err(_) => return false,
        }
    }
}

async fn workspace(peer: &Peer<MemoryStore>, name: &str) -> Uuid {
    peer.create_block(
        Uuid::now_v7(),
        be_type_workspace::WorkspaceContent::CONTENT_TYPE,
        BlockParent::Root,
        &be_block_content::BlockMetadata::named(name),
        false,
    )
    .await
    .unwrap()
    .id
}

async fn my_files(peer: &Peer<MemoryStore>) -> Uuid {
    peer.create_block(
        be_block::my_files(peer.organization(), peer.account()),
        be_type_workspace::WorkspaceContent::CONTENT_TYPE,
        BlockParent::Root,
        &be_block_content::BlockMetadata::named("My files"),
        false,
    )
    .await
    .unwrap()
    .id
}

/// Imports `saved` at the root and uploads it, with every answer `queue_import` and
/// `upload_import` give.
async fn import_one(
    peer: &Peer<MemoryStore>,
    saved: crate::transfer::Saved,
) -> Vec<crate::peer::Imported> {
    let mut answers = Vec::new();
    let queued = peer
        .queue_import(&[(saved, BlockParent::Root)], |_, imported| {
            answers.push(imported)
        })
        .await;
    peer.upload_import(queued, |_, imported| answers.push(imported))
        .await;
    answers
}

async fn text(peer: &Peer<MemoryStore>, block: Uuid) -> String {
    let opened = peer
        .open::<be_type_text::TextContent>(block)
        .await
        .unwrap()
        .expect("the block has content");
    be_type_text::TextBlock::text(&opened)
}

/// A device on `harness` with the default chunker, so a large text is a few large objects.
async fn device(harness: &Harness, email: &str) -> Peer<MemoryStore> {
    Peer::connect(
        PeerConfig::new(
            &harness.url,
            identity(email),
            Credentials::Token(harness.server.register(email, email).await),
        ),
        MemoryStore::new(),
    )
    .await
    .unwrap()
}

/// Another device of `owner`'s account.
async fn another(harness: &Harness, owner: &Peer<MemoryStore>) -> Peer<MemoryStore> {
    Peer::connect(
        PeerConfig::new(
            &harness.url,
            owner.keys().identity().clone(),
            Credentials::Token(owner.token().to_owned()),
        )
        .organization(Some(owner.organization())),
        MemoryStore::new(),
    )
    .await
    .unwrap()
}

/// A text block in a workspace of its own.
async fn note(owner: &Peer<MemoryStore>) -> Uuid {
    let workspace = owner
        .create_block(
            Uuid::now_v7(),
            be_type_workspace::WorkspaceContent::CONTENT_TYPE,
            BlockParent::Root,
            &be_block_content::BlockMetadata::named("Notes"),
            false,
        )
        .await
        .unwrap()
        .id;
    owner
        .create::<be_type_text::TextContent>(BlockParent::Block(workspace))
        .await
        .unwrap()
}

/// A text too large to travel inside a publish, so its objects move through URLs.
fn large_text() -> String {
    (0..400_000).map(|line| format!("line {line}\n")).collect()
}

/// Moves `account` from `old` to a new phrase, leaving a key change in each contact's mailbox.
async fn new_phrase(
    connection: &std::sync::Arc<crate::Connection>,
    account: Uuid,
    old: &be_block_keys::RecoverySecret,
) -> be_block_keys::RecoverySecret {
    let fresh = be_block_keys::RecoveryPhrase::generate().secret();
    take_phrase(connection, account, old, &fresh).await;
    fresh
}

/// Moves `account` from `old` to `fresh` as a device does (`contacts::set_recovery_key`).
async fn take_phrase(
    connection: &std::sync::Arc<crate::Connection>,
    account: Uuid,
    old: &be_block_keys::RecoverySecret,
    fresh: &be_block_keys::RecoverySecret,
) {
    crate::contacts::set_recovery_key(
        connection,
        account,
        Some(old),
        fresh,
        &std::collections::HashMap::new(),
    )
    .await
    .expect("a new phrase never waits on a contact's mailbox")
    .expect("the old phrase opens everything it sealed");
}

fn identity(email: &str) -> be_block_keys::RecoverySecret {
    let mut bytes = [7u8; 32];
    for (index, byte) in email.bytes().enumerate() {
        bytes[index % 32] = bytes[index % 32].wrapping_mul(31).wrapping_add(byte);
    }
    be_block_keys::RecoverySecret::from_bytes(bytes)
}

fn image(name: &str, bytes: usize, seed: u64) -> ImageContent {
    ImageContent::new(
        ImageHeader {
            source_name: name.into(),
            media_type: "image/png".into(),
            width: 1920,
            height: 1080,
            failure: None,
            thumbhash: None,
        },
        pseudorandom(bytes, seed),
    )
}

fn pseudorandom(length: usize, seed: u64) -> Vec<u8> {
    let mut state = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
    (0..length)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            u8::try_from((state >> 33) & 0xff).unwrap()
        })
        .collect()
}

struct Link {
    targets: Vec<Uuid>,
}

impl BlockContent for Link {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x11_11);

    fn encode(&self) -> Vec<u8> {
        self.targets
            .iter()
            .flat_map(|target| target.into_bytes())
            .collect()
    }

    fn decode(bytes: &[u8]) -> Result<Self, ContentError> {
        if !bytes.len().is_multiple_of(16) {
            return Err(ContentError::Malformed("link list"));
        }
        Ok(Self {
            targets: bytes
                .as_chunks::<16>()
                .0
                .iter()
                .map(|chunk| Uuid::from_slice(chunk).expect("sixteen bytes"))
                .collect(),
        })
    }

    fn references(&self) -> Vec<Uuid> {
        self.targets.clone()
    }
}

/// Refs for one account in one organization, kept in `db`: scoping `db` again reads them back as a
/// device does when it starts.
fn scoped(db: &refs::RefsDb) -> refs::Refs {
    db.scope(refs::Scope {
        server: "http://server".to_owned(),
        account: Uuid::from_u128(1),
        organization: Uuid::from_u128(2),
    })
    .unwrap()
}

/// A block's summary as the server would tell it, placed under `parent` as of `changed`.
fn placed(id: u128, parent: BlockParent, changed: u64) -> be_block_protocol::BlockSummary {
    be_block_protocol::BlockSummary {
        id: Uuid::from_u128(id),
        content_type: Uuid::from_u128(0x11_11),
        access: be_block_graph::Access::Manage,
        author: None,
        parent: Some(parent),
        head: None,
        metadata: Vec::new(),
        version: None,
        key: None,
        scope: None,
        changed,
        entry: None,
        held: false,
        opens_inside: false,
        sets_access: false,
        size: 0,
        tree_size: 0,
        owner_only: false,
    }
}

/// The ids a listing answers offline.
fn offline_ids(refs: &refs::Refs, listing: be_block_protocol::Listing) -> Option<Vec<Uuid>> {
    refs.cached_listing(listing)
        .map(|blocks| blocks.iter().map(|summary| summary.id).collect())
}

/// The build of a block's type the test peers run.
const BUILD: be_block_store::Hash = be_block_store::Hash::from_bytes([3; 32]);

/// Two notes `peer` saved, each in a workspace of its own: one whose readers are many, say, and
/// one whose readers are few. A server that holds both could serve what was sealed in the open
/// one as the secret one's.
async fn open_and_secret(peer: &Peer<MemoryStore>) -> (Uuid, Uuid) {
    let mut notes = Vec::new();
    for name in ["Shared", "Private"] {
        let note = peer
            .create::<be_type_text::TextContent>(BlockParent::Block(workspace(peer, name).await))
            .await
            .unwrap();
        peer.save(note, &be_type_text::TextBlock::of(name), None)
            .await
            .unwrap();
        notes.push(note);
    }
    (notes[0], notes[1])
}

/// Puts `escrow` in the organization's chain straight into the server's database, as a server
/// that makes one up would.
fn put_escrow(harness: &Harness, organization: Uuid, escrow: &be_block_protocol::EscrowKey) {
    harness
        .database()
        .execute(
            "INSERT INTO keys
                (organization_id, id, block_id, kind, epoch, created, public, maker, signature, head)
             VALUES (?1, ?2, ?1, 'escrow', ?3, 0, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                organization.to_string(),
                escrow.id.to_string(),
                i64::try_from(escrow.epoch).unwrap(),
                escrow.public.to_vec(),
                escrow.maker.to_string(),
                escrow.signature,
                escrow.head.to_vec()
            ],
        )
        .unwrap();
}

/// How many wraps the server holds of `key` sealed to `account`.
fn sealed_to(harness: &Harness, key: Uuid, account: Uuid) -> i64 {
    harness
        .database()
        .query_row(
            "SELECT COUNT(*) FROM wrapped_keys WHERE key_id = ?1 AND wrapped_by = ?2",
            [key.to_string(), format!("account:{account}")],
            |row| row.get(0),
        )
        .unwrap()
}

/// The newest entry of an access log, taken out of the server's database as a server that
/// withholds it would (`withhold_newest`), to put back with `Withheld::serve`.
struct Withheld {
    organization: Uuid,
    block: Option<Uuid>,
    position: i64,
    entry: Vec<u8>,
}

/// Takes the newest entry of `block`'s log, or with none the organization's, out of the
/// server's database: what it serves stops before it.
fn withhold_newest(harness: &Harness, organization: Uuid, block: Option<Uuid>) -> Withheld {
    let database = harness.database();
    let (position, entry): (i64, Vec<u8>) = match block {
        Some(block) => database.query_row(
            "SELECT position, entry FROM access_log WHERE organization_id = ?1 AND block_id = ?2
             ORDER BY position DESC LIMIT 1",
            [organization.to_string(), block.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ),
        None => database.query_row(
            "SELECT position, entry FROM organization_log WHERE organization_id = ?1
             ORDER BY position DESC LIMIT 1",
            [organization.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ),
    }
    .unwrap();
    match block {
        Some(block) => database.execute(
            "DELETE FROM access_log WHERE organization_id = ?1 AND block_id = ?2 AND position = ?3",
            rusqlite::params![organization.to_string(), block.to_string(), position],
        ),
        None => database.execute(
            "DELETE FROM organization_log WHERE organization_id = ?1 AND position = ?2",
            rusqlite::params![organization.to_string(), position],
        ),
    }
    .unwrap();
    Withheld {
        organization,
        block,
        position,
        entry,
    }
}

impl Withheld {
    /// The entry as an access log entry, to name its hash.
    fn hash(&self) -> be_block_keys::EntryHash {
        be_block_keys::Entry::decode(&self.entry).unwrap().hash()
    }

    /// Puts the entry back: the server serves the whole log again.
    fn serve(self, harness: &Harness) {
        let database = harness.database();
        match self.block {
            Some(block) => database.execute(
                "INSERT INTO access_log (organization_id, block_id, position, entry)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    self.organization.to_string(),
                    block.to_string(),
                    self.position,
                    self.entry
                ],
            ),
            None => database.execute(
                "INSERT INTO organization_log (organization_id, position, entry)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![self.organization.to_string(), self.position, self.entry],
            ),
        }
        .unwrap();
    }
}

/// Makes `accepter` and `maker` each other's verified contacts, by a contact link `maker` makes.
async fn befriend(maker: &Peer<MemoryStore>, accepter: &Peer<MemoryStore>) {
    let code = crate::contacts::make_link(
        &maker.connection(),
        maker.keys().identity(),
        &crate::contacts::card(
            maker.account(),
            maker.keys().identity(),
            "Maker",
            "maker@example.com",
        ),
        None,
    )
    .await
    .unwrap();
    crate::contacts::accept_link(
        &accepter.connection(),
        accepter.keys().identity(),
        &crate::contacts::card(
            accepter.account(),
            accepter.keys().identity(),
            "Accepter",
            "accepter@example.com",
        ),
        &code,
        crate::contacts::Expected::Contact,
    )
    .await
    .unwrap();
    maker.finish_links().await.unwrap();
    maker.refresh_contacts().await.unwrap();
    accepter.refresh_contacts().await.unwrap();
}

/// A team where `bob` viewed and was revoked, and the epoch after: `(admin, carol, bob, team,
/// note, head, old)`, `old` the scope epoch Bob still holds.
struct Revoked {
    admin: Peer<MemoryStore>,
    carol: Peer<MemoryStore>,
    bob: Peer<MemoryStore>,
    team: Uuid,
    note: Uuid,
    head: Option<be_block_commit::CommitId>,
    old: Uuid,
}
async fn revoked(harness: &Harness) -> Revoked {
    revoked_from(harness, Access::View).await
}
/// `revoked`, with Bob granted `access` before his revocation.
async fn revoked_from(harness: &Harness, access: Access) -> Revoked {
    let admin = harness.owner("admin@example.com").await;
    let carol = harness.member(&admin, "carol@example.com").await;
    let bob = harness.member(&admin, "bob@example.com").await;
    let team = workspace(&admin, "Team").await;
    let note = admin
        .create::<TextContent>(BlockParent::Block(team))
        .await
        .unwrap();
    let head = admin
        .save(note, &TextBlock::of("before"), None)
        .await
        .unwrap()
        .published();
    admin
        .grant(team, carol.account(), Access::Edit)
        .await
        .unwrap();
    admin.grant(team, bob.account(), access).await.unwrap();
    admin.tend().await.unwrap();
    bob.refresh_keys(None).await.unwrap();
    let old = bob.keys().scope_key(team).unwrap();
    admin
        .grant(team, bob.account(), Access::None)
        .await
        .unwrap();
    admin.tend().await.unwrap();
    Revoked {
        admin,
        carol,
        bob,
        team,
        note,
        head,
        old,
    }
}
/// Bob, with the server, makes up a data key of the note naming `named`, wraps it under the
/// epoch he still holds and has it listed as the note's newest.
fn plant(harness: &Harness, revoked: &Revoked, named: Option<EntryHash>) -> Uuid {
    let made_up = Uuid::new_v4();
    let carried = CarriedKey {
        key: [5; 32],
        label: planted(made_up, "data", 90),
        block: revoked.note.into_bytes(),
        root: None,
        heads: Heads {
            block: named,
            organization: None,
        },
    };
    let sealed = revoked
        .bob
        .keys()
        .ring()
        .seal(revoked.old, &carried.encode())
        .unwrap();
    let organization = revoked.admin.organization();
    let database = harness.database();
    database
        .execute(
            "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
             VALUES (?1, ?2, ?3, 'data', 90, 0)",
            rusqlite::params![
                organization.to_string(),
                made_up.to_string(),
                revoked.note.to_string()
            ],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                organization.to_string(),
                made_up.to_string(),
                revoked.old.to_string(),
                revoked.bob.account().to_string(),
                sealed
            ],
        )
        .unwrap();
    made_up
}
/// Saves as `device`, which opened `made_up`, and checks the save gave the team a new epoch
/// first and wrote under a key Bob never gets.
async fn saves_past(revoked: &Revoked, device: &Peer<MemoryStore>, made_up: Uuid) {
    assert!(
        device.keys().trusted(made_up),
        "the made-up key did not open, so the test shows nothing"
    );
    let epochs = device.keys().keys_of(revoked.team, KeyKind::Scope).len();
    device
        .save(revoked.note, &TextBlock::of("after"), revoked.head)
        .await
        .unwrap();
    assert_eq!(
        device.keys().keys_of(revoked.team, KeyKind::Scope).len(),
        epochs + 1,
        "the save wrote under the made-up key"
    );
    let written = device
        .keys()
        .keys_of(revoked.note, KeyKind::Data)
        .last()
        .unwrap()
        .id;
    assert_ne!(written, made_up);
    revoked.bob.refresh_keys(None).await.unwrap();
    assert!(!revoked.bob.keys().has(written));
}
/// The label a key a test plants carries: its id, `kind` as the server's database spells it,
/// and `epoch`, matching the row the test puts beside it.
pub(crate) fn planted(id: Uuid, kind: &str, epoch: u64) -> be_block_keys::Label {
    be_block_keys::Label {
        id: id.into_bytes(),
        kind: match kind {
            "data" => be_block_keys::KeyRole::Data,
            "scope" => be_block_keys::KeyRole::Scope,
            "escrow" => be_block_keys::KeyRole::Escrow,
            other => panic!("no key kind {other}"),
        },
        epoch,
    }
}

/// Puts a key of `block` with id `key` and `kind`, sealed by Bob as `sealed`, in the server's
/// database, wrapped by `wrapped_by` (an account or a key, as the server writes it).
fn plant_sealed(
    harness: &Harness,
    revoked: &Revoked,
    key: Uuid,
    block: Uuid,
    kind: &str,
    wrapped_by: String,
    sealed: Vec<u8>,
) {
    let organization = revoked.admin.organization();
    let database = harness.database();
    database
        .execute(
            "INSERT INTO keys (organization_id, id, block_id, kind, epoch, created)
             VALUES (?1, ?2, ?3, ?4, 90, 0)",
            rusqlite::params![
                organization.to_string(),
                key.to_string(),
                block.to_string(),
                kind
            ],
        )
        .unwrap();
    database
        .execute(
            "INSERT INTO wrapped_keys (organization_id, key_id, wrapped_by, sender, sealed)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                organization.to_string(),
                key.to_string(),
                wrapped_by,
                revoked.bob.account().to_string(),
                sealed
            ],
        )
        .unwrap();
}
/// The head of the team's log, past Bob's revocation, as `device` checks it.
async fn team_head(revoked: &Revoked, device: &Peer<MemoryStore>) -> EntryHash {
    device.checked_access(revoked.team).await.unwrap().1.head
}

/// Hands `block`'s order the `counter`-th submission of one origin, online, and waits for the
/// server to number it.
async fn submitted(peer: &Peer<MemoryStore>, block: Uuid, counter: u64) {
    let submission = be_block_session::Submission {
        id: be_block_session::OpId { origin: 1, counter },
        base: counter - 1,
        barrier: false,
        payload: format!("edit {counter}").into_bytes(),
    };
    let Ok(peer::Submitted::Server(answer)) = peer
        .submit(block, BUILD, submission, true, Vec::new())
        .await
    else {
        panic!("an online submission goes to the server");
    };
    answer.await.unwrap();
}

/// A seal of a block's session: `content` on `expected`, holding the order through
/// `through`.
fn sealing<C: BlockContent>(
    content: &C,
    expected: be_block_commit::CommitId,
    through: u64,
) -> Write<'_> {
    Write::encoded(C::CONTENT_TYPE, content.parts(), Some(expected)).seals_journal(Some(
        SealsJournal {
            tag: BUILD,
            run: 0,
            through,
            state: format!("state through {through}").into_bytes(),
        },
    ))
}

/// The server's journal of `block` as `peer` reads it: its base's commit and sequence, and the
/// sequence of each entry after it; none when it holds no entries.
async fn server_journal(
    peer: &Peer<MemoryStore>,
    block: Uuid,
) -> Option<(Option<be_block_commit::CommitId>, u64, Vec<u64>)> {
    let read = peer.read_contents(&[(block, None)]).await.unwrap();
    let journal = read.into_iter().next().and_then(|read| read.journal)?;
    Some((
        journal.base.commit,
        journal.base.sequence,
        journal.entries.iter().map(|entry| entry.sequence).collect(),
    ))
}
