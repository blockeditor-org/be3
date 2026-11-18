use std::{path::PathBuf, time::Duration};

use be_block_commit::{Commit, CommitId, CommitStore};
use be_block_graph::BlockParent;
use be_block_protocol::{
    Authenticated, BlockSummary, ChangeCursor, ClientId, DeviceLoginAnswer, DeviceLoginStep, Done,
    Error, ErrorCode, Event, FromDone, FromPage, HistoryEntry, Hold, KeyKind, KeyRow, KeySet,
    Listing, ObjectLength, Op, OrganizationRole, PairingMessage, PutKeys, Query, SessionEvent,
    SessionSend, SessionState, Target, Topic, UploadUrl, WrappedBy, WrappedKey, WriteError, client,
};
use be_block_session::{Resume, resume};
use be_block_store::{ChunkerConfig, ContentKey, Hash, Keyring, MemoryStore, ObjectStore, Vault};
use be_wasm_ops::net::{Caller, Peer, Stream};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::oneshot;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message as WsMessage,
};
use uuid::Uuid;

use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
mod a_backup_restores_into_a_server_that_serves_the_same_blocks;
mod a_block_agent_is_deleted_with_its_grant;
mod a_block_agent_is_refused_while_its_block_is_in_the_trash;
mod a_block_agent_never_becomes_an_administrator;
mod a_block_agent_reaches_its_block_and_nothing_else;
mod a_block_agent_reaches_no_further_than_its_creator;
mod a_block_agents_long_poll_session_is_replaced_too;
mod a_block_agents_new_connection_replaces_the_open_one;
mod a_block_holds_a_bounded_number_of_agents_of_its_own;
mod a_block_in_unfiled_stays_live_without_a_listing_until_it_is_deleted;
mod a_block_made_with_no_key_in_its_write_is_refused;
mod a_block_shared_from_a_hidden_folder_is_listed_as_shared;
mod a_blocks_references_are_listed_and_announced_together;
mod a_browser_preflight_to_the_s3_endpoint_is_answered;
mod a_browser_sign_in_waits_on_polls_and_holds_no_connection;
mod a_bunny_storage_zone_holds_a_backup;
mod a_burst_of_publishes_reaches_a_follower_as_a_few_tree_advances;
mod a_change_a_follower_read_before_it_was_told_leaves_the_next_one_told;
mod a_change_made_while_a_follow_is_read_is_read_or_told;
mod a_changed_provider_email_held_by_another_account_is_not_taken;
mod a_client_speaks_the_protocol_over_http_posts;
mod a_closed_server_also_refuses_browser_registration_and_login;
mod a_closed_server_refuses_new_accounts_and_sign_ins_but_not_tokens;
mod a_code_from_another_provider_or_sent_to_its_callback_is_refused;
mod a_code_is_refused_after_five_wrong_tries_once_expired_and_once_replaced;
mod a_code_signs_in_only_the_browser_that_asked_for_it;
mod a_codes_link_opened_in_another_browser_shows_the_code_and_signs_nothing_in;
mod a_commit_from_the_far_future_is_refused;
mod a_connection_follows_a_bounded_number_of_entry_points;
mod a_connection_hears_only_of_the_trees_it_follows;
mod a_connection_holds_one_events_stream_and_a_bounded_number_of_sessions;
mod a_contact_card_the_server_swaps_does_not_open;
mod a_device_registers_only_an_endpoint_the_server_would_push_to;
mod a_download_does_not_wait_behind_another_organizations_store_call;
mod a_follower_of_a_shared_subtree_hears_a_block_leave_it_within_one_tree;
mod a_grant_or_a_key_for_someone_not_a_contact_is_refused;
mod a_grant_that_changes_nothing_writes_nothing;
mod a_graph_change_reaches_only_the_members_who_may_see_it;
mod a_greeting_that_signs_nothing_in_is_closed_before_any_call;
mod a_head_moves_only_to_a_commit_that_descends_from_it;
mod a_holder_that_loses_edit_access_hands_the_lease_on;
mod a_key_change_past_the_bound_lets_the_oldest_go_but_an_accepted_card_is_refused;
mod a_kind_open_inside_passes_from_one_viewer_inside_to_another;
mod a_link_accepted_then_unmade_as_a_contact_leaves_no_link_or_card;
mod a_link_or_a_proof_begins_only_with_the_settings_pages_check;
mod a_link_or_a_proof_needs_a_sign_in_at_the_provider_just_now;
mod a_listing_pages_in_id_order_without_skipping_a_block_that_stays;
mod a_login_link_for_a_new_email_makes_its_account;
mod a_login_link_signs_in_once_and_ends_other_sessions;
mod a_login_link_to_an_account_keeps_no_email_it_did_not_check;
mod a_logout_naming_the_devices_endpoint_forgets_it_in_every_organization;
mod a_member_makes_contact_links_but_only_an_administrator_invites;
mod a_member_removes_only_the_agents_it_made_while_it_may;
mod a_member_who_loses_view_of_a_trashed_block_leaves_its_session;
mod a_member_who_loses_view_when_a_block_is_trashed_is_told_it_is_gone;
mod a_member_who_loses_view_when_a_block_moves_somewhere_hidden_is_told_it_is_gone;
mod a_message_arriving_slowly_is_not_taken_for_silence;
mod a_move_missing_one_blocks_wrap_under_its_new_scope_is_refused_whole;
mod a_move_never_takes_a_workspace_or_more_access_without_manage;
mod a_move_past_the_new_payers_quota_is_refused;
mod a_mute_set_on_one_device_is_announced_to_the_others;
mod a_new_recovery_key_reseals_every_key_sealed_to_the_account;
mod a_notice_under_a_week_old_refuses_deletion_and_one_older_deletes;
mod a_notification_sent_again_with_the_keys_it_told_pushes_nothing;
mod a_page_that_reads_on_through_another_trees_change_stops_at_its_own_next_block;
mod a_part_held_until_committed_lasts_until_a_commit_takes_it_or_its_grace_passes;
mod a_passkey_assertion_counts_once;
mod a_passkey_for_an_old_origin_is_no_way_in;
mod a_passkey_for_another_rp_id_or_origin_is_refused;
mod a_passkey_signs_in_with_no_email_typed;
mod a_pending_grant_is_applied_when_its_link_is_accepted;
mod a_provider_email_matching_an_account_links_only_after_the_emailed_code;
mod a_provider_is_called_only_so_often_whatever_strangers_send;
mod a_provider_sign_in_with_no_account_waits_for_an_invite_code;
mod a_provider_the_server_no_longer_has_is_no_way_in;
mod a_provider_token_with_the_wrong_audience_or_a_strangers_signature_is_refused;
mod a_publish_adding_a_reference_the_block_already_holds_moves_it;
mod a_publish_pushes_the_new_storage_usage_to_its_payer;
mod a_publish_with_no_commit_changes_the_metadata_and_leaves_the_head;
mod a_push_hosts_cap_is_keyed_by_its_allowlist_entry_and_statuses_are_judged;
mod a_push_reaches_members_who_are_not_connected_and_want_it;
mod a_push_service_refusing_the_server_or_redirecting_does_not_cost_the_right_endpoints;
mod a_push_service_that_never_answers_delays_only_its_own_account;
mod a_range_get_reads_part_of_an_s3_object;
mod a_refused_call_keeps_the_cached_graph;
mod a_refused_write_demotes_and_disconnects_nobody;
mod a_relay_to_a_client_that_left_the_session_is_dropped_without_an_error;
mod a_remembered_account_approves_a_manually_entered_code_and_can_switch_accounts;
mod a_removed_agents_blocks_stay_charged_to_its_creator;
mod a_reply_before_a_change_and_the_change_after_it_keep_their_order_across_a_restart;
mod a_reserved_object_counts_only_once_its_bytes_arrive;
mod a_rotation_gives_every_block_below_it_a_new_key_or_nothing;
mod a_rotation_missing_one_blocks_key_is_refused_whole;
mod a_seal_by_the_old_build_after_a_new_build_started_over_trims_nothing;
mod a_search_index_is_capped_by_the_bytes_its_commit_holds;
mod a_search_index_takes_exactly_the_access_of_the_block_it_indexes;
mod a_server_without_a_portal_answers_its_pages_as_json;
mod a_session_hands_the_lease_over_without_a_merge;
mod a_session_started_again_counts_its_membership_past_the_one_before;
mod a_settings_block_changes_only_with_manage_on_its_parent;
mod a_settings_kind_opens_to_anyone_who_views_a_block_inside;
mod a_slow_store_call_does_not_delay_another_connections_ping;
mod a_stale_proof_and_another_sessions_revocation_without_one_are_refused;
mod a_stale_publish_is_rejected_with_the_current_head;
mod a_stale_seal_from_an_earlier_run_publishes_and_trims_nothing;
mod a_stored_key_is_told_only_to_who_may_open_it;
mod a_stranger_cannot_read_a_block_in_an_organization_it_is_not_in;
mod a_submission_from_a_build_the_session_does_not_speak_is_refused;
mod a_summary_names_no_references_and_a_referenced_block_still_reads_as_known;
mod a_trashed_block_reaches_its_reader_as_detached;
mod a_trim_by_another_build_marks_the_journal_lost_until_the_next_commit;
mod a_trim_from_other_than_the_holder_is_not_kept;
mod a_trim_past_the_orders_last_entry_is_not_kept;
mod a_type_with_a_short_retention_keeps_only_its_recent_states;
mod a_viewer_neither_holds_nor_claims_the_lease_and_its_presence_still_reaches_the_others;
mod a_viewer_reads_the_names_of_who_may_view_a_block_and_nothing_more;
mod a_write_let_go_of_midway_keeps_nothing_and_leaves_no_transaction_open;
mod a_write_whose_second_op_is_refused_keeps_nothing_of_its_first;
mod accepting_either_kind_of_link_makes_two_contacts;
mod accounts_stranded_by_a_removed_provider_are_counted;
mod adding_a_provider_or_moving_its_email_tells_the_account;
mod adding_or_removing_a_passkey_needs_a_fresh_proof_and_keeps_the_last_way_in;
mod an_account_adds_ways_to_sign_in_only_so_often_a_day_and_always_removes_them;
mod an_account_creates_a_bounded_number_of_agents;
mod an_account_holds_a_bounded_number_of_passkeys;
mod an_account_quota_refuses_what_would_pass_it_but_not_a_big_file_within;
mod an_administrator_creates_lists_and_revokes_an_invitation;
mod an_administrator_sees_a_members_unfiled_as_their_my_files;
mod an_agent_signs_in_with_its_token_until_it_is_removed;
mod an_agent_stores_against_its_creators_quota;
mod an_agents_creator_removes_it_as_a_member_and_key_work_is_told;
mod an_app_token_left_unused_expires_and_one_in_use_is_renewed;
mod an_editor_changes_the_prefixes_and_tags_but_no_other_kind;
mod an_editor_sees_only_the_blocks_it_made_or_was_given;
mod an_email_change_is_told_on_the_accounts_devices_and_listed;
mod an_email_with_no_account_is_answered_the_same_and_sent_nothing;
mod an_endpoint_its_push_service_says_is_gone_is_dropped;
mod an_endpoint_on_a_host_taken_off_the_list_is_dropped_at_send;
mod an_endpoint_that_keeps_failing_for_a_day_is_dropped;
mod an_expired_or_revoked_invitation_is_refused;
mod an_http_answer_leaves_what_would_overflow_it_for_the_next_post;
mod an_invitation_ends_when_its_inviter_stops_administering;
mod an_invitation_is_read_before_sign_in_and_listed_as_it_goes;
mod an_invitation_joins_any_account_as_an_editor_once;
mod an_invited_account_joins_the_organization_and_a_logout_ends_the_session;
mod an_object_asked_for_without_its_length_is_answered_only_to_one_who_holds_it;
mod an_object_uploads_and_downloads_with_presigned_s3_urls;
mod an_old_nonce_a_reused_state_or_another_browser_is_refused;
mod an_older_backup_renamed_as_the_newest_does_not_restore;
mod an_s3_upload_that_is_not_its_key_stores_nothing;
mod an_unknown_emails_budget_runs_out_as_a_known_ones_does;
mod an_unverified_provider_email_links_nothing;
mod an_upload_is_used_once_and_what_a_block_holds_is_not_asked_for_again;
mod another_account_cannot_reach_a_block_by_its_id_or_its_objects_by_their_hashes;
mod another_build_is_greeted_and_turned_away;
mod approving_a_device_code_needs_a_fresh_proof;
mod block_sizes_add_up_their_subtrees_and_follow_moves;
mod blocks_publish_and_read_back_from_a_bucket;
mod blocks_publish_and_read_back_through_the_server;
mod browser_forms_enforce_origin_policy_and_deny_sign_in;
mod browser_pages_escape_codes_and_keep_available_actions;
mod changed_since_does_not_name_what_only_its_author_may_read;
mod changed_since_pages_what_changed_in_a_tree_and_names_what_left_it;
mod clients_not_signed_in_hold_only_their_share;
mod closing_a_kind_rotates_it_for_who_read_it_inside;
mod contacts_are_made_by_a_link_and_finished_by_its_owner;
mod email_change_links_are_bounded_and_expire;
mod email_changes_wait_for_verification_and_notify_the_old_address;
mod every_call_states_who_may_make_it;
mod every_graph_change_advances_a_blocks_version;
mod every_key_is_read_a_page_at_a_time_without_one_skipped;
mod every_member_connection_hears_how_the_graph_changes;
mod fcm_sends_a_data_message_with_the_service_accounts_token;
mod following_key_work_answers_its_current_value;
mod history_records_each_commits_parents;
mod key_rows_are_served_only_to_who_may_see_their_block;
mod linking_or_proving_with_a_provider_needs_the_signed_in_account;
mod losing_every_block_inside_rotates_a_kind_open_inside;
mod members_manage_only_the_agents_they_created;
mod metadata_travels_with_the_content_once_a_block_has_any;
mod moving_a_block_drops_the_wraps_of_its_old_scope;
mod moving_a_block_out_of_what_is_shared_rotates_its_keys;
mod nobody_makes_or_writes_to_another_accounts_unfiled;
mod objects_are_served_only_from_blocks_the_reader_can_see;
mod old_database_backups_thin_out_by_age;
mod one_address_holds_only_so_many_connections_and_http_sessions;
mod one_connection_reads_two_organizations_without_reconnecting;
mod one_shot_calls_only_sign_in_and_are_rate_limited;
mod only_a_manager_lets_go_of_a_blocks_settings;
mod only_a_manager_shares_and_a_lower_grant_reduces_nothing;
mod only_members_who_may_view_and_did_not_mute_a_block_are_notified;
mod pairing_messages_reach_only_the_same_accounts_other_connections;
mod proxy_account_authentication_never_discloses_the_token;
mod publishing_prunes_the_commit_it_replaces_but_keeps_bookmarks_and_merge_parents;
mod read_summaries_leaves_out_what_the_caller_may_not_know_of;
mod recovering_by_code_signs_out_every_other_session;
mod recovering_removes_the_passkeys_and_agents_an_intruder_added;
mod references_keep_the_order_their_block_lists_them_in;
mod registering_by_code_makes_the_account_once_the_code_is_entered;
mod registering_joins_the_one_personal_organization_as_an_editor;
mod registration_codes_are_capped_per_invite_and_overall;
mod registration_needs_an_invite_code_spent_when_the_account_is_made;
mod relayed_session_traffic_passes_through_the_server_sealed;
mod removing_a_contact_revokes_what_the_two_shared;
mod removing_the_email_it_signs_in_with_clears_the_accounts_email;
mod requests_sent_before_a_close_are_still_carried_out;
mod revoking_a_session_announces_only_a_change;
mod revoking_an_http_session_removes_its_access;
mod rotating_an_agents_token_revokes_every_older_one;
mod senders_are_capped_overall_and_per_push_host;
mod sessions_are_scoped_to_the_account_and_form_tokens_to_the_browser;
mod shared_chunks_survive_until_the_last_commit_releases_them;
mod signing_keys_are_served_only_for_accounts_the_caller_knows;
mod strangers_never_stop_the_owner_getting_a_code_from_an_address_they_signed_in_from;
mod the_access_list_leaves_administrators_to_the_organization_line;
mod the_browser_authorization_requires_the_private_secret_and_works_once;
mod the_portal_serves_its_assets_immutably_beside_a_hydrating_page;
mod the_portals_cookies_are_host_cookies_over_https;
mod the_providers_file_is_checked_and_keeps_its_secrets;
mod the_server_orders_sealed_submissions_and_a_seal_trims_them;
mod the_trees_owner_pays_for_what_anyone_makes_in_it;
mod told_keys_past_the_newest_kept_or_of_a_removed_member_are_news;
mod unsigned_or_expired_s3_requests_are_refused;
mod uploads_no_block_takes_are_collected_after_their_grace;
mod what_one_account_keeps_on_the_server_is_bounded;

const CONTENT: Uuid = Uuid::from_u128(0x7465_7874);
const KEY: Uuid = Uuid::from_u128(0x006b_6579);

const BUILD: &str = "a test build";

/// Opens the server's metadata database in `directory` beside a running server, with the busy
/// timeout every server connection has, so it waits on the server's writes rather than failing.
fn metadata_database(directory: &std::path::Path) -> rusqlite::Connection {
    be_block_sqlite_schema::open(directory.join("metadata.sqlite")).unwrap()
}

struct Harness {
    mail: Arc<CapturedMail>,
    server: Running,
}

impl std::ops::Deref for Harness {
    type Target = Running;

    fn deref(&self) -> &Running {
        &self.server
    }
}

impl Harness {
    async fn start() -> Self {
        Self::configured(ServerConfig::OPEN).await
    }

    async fn at(directory: PathBuf) -> Self {
        Self::serve(directory, ServerConfig::OPEN).await
    }

    async fn configured(config: ServerConfig) -> Self {
        Self::serve(
            std::env::temp_dir().join(format!("be-server-test-{}", Uuid::new_v4())),
            config,
        )
        .await
    }

    async fn serve(directory: PathBuf, config: ServerConfig) -> Self {
        be_lib_build_id::install(BUILD);
        let mail = Arc::new(CapturedMail::default());
        let server = Running::with_mail(directory, config, mail.clone()).await;
        Self { mail, server }
    }

    fn socket(&self) -> String {
        format!("ws://{}", self.server.address)
    }

    /// The references the server keeps for `block`, in its order, read from its database:
    /// no summary names them (a reader takes them from the block's content).
    async fn kept_references(&self, organization: Uuid, block: Uuid) -> Vec<Uuid> {
        ServerStore::open(&self.directory)
            .expect("the store opens beside the server")
            .with_graph(organization, move |_, graph, _| Ok(graph.references(block)))
            .await
            .expect("the organization's graph reads")
    }

    /// A connection its greeting signs in with `token`, or the server's refusal, the connection
    /// then closed.
    async fn sign_in(&self, token: &str) -> Result<TestClient, Error> {
        let mut client = TestClient::connect(&self.socket(), Some(token)).await?;
        client.directory = Some(self.directory.clone());
        Ok(client)
    }

    async fn signed_in(&self, token: &str) -> TestClient {
        self.sign_in(token)
            .await
            .unwrap_or_else(|error| panic!("signing in failed: {error:?}"))
    }

    /// A socket that greeted the server as `build`, with no token.
    async fn greeted(&self, build: &str) -> Socket {
        greet(&self.socket(), build, None).await
    }

    /// A new account's connection, the account's recovery key set to its test keys.
    async fn member(&self, email: &str) -> TestClient {
        let (_, token) = ServerStore::open(&self.directory)
            .unwrap()
            .register(email, email)
            .await
            .unwrap();
        let mut client = self.signed_in(&token).await;
        let identity = test_secret(client.id()).identity();
        let response = client
            .call(|peer| {
                peer.write::<()>(be_block_protocol::Op::SetRecoveryKey {
                    public: identity.public.0,
                    signing: identity.signing.0,
                    sealed: Vec::new(),
                    changes: Vec::new(),
                    retirements: Vec::new(),
                })
            })
            .await;
        assert!(
            response.is_ok(),
            "setting the recovery key failed: {response:?}"
        );
        client
    }

    async fn second_connection(&self, token: &str, organization: Uuid) -> TestClient {
        let mut client = self.signed_in(token).await;
        client.open(organization).await;
        client
    }

    async fn restart(mut self) -> Self {
        self.server.restart().await;
        self
    }

    async fn stop(self) {
        self.server.stop().await;
    }
}

/// The build of a block's type the test clients run.
const TYPE_BUILD: be_block_store::Hash = be_block_store::Hash::from_bytes([3; 32]);

#[derive(Clone, Debug, PartialEq)]
enum Notice {
    TreeAdvanced {
        entry: Uuid,
        sequence: u64,
    },
    Roots,
    Membership {
        block: Uuid,
        entry: be_block_protocol::MembershipEntry,
    },
    Sealed {
        block: Uuid,
        commit: CommitId,
        through: u64,
    },
    Relayed {
        block: Uuid,
        from: ClientId,
        payload: Vec<u8>,
        received: i64,
    },
    Paired {
        from: ClientId,
        payload: Vec<u8>,
    },
    MailboxChanged,
    Sequenced {
        block: Uuid,
        entry: be_block_protocol::JournalEntry,
    },
    /// Another topic the connection follows changed (`event.topic-changed`).
    Changed {
        organization: Option<Uuid>,
        topic: Topic,
    },
}

impl Notice {
    fn of(event: Event) -> Self {
        match event {
            Event::TreeAdvanced {
                entry, sequence, ..
            } => Notice::TreeAdvanced { entry, sequence },
            Event::TopicChanged {
                topic: Topic::Roots,
                ..
            } => Notice::Roots,
            Event::TopicChanged {
                topic: Topic::Mailbox,
                ..
            } => Notice::MailboxChanged,
            Event::TopicChanged {
                organization,
                topic,
            } => Notice::Changed {
                organization,
                topic,
            },
            Event::Paired(PairingMessage { from, payload }) => Notice::Paired { from, payload },
            Event::Session { block, event, .. } => match event {
                SessionEvent::Membership(entry) => Notice::Membership { block, entry },
                SessionEvent::Sealed {
                    commit, through, ..
                } => Notice::Sealed {
                    block,
                    commit,
                    through,
                },
                SessionEvent::Relayed {
                    from,
                    payload,
                    received,
                } => Notice::Relayed {
                    block,
                    from,
                    payload,
                    received,
                },
                SessionEvent::Sequenced(entry) => Notice::Sequenced { block, entry },
            },
        }
    }
}

/// A test client's peer and the organization it works in: what makes its calls. Each read, op
/// and target names the organization its rule asks for (`client::query_rule`, ...): none for
/// the account's own, else this one.
#[derive(Clone)]
struct At {
    peer: Peer,
    organization: Option<Uuid>,
}

impl Caller for At {
    fn start(
        &self,
        parent: Option<be_wasm_ops::OpId>,
        method: be_wasm_ops::MethodId,
        args: Vec<u8>,
    ) -> be_wasm_ops::OpId {
        self.peer.start(parent, method, args)
    }

    fn poll_op(
        &self,
        op: be_wasm_ops::OpId,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<be_wasm_ops::net::Arrival> {
        self.peer.poll_op(op, context)
    }

    fn credit(&self, op: be_wasm_ops::OpId, items: u32, bytes: u64) {
        self.peer.credit(op, items, bytes);
    }

    fn cancel(&self, op: be_wasm_ops::OpId) {
        self.peer.cancel(op);
    }

    fn forget(&self, op: be_wasm_ops::OpId) {
        self.peer.forget(op);
    }

    fn request_cancel(&self, op: be_wasm_ops::OpId) {
        self.peer.request_cancel(op);
    }
}

type Answered<T> = Result<Result<T, Error>, be_wasm_ops::OpError>;

impl At {
    fn organization_for(&self, rule: crate::client::Rule) -> Option<Uuid> {
        match rule {
            crate::client::Rule::OneShot | crate::client::Rule::Account => None,
            _ => self.organization,
        }
    }

    /// The organization a write of `op` names.
    fn op_organization(&self, op: &Op) -> Option<Uuid> {
        match crate::client::op_rule(op) {
            // Removing a contact names the organization whose revocations go with it.
            crate::client::Rule::Anywhere => self.organization,
            rule => self.organization_for(rule),
        }
    }

    fn read<T: FromPage + Send + 'static>(
        &self,
        query: Query,
    ) -> impl Future<Output = Answered<T>> + use<T> {
        self.ask(query, false)
    }

    fn ask<T: FromPage + Send + 'static>(
        &self,
        query: Query,
        follow: bool,
    ) -> impl Future<Output = Answered<T>> + use<T> {
        let organization = self.organization_for(crate::client::query_rule(&query));
        be_block_protocol::ask(self, organization, query, follow)
    }

    fn read_in<T: FromPage + Send + 'static>(
        &self,
        organization: Option<Uuid>,
        query: Query,
    ) -> impl Future<Output = Answered<T>> + use<T> {
        be_block_protocol::read(self, organization, query)
    }

    fn write<T: FromDone + Send + 'static>(
        &self,
        op: Op,
    ) -> impl Future<Output = Answered<T>> + use<T> {
        let organization = self.op_organization(&op);
        be_block_protocol::write(self, organization, op)
    }

    fn write_in<T: FromDone + Send + 'static>(
        &self,
        organization: Option<Uuid>,
        op: Op,
    ) -> impl Future<Output = Answered<T>> + use<T> {
        be_block_protocol::write(self, organization, op)
    }

    /// Writes `ops` as one write in the organization of the first, answered as the case `T` the
    /// first makes.
    fn write_all<T: FromDone + Send + 'static>(
        &self,
        ops: Vec<Op>,
    ) -> impl Future<Output = Answered<T>> + use<T> {
        let organization = ops.first().and_then(|op| self.op_organization(op));
        be_block_protocol::write_all(self, organization, ops)
    }

    /// Writes `ops` as one write in the organization.
    fn writes(&self, ops: Vec<Op>) -> impl Future<Output = Answered<Vec<Done>>> + use<> {
        let written = client::server::write(self, self.organization, ops);
        async move { Ok(written.await?.map_err(|refused: WriteError| refused.error)) }
    }

    fn send(&self, target: Target, payload: Vec<u8>) -> impl Future<Output = Answered<()>> + use<> {
        let organization = self.organization_for(crate::client::target_rule(&target));
        client::server::send(self, organization, target, payload)
    }

    fn unfollow(&self, topic: Topic) -> impl Future<Output = Answered<()>> + use<> {
        let organization = self.organization_for(crate::client::topic_rule(&topic));
        client::server::unfollow(self, organization, topic)
    }
}

/// A link as the server holds it for its owner (`TestClient::links`).
#[derive(Debug)]
struct OwnLink {
    id: Uuid,
    accepter: Option<Uuid>,
}

struct TestClient {
    directory: Option<PathBuf>,
    peer: At,
    /// The events stream, once the client signed in.
    heard: Option<Stream<Event>>,
    listening: bool,
    /// The membership each session answered as this client joined it.
    joined: std::collections::HashMap<Uuid, SessionState>,
    notifications: Vec<Notice>,
    token: String,
    /// This connection as the server names it (`authenticated.client`).
    client: Option<ClientId>,
    closing: Option<oneshot::Sender<()>>,
    pump: Option<tokio::task::JoinHandle<()>>,
    /// The account this client signed in as, whose keys are `test_secret`'s.
    account: Option<Uuid>,
    /// Where this client has read each entry point it follows to (`follow`).
    cursors: std::collections::HashMap<Uuid, ChangeCursor>,
}

/// The codes of the invitations tests made, by link and organization (`TestClient::invite`;
/// tests running at once reuse link ids), which accepting one signs with.
static CODES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<(Uuid, Uuid), be_block_keys::InviteCode>>,
> = std::sync::LazyLock::new(Default::default);

/// Identity secrets tests gave accounts besides `test_secret`'s (an agent's, a new phrase's), by
/// account (`remember_secret`).
static SECRETS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<Uuid, be_block_keys::RecoverySecret>>,
> = std::sync::LazyLock::new(Default::default);

/// A new organization's id and the root of its log, made by `account` with its test keys.
fn organization_root(account: Uuid) -> (Uuid, Vec<u8>) {
    let id = Uuid::new_v4();
    let root = be_block_keys::Entry::root(
        &test_secret(account),
        account.into_bytes(),
        be_block_keys::LogKind::Organization,
        id.into_bytes(),
    );
    (id, root.encode())
}

/// Makes a company organization straight through the store, as `account`, whose test keys it
/// registers first when it holds none.
async fn signed_organization(
    store: &ServerStore,
    account: Uuid,
    name: &str,
) -> Result<be_block_protocol::Organization, ServerError> {
    let identity = test_secret(account).identity();
    if store.recovery_key(account).await?.0.is_none() {
        store
            .set_recovery_key(
                account,
                identity.public.0,
                identity.signing.0,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
            .await?;
    }
    let (id, root) = organization_root(account);
    store
        .create_organization(account, id, name, root, Vec::new())
        .await
}

/// Makes `secret` the one `account` signs with in tests from now on.
fn remember_secret(account: Uuid, secret: be_block_keys::RecoverySecret) {
    SECRETS.lock().unwrap().insert(account, secret);
}

/// The identity secret a test account holds: set as its recovery key when it registers, or
/// made for it as an agent, so it signs the roots of the blocks it makes (`TestClient::root`).
fn test_secret(account: Uuid) -> be_block_keys::RecoverySecret {
    if let Some(secret) = SECRETS.lock().unwrap().get(&account) {
        return secret.clone();
    }
    let mut bytes = [0; 32];
    bytes[..16].copy_from_slice(account.as_bytes());
    bytes[16..].copy_from_slice(account.as_bytes());
    be_block_keys::RecoverySecret::from_bytes(bytes)
}

impl Drop for TestClient {
    fn drop(&mut self) {
        self.closing.take();
    }
}

impl TestClient {
    /// A connection its greeting signs in with `token`, or with the request's Authorization
    /// header when none, opening the events stream and following the account's contacts as
    /// every client does; or the server's refusal.
    async fn connect(url: &str, token: Option<&str>) -> Result<Self, Error> {
        let socket = greet(url, BUILD, token).await;
        Self::welcomed(socket).await
    }

    /// A connection signed in with `token` that neither opens the events stream nor follows
    /// anything, for one whose following would be refused.
    async fn unheard(url: &str, token: &str) -> Result<Self, Error> {
        Self::signed_in_over(greet(url, BUILD, Some(token)).await).await
    }

    /// The client over `socket`, once the server's greeting on it says who it signed in as,
    /// listening as every client does.
    async fn welcomed(socket: Socket) -> Result<Self, Error> {
        let mut client = Self::signed_in_over(socket).await?;
        client.listen().await;
        Ok(client)
    }

    async fn signed_in_over(mut socket: Socket) -> Result<Self, Error> {
        let frame = next_binary(&mut socket).await;
        let hello = frame
            .as_deref()
            .and_then(read_hello)
            .expect("the server greets");
        assert_eq!(
            hello.build, BUILD,
            "the server did not greet a client of its own build"
        );
        let Authenticated {
            account,
            token,
            client,
            ..
        } = hello
            .said::<be_block_protocol::Welcome>()
            .expect("the server's greeting says who signed in")?;
        let mut client_of = Self::over(socket);
        client_of.account = Some(account);
        client_of.token = token;
        client_of.client = Some(client);
        Ok(client_of)
    }

    fn over(socket: Socket) -> Self {
        let (sending, outbound) = tokio::sync::mpsc::unbounded_channel();
        let peer = Peer::new(be_wasm_ops::Side::Guest, move |frame| {
            let _ = sending.send(frame);
        });
        let (closing, closed) = oneshot::channel();
        let pump = tokio::spawn(pump(socket, peer.clone(), outbound, closed));
        Self {
            directory: None,
            peer: At {
                peer,
                organization: None,
            },
            heard: None,
            listening: false,
            joined: std::collections::HashMap::new(),
            notifications: Vec::new(),
            token: String::new(),
            account: None,
            client: None,
            closing: Some(closing),
            pump: Some(pump),
            cursors: std::collections::HashMap::new(),
        }
    }

    async fn ended(&mut self) {
        if let Some(pump) = self.pump.take() {
            let _ = pump.await;
        }
    }

    async fn close(mut self) {
        self.closing.take();
        if let Some(pump) = self.pump.take() {
            let _ = pump.await;
        }
    }

    async fn call<T: 'static, F: Future<Output = Answered<T>>>(
        &mut self,
        start: impl FnOnce(&At) -> F,
    ) -> Result<T, Error> {
        let answered = start(&self.peer).await.expect("the connection stayed open");
        self.collect();
        answered
    }

    async fn listen(&mut self) {
        if std::mem::replace(&mut self.listening, true) {
            return;
        }
        self.heard = Some(client::server::events(&self.peer));
        let mailbox = self
            .peer
            .ask::<Vec<be_block_protocol::Message>>(Query::Mailbox, true)
            .await
            .expect("the connection stayed open");
        assert!(mailbox.is_ok(), "following the mailbox failed: {mailbox:?}");
    }

    async fn round_trip(&mut self) {
        client::server::ping(&self.peer)
            .await
            .expect("the connection stayed open");
        self.collect();
    }

    fn collect(&mut self) {
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        while let Some(heard) = &mut self.heard {
            match heard.poll_recv(&mut context) {
                std::task::Poll::Ready(Some(event)) => self.notifications.push(Notice::of(event)),
                std::task::Poll::Ready(None) => self.heard = None,
                std::task::Poll::Pending => break,
            }
        }
    }

    /// The account's storage usage (`query.storage`).
    async fn storage(&mut self) -> Result<be_block_protocol::AccountStorage, Error> {
        self.call(|peer| peer.read(Query::Storage)).await
    }

    /// The organization's key work (`query.key-work`).
    async fn key_work(&mut self) -> Result<be_block_protocol::KeyWork, Error> {
        self.call(|peer| peer.read(Query::KeyWork)).await
    }

    /// Follows the entry point of `block`'s tree from now on, so `changes` tells this connection
    /// as it advances; answers the entry.
    async fn follow(&mut self, block: Uuid) -> Uuid {
        let entry = self
            .read_block(block)
            .await
            .entry
            .expect("a block this client views has an entry point");
        self.follow_entry(entry).await;
        entry
    }

    /// Follows `entry` from now on (`changed-since`'s `follow`), reading to the end of it.
    async fn follow_entry(&mut self, entry: Uuid) {
        let sequence = self
            .call(|peer| {
                peer.ask::<be_block_protocol::ChangedPage>(
                    be_block_protocol::Query::Tree {
                        entry: entry,
                        after: ChangeCursor {
                            changed: u64::MAX >> 1,
                            block: Uuid::max(),
                        },
                        limit: 1,
                    },
                    true,
                )
            })
            .await
            .expect("following failed")
            .sequence;
        self.cursors.insert(
            entry,
            ChangeCursor {
                changed: sequence,
                block: Uuid::max(),
            },
        );
    }

    /// Reads what changed in the tree of `entry` since this client last read it, every page,
    /// and moves its cursor past it: the summaries of what changed, and what left it.
    async fn read_changes(&mut self, entry: Uuid) -> (Vec<BlockSummary>, Vec<Uuid>) {
        let mut after = self.cursors.get(&entry).copied().unwrap_or(ChangeCursor {
            changed: 0,
            block: Uuid::max(),
        });
        let (mut summaries, mut removed) = (Vec::new(), Vec::new());
        loop {
            let page = self
                .call(|peer| {
                    peer.ask::<be_block_protocol::ChangedPage>(
                        be_block_protocol::Query::Tree {
                            entry: entry,
                            after: after,
                            limit: 100,
                        },
                        false,
                    )
                })
                .await
                .expect("reading changes failed");
            summaries.extend(page.summaries);
            removed.extend(page.removed);
            match page.next {
                Some(next) => after = next,
                None => {
                    self.cursors.insert(
                        entry,
                        ChangeCursor {
                            changed: page.sequence,
                            block: Uuid::max(),
                        },
                    );
                    return (summaries, removed);
                }
            }
        }
    }

    /// Waits until a tree this client follows tells that `block` left it, reading each change
    /// it is told of; answers the entry it left.
    async fn heard_left(&mut self, block: Uuid) -> Uuid {
        loop {
            if let Notice::TreeAdvanced { entry, .. } = self.graph_change().await {
                let (_, removed) = self.read_changes(entry).await;
                if removed.contains(&block) {
                    return entry;
                }
            }
        }
    }

    /// Waits until a tree this client follows tells of a change to `block` that `matches`,
    /// reading each change it is told of; answers its summary.
    async fn heard_change(
        &mut self,
        block: Uuid,
        matches: impl Fn(&BlockSummary) -> bool,
    ) -> BlockSummary {
        loop {
            if let Notice::TreeAdvanced { entry, .. } = self.graph_change().await {
                let (summaries, _) = self.read_changes(entry).await;
                if let Some(found) = summaries
                    .into_iter()
                    .find(|summary| summary.id == block && matches(summary))
                {
                    return found;
                }
            }
        }
    }

    /// Waits until the tree of `block`'s entry point (which this client follows) tells that
    /// `block`'s head is `head`, reading each change it is told of.
    async fn heard_head(&mut self, block: Uuid, head: CommitId) {
        loop {
            if let Notice::TreeAdvanced { entry, .. } = self.graph_change().await {
                let (summaries, _) = self.read_changes(entry).await;
                if summaries
                    .iter()
                    .any(|summary| summary.id == block && summary.head == Some(head))
                {
                    return;
                }
            }
        }
    }

    async fn try_join(&mut self, block: Uuid) -> Result<(ClientId, SessionState), Error> {
        self.try_join_as(block, TYPE_BUILD).await
    }

    /// Joins `block`'s session running the build `tag` of its type.
    async fn try_join_as(
        &mut self,
        block: Uuid,
        tag: be_block_store::Hash,
    ) -> Result<(ClientId, SessionState), Error> {
        let joined = self
            .call(|peer| {
                peer.ask::<be_block_protocol::JoinedSession>(Query::Session { block, tag }, true)
            })
            .await
            .map(|joined| (joined.client, joined.state));
        if let Ok((_, state)) = &joined {
            self.joined.insert(block, state.clone());
        }
        joined
    }

    /// Leaves `block`'s session (`unfollow` of its topic).
    async fn leave(&mut self, block: Uuid) {
        let left = self.call(|peer| peer.unfollow(Topic::Session(block))).await;
        assert!(left.is_ok(), "leaving the session failed: {left:?}");
    }

    async fn join_session(&mut self, block: Uuid) -> (ClientId, SessionState) {
        self.try_join(block).await.expect("joining failed")
    }

    /// Hands `block`'s order a submission of `payload`, numbered `counter` by `origin`.
    async fn submit(
        &mut self,
        block: Uuid,
        origin: u64,
        counter: u64,
        base: u64,
        payload: Vec<u8>,
    ) -> Result<(), Error> {
        let kind = SessionSend::Ordered(be_block_protocol::Submission {
            origin,
            counter,
            base,
            barrier: false,
        });
        self.call(|peer| peer.send(Target::Session { block, kind }, payload))
            .await
    }

    /// `block`'s session as this client knows it once the server answered everything it sent
    /// before: the membership it joined at with every entry it heard since.
    async fn joined_state(&mut self, block: Uuid) -> SessionState {
        use be_block_session::Membership;
        self.round_trip().await;
        let mut state = self.joined[&block].clone();
        for notice in &self.notifications {
            if let Notice::Membership { block: of, entry } = notice
                && *of == block
            {
                state.apply(entry);
            }
        }
        state
    }

    /// Claims the lease of `block`'s session.
    async fn claim(&mut self, block: Uuid) -> Result<(), Error> {
        let kind = SessionSend::Claim;
        self.call(|peer| peer.send(Target::Session { block, kind }, Vec::new()))
            .await
    }

    /// What a reader of `block` is told of its journal.
    async fn journal_of(&mut self, block: Uuid) -> Option<be_block_protocol::Journal> {
        let response = self
            .call(|peer| {
                peer.read::<be_block_protocol::Contents>(be_block_protocol::Query::Blocks {
                    blocks: vec![block],
                    contents: true,
                    held: Vec::new(),
                })
            })
            .await;
        let Ok(contents) = response else {
            panic!("reading failed: {response:?}");
        };
        contents.journals.into_iter().next().flatten()
    }

    /// The next entry of `block`'s order this client hears.
    async fn sequenced(&mut self, of: Uuid) -> be_block_protocol::JournalEntry {
        loop {
            if let Notice::Sequenced { block, entry } = self.notification().await
                && block == of
            {
                return entry;
            }
        }
    }

    /// The membership entries this client hears of `block`'s session until it holds the
    /// `index`-th, applied to `state`.
    async fn membership_until(&mut self, of: Uuid, state: &mut SessionState, index: u64) {
        use be_block_session::Membership;
        while state.index < index {
            if let Notice::Membership { block, entry } = self.notification().await
                && block == of
            {
                state.apply(&entry);
            }
        }
    }

    async fn relay(
        &mut self,
        block: Uuid,
        to: Option<ClientId>,
        payload: Vec<u8>,
    ) -> Result<(), Error> {
        let kind = SessionSend::Ephemeral(to);
        self.call(|peer| peer.send(Target::Session { block, kind }, payload))
            .await
    }

    async fn notification(&mut self) -> Notice {
        loop {
            let next = self.next_notification().await;
            if !matches!(next, Notice::TreeAdvanced { .. } | Notice::Roots) {
                return next;
            }
        }
    }

    /// Waits until a topic this connection follows tells it changed, as `topic` matches it.
    async fn heard_changed(&mut self, topic: Topic) {
        loop {
            if let Notice::Changed { topic: changed, .. } = self.notification().await
                && changed == topic
            {
                return;
            }
        }
    }

    /// Whether this connection was told `topic` changed, after two round trips: a push queued
    /// for it before the first is sent before the second's ping arrives, whichever the server
    /// took first. Forgets what it was told of `topic`.
    async fn told(&mut self, topic: Topic) -> bool {
        self.round_trip().await;
        self.round_trip().await;
        let before = self.notifications.len();
        self.notifications.retain(
            |notice| !matches!(notice, Notice::Changed { topic: changed, .. } if *changed == topic),
        );
        self.notifications.len() != before
    }

    async fn graph_change(&mut self) -> Notice {
        loop {
            let next = self.next_notification().await;
            if matches!(next, Notice::TreeAdvanced { .. } | Notice::Roots) {
                return next;
            }
        }
    }

    async fn next_notification(&mut self) -> Notice {
        self.collect();
        if !self.notifications.is_empty() {
            return self.notifications.remove(0);
        }
        let Some(heard) = &mut self.heard else {
            return std::future::pending().await;
        };
        match heard.recv().await {
            Some(event) => Notice::of(event),
            None => {
                self.heard = None;
                std::future::pending().await
            }
        }
    }

    /// The account this connection signed in as.
    fn id(&self) -> Uuid {
        self.account.expect("the connection signed in")
    }

    /// Moves `account` to a new phrase with nothing sealed to it to reseal, retiring its old keys
    /// in every log it signed in, and answers the new secret, which it signs with from now on.
    async fn new_phrase(&mut self, account: Uuid) -> be_block_keys::RecoverySecret {
        let old = test_secret(account);
        let new = be_block_keys::RecoveryPhrase::generate().secret();
        let found = self
            .call(|peer| {
                peer.read::<be_block_protocol::RecoveryKey>(be_block_protocol::Query::RecoveryKey)
            })
            .await
            .expect("reading the recovery key failed");
        let retirements = found
            .logs
            .into_iter()
            .map(|log| be_block_protocol::Retirement {
                organization: log.organization,
                kind: log.kind,
                log: log.log,
                entry: be_block_keys::Entry::retirement(
                    &old,
                    &new,
                    account.into_bytes(),
                    log.log.into_bytes(),
                    log.head.try_into().unwrap(),
                )
                .encode(),
            })
            .collect();
        let identity = new.identity();
        let response = self
            .call(|peer| {
                peer.write::<()>(be_block_protocol::Op::SetRecoveryKey {
                    public: identity.public.0,
                    signing: identity.signing.0,
                    sealed: Vec::new(),
                    changes: Vec::new(),
                    retirements: retirements,
                })
            })
            .await;
        assert!(matches!(response, Ok(())), "{response:?}");
        remember_secret(account, new.clone());
        new
    }

    /// The root of `block`'s access log, as this client's account signs it on making the block.
    fn root(&self, block: Uuid) -> Vec<u8> {
        let account = self.account.expect("a client makes blocks once signed in");
        be_block_keys::Entry::root(
            &test_secret(account),
            account.into_bytes(),
            be_block_keys::LogKind::Block,
            block.into_bytes(),
        )
        .encode()
    }

    async fn organization(&mut self, name: &str) -> Uuid {
        let account = self
            .account
            .expect("a client makes an organization once signed in");
        let response = self
            .call(|peer| {
                let (id, root) = organization_root(account);
                peer.write::<be_block_protocol::Organization>(
                    be_block_protocol::Op::CreateOrganization {
                        id: id,
                        name: name.into(),
                        root: root,
                        pinned: Vec::new(),
                    },
                )
            })
            .await;
        let Ok(organization) = response else {
            panic!("organization creation failed: {response:?}");
        };
        let organization = organization.id;
        self.open(organization).await;
        organization
    }

    /// Works in `organization` from now on, following its roots, as a device that shows it
    /// does: the connection then hears what its account's connections there hear.
    async fn open(&mut self, organization: Uuid) {
        self.peer.organization = Some(organization);
        let response = self
            .call(|peer| {
                peer.ask::<be_block_protocol::Listed>(
                    Query::List {
                        listing: Listing::Children(BlockParent::Root),
                        after: None,
                    },
                    true,
                )
            })
            .await;
        assert!(response.is_ok(), "opening failed: {response:?}");
    }

    async fn create_block(&mut self, parent: BlockParent) -> Uuid {
        let block = Uuid::new_v4();
        let key = Uuid::new_v4();
        let keys = KeySet {
            keys: vec![row(key, block, KeyKind::Data)],
            wrapped: Vec::new(),
        };
        self.create_keyed(block, parent, key, keys).await;
        block
    }

    async fn create_keyed(&mut self, block: Uuid, parent: BlockParent, key: Uuid, keys: KeySet) {
        self.create_typed(block, CONTENT, parent, key, keys).await;
    }

    async fn create_typed(
        &mut self,
        block: Uuid,
        content_type: Uuid,
        parent: BlockParent,
        key: Uuid,
        keys: KeySet,
    ) {
        let ops = self.create_op(block, content_type, parent, key, keys).await;
        let response = self
            .call(|peer| peer.write_all::<be_block_protocol::BlockSummary>(ops))
            .await;
        assert!(response.is_ok(), "{response:?}");
    }

    /// The ops making `block`, signed by this client's account as `create_typed` makes it: the
    /// creation and its keys.
    async fn create_op(
        &mut self,
        block: Uuid,
        content_type: Uuid,
        parent: BlockParent,
        key: Uuid,
        keys: KeySet,
    ) -> Vec<Op> {
        let root = self.root(block);
        let placement = self.placement(block, parent).await;
        vec![
            Op::CreateBlock {
                block,
                content_type,
                parent,
                metadata: Vec::new(),
                references: Vec::new(),
                key,
                owner_only: false,
                root,
                placement,
            },
            Op::PutKeys(PutKeys::Rows(keys)),
        ]
    }

    /// The ops making a new block of the test type at `parent`, with a data key of its own.
    async fn new_block_op(&mut self, parent: BlockParent) -> (Uuid, Vec<Op>) {
        let (block, key) = (Uuid::new_v4(), Uuid::new_v4());
        let keys = KeySet {
            keys: vec![row(key, block, KeyKind::Data)],
            wrapped: Vec::new(),
        };
        (
            block,
            self.create_op(block, CONTENT, parent, key, keys).await,
        )
    }

    /// Every page of `get-keys`, as a client reads them.
    async fn keys(&mut self, blocks: Option<Vec<Uuid>>) -> KeySet {
        let mut keys = KeySet::default();
        let mut after = None;
        loop {
            let asked = blocks.clone();
            let page = self
                .call(|peer| {
                    peer.read::<be_block_protocol::KeyPage>(be_block_protocol::Query::Keys {
                        blocks: asked,
                        after: after,
                    })
                })
                .await
                .expect("reading keys failed");
            keys.keys.extend(page.keys.keys);
            keys.wrapped.extend(page.keys.wrapped);
            match page.next {
                Some(next) => after = Some(next),
                None => return keys,
            }
        }
    }

    async fn upload(&mut self, store: &MemoryStore, hashes: &[Hash]) {
        let objects = hashes
            .iter()
            .map(|hash| store.get(*hash).unwrap().unwrap())
            .collect();
        let response = self.put(objects).await;
        assert!(response.is_ok(), "{response:?}");
    }

    /// Asks for URLs to upload `objects` to, without sending them.
    async fn reserve(&mut self, objects: &[Vec<u8>]) -> Result<Vec<Option<UploadUrl>>, Error> {
        let lengths = objects
            .iter()
            .map(|bytes| ObjectLength {
                hash: Hash::of(bytes),
                length: Some(bytes.len() as u64),
            })
            .collect();
        self.call(|peer| {
            peer.write::<Vec<Option<be_block_protocol::UploadUrl>>>(
                be_block_protocol::Op::ReserveUploads(lengths),
            )
        })
        .await
    }

    /// The hashes of the `objects` the server asks this client to upload: those it does not hold
    /// as far as the client may know (`upload-urls` answers them a URL).
    async fn missing(&mut self, objects: &[Vec<u8>]) -> Result<Vec<Hash>, Error> {
        let urls = self.reserve(objects).await?;
        Ok(objects
            .iter()
            .zip(urls)
            .filter(|(_, url)| url.is_some())
            .map(|(bytes, _)| Hash::of(bytes))
            .collect())
    }

    /// Uploads `objects` as a client does: asks for URLs, then PUTs the bytes of each one the
    /// server does not hold already.
    async fn put(&mut self, objects: Vec<Vec<u8>>) -> Result<(), Error> {
        let urls = self.reserve(&objects).await?;
        assert_eq!(urls.len(), objects.len());
        for (url, bytes) in urls.into_iter().zip(objects) {
            let Some(url) = url else {
                continue;
            };
            let status = put_to(url, bytes).await;
            assert_eq!(status, 200, "the upload was refused");
        }
        Ok(())
    }

    /// Downloads an object as a client does: asks for its URL, then GETs it. None when the
    /// server answers no URL; a URL that is not found is a failure.
    async fn download(&mut self, hash: Hash) -> Result<Option<Vec<u8>>, Error> {
        let mut urls = self
            .call(|peer| {
                peer.read::<Vec<Option<String>>>(be_block_protocol::Query::DownloadUrls(vec![hash]))
            })
            .await?;
        assert_eq!(urls.len(), 1);
        let Some(url) = urls.pop().unwrap() else {
            return Ok(None);
        };
        let answer = get_from(url).await;
        assert_eq!(answer.0, 200, "the download failed");
        Ok(Some(answer.1))
    }

    async fn read_block(&mut self, block: Uuid) -> BlockSummary {
        self.try_read_block(block).await.expect("read failed")
    }

    /// Reads a block's summary through `read-summaries`, answering a block it may not know of
    /// as not found.
    async fn try_read_block(&mut self, block: Uuid) -> Result<BlockSummary, Error> {
        let answered = read_block(&self.peer, block)
            .await
            .expect("the connection stayed open");
        self.collect();
        answered
    }

    async fn list(&mut self, parent: BlockParent) -> Vec<BlockSummary> {
        self.call(|peer| {
            peer.read::<be_block_protocol::Listed>(be_block_protocol::Query::List {
                listing: Listing::Children(parent),
                after: None,
            })
        })
        .await
        .expect("listing failed")
        .blocks
    }

    async fn publish(
        &mut self,
        author: &Author,
        block: Uuid,
        commit: CommitId,
        chunks: Vec<Hash>,
        expected: Option<CommitId>,
        time: i64,
    ) -> Result<be_block_protocol::PublishOutcome, Error> {
        self.seal(author, block, commit, chunks, expected, time, None)
            .await
    }

    /// Publishes as `publish` does, with `journal` a seal of the block's session.
    #[allow(clippy::too_many_arguments)]
    async fn seal(
        &mut self,
        author: &Author,
        block: Uuid,
        commit: CommitId,
        chunks: Vec<Hash>,
        expected: Option<CommitId>,
        time: i64,
        journal: Option<be_block_protocol::JournalSeal>,
    ) -> Result<be_block_protocol::PublishOutcome, Error> {
        self.upload(&author.store, &author.objects_of(commit, &chunks))
            .await;
        let before = author.held_by(expected);
        self.call(|peer| {
            peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish {
                block: block,
                expected: expected,
                commit: Some(be_block_protocol::NewCommit {
                    id: commit,
                    parents: expected.into_iter().collect(),
                    held: objects_change(&before, chunks),
                    time: time,
                    pinned: false,
                    objects: Vec::new(),
                }),
                references: listing(&[], Vec::new()),
                metadata: Vec::new(),
                journal: journal.clone(),
            })
        })
        .await
    }

    /// Trims `block`'s journal to `base` with no new commit: a publish of the head `head`
    /// on itself.
    async fn trim(
        &mut self,
        author: &Author,
        block: Uuid,
        (head, chunks): (CommitId, Vec<Hash>),
        journal: be_block_protocol::JournalSeal,
    ) -> Result<be_block_protocol::PublishOutcome, Error> {
        self.seal(
            author,
            block,
            head,
            chunks,
            Some(head),
            1_000,
            Some(journal),
        )
        .await
    }

    async fn history(&mut self, block: Uuid) -> Vec<HistoryEntry> {
        self.call(|peer| {
            peer.read::<Vec<be_block_protocol::HistoryEntry>>(be_block_protocol::Query::History(
                block,
            ))
        })
        .await
        .expect("history failed")
    }
}

/// PUTs `bytes` to an upload URL with its headers, answering the status.
/// A block's summary, read with `read-summaries`; a block the caller may not know of is not
/// found. The call is sent at once, as any call is, and answered when awaited.
fn read_block(
    peer: &At,
    block: Uuid,
) -> impl Future<Output = Result<Result<BlockSummary, Error>, be_wasm_ops::OpError>> + use<> {
    let read = peer.read::<Vec<Option<be_block_protocol::BlockSummary>>>(
        be_block_protocol::Query::Blocks {
            blocks: vec![block],
            contents: false,
            held: Vec::new(),
        },
    );
    async move {
        Ok(read.await?.and_then(|mut read| {
            read.pop().flatten().ok_or_else(|| Error {
                code: ErrorCode::BlockNotFound,
                message: format!("block {block} is not there for the caller"),
            })
        }))
    }
}

async fn put_to(url: UploadUrl, bytes: Vec<u8>) -> u16 {
    tokio::task::spawn_blocking(move || {
        let mut request = ureq::put(&url.url);
        // ureq sets Content-Length from the body itself, as a browser does.
        for header in url
            .headers
            .iter()
            .filter(|header| !header.name.eq_ignore_ascii_case("content-length"))
        {
            request = request.set(&header.name, &header.value);
        }
        match request.send_bytes(&bytes) {
            Ok(response) => response.status(),
            Err(ureq::Error::Status(status, response)) => {
                let body = response.into_string().unwrap_or_default();
                eprintln!("the upload was answered {status}: {body}");
                status
            }
            Err(error) => panic!("the upload failed: {error}"),
        }
    })
    .await
    .unwrap()
}

/// GETs a download URL, answering the status and the body.
async fn get_from(url: String) -> (u16, Vec<u8>) {
    tokio::task::spawn_blocking(move || {
        let response = match ureq::get(&url).call() {
            Ok(response) => response,
            Err(ureq::Error::Status(_, response)) => response,
            Err(error) => panic!("the download failed: {error}"),
        };
        let status = response.status();
        let mut body = Vec::new();
        std::io::Read::read_to_end(&mut response.into_reader(), &mut body).unwrap();
        (status, body)
    })
    .await
    .unwrap()
}

/// A commit time that many minutes ago: retention keeps one state per minute for the last hour,
/// so commits a minute apart are all kept.
fn minutes_ago(minutes: i64) -> i64 {
    be_block_commit::now_milliseconds() - minutes * be_block_commit::retention::MINUTE
}

fn row(id: Uuid, block: Uuid, kind: KeyKind) -> KeyRow {
    KeyRow {
        id,
        block,
        kind,
        epoch: 1,
    }
}

fn wrap(key: Uuid, wrapped_by: WrappedBy, sealed: &[u8]) -> WrappedKey {
    WrappedKey {
        key,
        wrapped_by,
        sender: Uuid::nil(),
        sealed: sealed.to_vec(),
    }
}

struct Scoped {
    block: Uuid,
    data: Uuid,
    scope: Uuid,
}

impl TestClient {
    async fn scope_root(&mut self, account: Uuid, parent: BlockParent) -> Scoped {
        let (block, data, scope) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let keys = KeySet {
            keys: vec![
                row(data, block, KeyKind::Data),
                row(scope, block, KeyKind::Scope),
            ],
            wrapped: vec![
                wrap(
                    scope,
                    WrappedBy::Account(account),
                    b"scope sealed to its author",
                ),
                wrap(data, WrappedBy::Key(scope), b"data under the scope"),
            ],
        };
        self.create_keyed(block, parent, data, keys).await;
        Scoped { block, data, scope }
    }

    async fn lists(&mut self, account: Uuid, parent: Uuid, children: Vec<Uuid>) {
        let author = Author::new(account);
        let (head, chunks) = author.compose(b"a listing\n", 1_000, None);
        self.upload(&author.store, &author.objects_of(head, &chunks))
            .await;
        let response = self
            .call(|peer| {
                peer.write::<be_block_protocol::PublishOutcome>(be_block_protocol::Op::Publish {
                    block: parent,
                    expected: None,
                    commit: Some(be_block_protocol::NewCommit {
                        id: head,
                        parents: Vec::new(),
                        held: objects_change(&[], chunks),
                        time: 1_000,
                        pinned: false,
                        objects: Vec::new(),
                    }),
                    references: listing(&[], children),
                    metadata: Vec::new(),
                    journal: None,
                })
            })
            .await;
        assert!(
            matches!(
                response,
                Ok(be_block_protocol::PublishOutcome::Published(_))
            ),
            "{response:?}"
        );
    }

    async fn inside(&mut self, scope: &Scoped) -> (Uuid, Uuid) {
        let (block, data) = (Uuid::new_v4(), Uuid::new_v4());
        let keys = KeySet {
            keys: vec![row(data, block, KeyKind::Data)],
            wrapped: vec![wrap(
                data,
                WrappedBy::Key(scope.scope),
                b"data under the scope",
            )],
        };
        self.create_keyed(block, BlockParent::Block(scope.block), data, keys)
            .await;
        (block, data)
    }
}

fn objects_change(before: &[Hash], after: Vec<Hash>) -> be_block_protocol::ObjectChange {
    be_block_protocol::ObjectChange::between(before, &after)
}

fn listing(before: &[Uuid], after: Vec<Uuid>) -> be_block_protocol::ReferenceChange {
    be_block_protocol::ReferenceChange::between(before, &after)
}

struct Author {
    store: MemoryStore,
    commits: CommitStore<MemoryStore>,
    account: Uuid,
}

impl Author {
    fn new(account: Uuid) -> Self {
        let store = MemoryStore::new();
        Self {
            commits: CommitStore::new(
                Vault::new(store.clone(), {
                    let keys = Keyring::new();
                    keys.insert(KEY, ContentKey::from_bytes([5; 32]), uuid::Uuid::nil());
                    keys
                })
                .with_chunker(ChunkerConfig::SMALL),
            ),
            store,
            account,
        }
    }

    fn compose(&self, text: &[u8], time: i64, parent: Option<CommitId>) -> (CommitId, Vec<Hash>) {
        let manifest = self.commits.vault().write(KEY, CONTENT, text).unwrap();
        let chunks = self.commits.vault().objects(&manifest).unwrap();
        let commit = self
            .commits
            .put(
                KEY,
                &Commit::new(manifest, self.account, time).with_parent(parent),
            )
            .unwrap();
        (commit, chunks)
    }

    /// The objects `commit` holds besides itself, as `compose` listed them.
    fn held_by(&self, commit: Option<CommitId>) -> Vec<Hash> {
        commit.map_or_else(Vec::new, |commit| {
            let manifest = self.commits.get(commit).unwrap().manifest;
            self.commits.vault().objects(&manifest).unwrap()
        })
    }

    fn objects_of(&self, commit: CommitId, chunks: &[Hash]) -> Vec<Hash> {
        let mut hashes = vec![commit.hash()];
        hashes.extend_from_slice(chunks);
        hashes
    }
}

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// The greeting of a one-shot call, or of a client that signs in with the request's
/// Authorization header: our build, and no token.
fn one_shot_hello() -> Vec<u8> {
    hello(BUILD, &be_block_protocol::Greeting::default())
}

/// The greeting of a client of our build that signs in with `token`.
fn token_hello(token: &str) -> Vec<u8> {
    hello(
        BUILD,
        &be_block_protocol::Greeting {
            token: Some(token.to_owned()),
        },
    )
}

/// What a server's greeting of our build says: who the connection signed in as.
fn welcome_of(frame: &[u8]) -> be_block_protocol::Welcome {
    let hello = read_hello(frame).expect("the server greets");
    assert_eq!(hello.build, BUILD, "the server is of another build");
    hello
        .said()
        .expect("the server's greeting says who signed in")
}

/// The build a server's greeting names.
fn build_of(frame: &[u8]) -> Option<String> {
    read_hello(frame).map(|hello| hello.build)
}

/// A socket that greeted the server as `build`, signing in with `token`.
async fn greet(url: &str, build: &str, token: Option<&str>) -> Socket {
    let (mut socket, _) = connect_async(url).await.unwrap();
    let greeting = be_block_protocol::Greeting {
        token: token.map(str::to_owned),
    };
    socket
        .send(WsMessage::Binary(hello(build, &greeting)))
        .await
        .unwrap();
    socket
}

async fn pump(
    socket: Socket,
    peer: Peer,
    mut outbound: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    mut closed: oneshot::Receiver<()>,
) {
    let (mut write, mut read) = socket.split();
    loop {
        tokio::select! {
            frame = outbound.recv() => {
                let Some(frame) = frame else { break };
                if write.send(WsMessage::Binary(frame)).await.is_err() {
                    break;
                }
            }
            message = read.next() => match message {
                Some(Ok(WsMessage::Binary(bytes))) => {
                    let _ = peer.receive(&bytes);
                }
                Some(Ok(_)) => {}
                _ => break,
            },
            _ = &mut closed => {
                while let Ok(frame) = outbound.try_recv() {
                    let _ = write.send(WsMessage::Binary(frame)).await;
                }
                let _ = write.close().await;
                while let Some(Ok(_)) = read.next().await {}
                break;
            }
        }
    }
    peer.close();
}

async fn next_binary(socket: &mut Socket) -> Option<Vec<u8>> {
    loop {
        match socket.next().await? {
            Ok(WsMessage::Binary(bytes)) => return Some(bytes),
            Ok(WsMessage::Close(_)) | Err(_) => return None,
            Ok(_) => {}
        }
    }
}

impl TestClient {
    async fn personal(&mut self) -> Uuid {
        let organizations = self
            .call(|peer| {
                peer.read::<Vec<be_block_protocol::Organization>>(
                    be_block_protocol::Query::Organizations,
                )
            })
            .await
            .expect("listing organizations failed");
        let personal = organizations
            .iter()
            .find(|organization| organization.kind == be_block_protocol::OrganizationKind::Personal)
            .expect("registering joined the personal organization")
            .id;
        self.open(personal).await;
        personal
    }

    async fn link(&mut self, grant: Option<be_block_protocol::PendingGrant>) -> Uuid {
        let link = Uuid::new_v4();
        let response = self
            .create_link(link, be_block_protocol::LinkKind::Contact(grant), None)
            .await;
        assert!(response.is_ok(), "{response:?}");
        link
    }

    async fn create_link(
        &mut self,
        link: Uuid,
        kind: be_block_protocol::LinkKind,
        signed: Option<Vec<u8>>,
    ) -> Result<(), Error> {
        self.call(|peer| {
            peer.write::<()>(be_block_protocol::Op::CreateLink {
                link: link,
                card: b"the owner's card".to_vec(),
                kind: kind,
                signed: signed,
            })
        })
        .await
    }

    /// Invites into `organization` (the one open) as an editor, opening the invitation in its
    /// log as a device does (`contacts::invite`); answers the link, or the refusal. Its code is
    /// kept for `accept` (`CODES`).
    async fn invite(&mut self, organization: Uuid) -> Result<Uuid, Error> {
        let link = Uuid::new_v4();
        self.invite_with(link, organization, b"the owner's card".to_vec(), Vec::new())
            .await
            .map(|()| link)
    }

    /// Invites into `organization` as `invite` does, by link `link`, with `card` and `escrow`.
    async fn invite_with(
        &mut self,
        link: Uuid,
        organization: Uuid,
        card: Vec<u8>,
        escrow: Vec<u8>,
    ) -> Result<(), Error> {
        let code = be_block_keys::InviteCode::generate();
        let signed = self
            .invitation_record(code.invitation_key(organization.into_bytes()), true)
            .await;
        CODES.lock().unwrap().insert((link, organization), code);
        self.call(|peer| {
            peer.write::<()>(be_block_protocol::Op::CreateLink {
                link: link,
                card: card.clone(),
                kind: be_block_protocol::LinkKind::Invitation(be_block_protocol::Invitation {
                    organization,
                    escrow: escrow.clone(),
                }),
                signed: signed.clone(),
            })
        })
        .await
    }

    /// An invitation record opening or ending the invitation with `key` in the log of the
    /// organization open, signed with this client's test keys after its head, whatever this
    /// client may do there (the server judges it).
    async fn invitation_record(
        &mut self,
        key: be_block_keys::SigningPublic,
        open: bool,
    ) -> Option<Vec<u8>> {
        let me = self.account?;
        let (log, checked) = self.organization_log().await?;
        let entry = be_block_keys::Entry::sign(
            &test_secret(me),
            me.into_bytes(),
            log,
            Some(checked.head),
            false,
            be_block_keys::Record::Invitation(be_block_keys::Invitation { key, open }),
        );
        Some(entry.encode())
    }

    /// Revokes an invitation as an administrator's device does (`Peer::revoke_invitation`),
    /// ending its key in the organization's log.
    async fn revoke(&mut self, link: Uuid) -> Result<(), Error> {
        let organization = self.peer.organization;
        let key = organization.and_then(|organization| {
            CODES
                .lock()
                .unwrap()
                .get(&(link, organization))
                .map(|code| code.to_bytes())
        });
        let signed = match (key, organization) {
            (Some(code), Some(organization)) => {
                let code = be_block_keys::InviteCode::from_bytes(&code).unwrap();
                self.invitation_record(code.invitation_key(organization.into_bytes()), false)
                    .await
            }
            _ => None,
        };
        self.call(|peer| {
            peer.write::<()>(be_block_protocol::Op::DeleteLink {
                link: link,
                signed: signed.clone(),
            })
        })
        .await
    }

    /// This account's links the server holds still to act on, read from its database: no call
    /// lists them, as each device keeps its links' codes in the account's block.
    async fn links(&mut self) -> Vec<OwnLink> {
        let database = metadata_database(self.directory.as_ref().expect("signed in"));
        let mut statement = database
            .prepare(
                "SELECT id, accepter_id FROM links WHERE owner_id = ?1
                    AND (invite_organization = ''
                         OR (invite_finished = 0 AND invite_revoked = 0
                             AND (accepter_id != '' OR invite_expires > ?2)))
                 ORDER BY id",
            )
            .unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        statement
            .query_map(rusqlite::params![self.id().to_string(), now], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap()
            .map(|row| {
                let (id, accepter) = row.unwrap();
                OwnLink {
                    id: Uuid::parse_str(&id).unwrap(),
                    accepter: Uuid::parse_str(&accepter).ok(),
                }
            })
            .collect()
    }

    /// Accepts `link`; an invitation (one `invite` made) signs this client in with its code,
    /// after the head the link names, as a device does (`contacts::accept_link`).
    async fn accept(&mut self, link: Uuid, key: &[u8]) -> Result<(), Error> {
        let member = self.acceptance(link).await;
        self.call(|peer| {
            peer.write::<()>(be_block_protocol::Op::AcceptLink {
                link: link,
                card: b"the accepter's card".to_vec(),
                contact: b"the owner's key, sealed for the accepter".to_vec(),
                key: key.to_vec(),
                member: member.clone(),
            })
        })
        .await
    }

    /// The member record accepting invitation `link` signs this client in with: none for a
    /// link `invite` did not make, or one that does not read.
    async fn acceptance(&mut self, link: Uuid) -> Option<Vec<u8>> {
        let me = self.account?;
        let read = self
            .call(|peer| peer.read::<be_block_protocol::Link>(Query::Link(link)))
            .await
            .ok()?;
        let be_block_protocol::LinkKind::Invitation(invitation) = read.kind else {
            return None;
        };
        let code = CODES
            .lock()
            .unwrap()
            .get(&(link, invitation.organization))
            .map(|code| code.to_bytes())?;
        let code = be_block_keys::InviteCode::from_bytes(&code).ok()?;
        let entry = be_block_keys::Entry::accepted(
            &code,
            invitation.organization.into_bytes(),
            read.head?,
            me.into_bytes(),
            &test_secret(me),
        );
        Some(entry.encode())
    }

    /// The accounts the server holds as this account's contacts, read from its database: no
    /// call lists them, as each device keeps its records in the account's block.
    async fn contacts(&mut self) -> Vec<Uuid> {
        let database = metadata_database(self.directory.as_ref().expect("signed in"));
        let mut statement = database
            .prepare("SELECT contact_id FROM contacts WHERE account_id = ?1 ORDER BY contact_id")
            .unwrap();
        statement
            .query_map([self.id().to_string()], |row| row.get::<_, String>(0))
            .unwrap()
            .map(|id| Uuid::parse_str(&id.unwrap()).unwrap())
            .collect()
    }

    /// Makes an agent bound to `block`; answers its account and token.
    async fn block_agent(
        &mut self,
        organization: Uuid,
        block: Uuid,
    ) -> Result<(Uuid, String), Error> {
        let secret = be_block_keys::RecoveryPhrase::generate().secret();
        let identity = secret.identity();
        let account = agent_id(
            self.account.expect("a client makes agents once signed in"),
            &identity,
        );
        remember_secret(account, secret.clone());
        let signed = self
            .signed_grant(block, account, be_block_graph::Access::Edit)
            .await
            .unwrap_or_default();
        let member = self.member_record(account, identity, true, true).await;
        self.call(|peer| {
            peer.write_in::<be_block_protocol::CreatedAgent>(
                Some(organization),
                be_block_protocol::Op::CreateAgent {
                    account: account,
                    name: "Terminal host".into(),
                    recovery: identity.public.0,
                    signing: identity.signing.0,
                    contact: Vec::new(),
                    record: Vec::new(),
                    escrow: Vec::new(),
                    bound_to: Some(be_block_protocol::BoundAgent {
                        block,
                        signed: signed.clone(),
                    }),
                    member: member.clone(),
                },
            )
        })
        .await
        .map(|created| {
            remember_secret(created.agent.account, secret);
            (created.agent.account, created.token)
        })
    }

    /// Whether `token` still signs a new connection in.
    async fn signs_in(harness: &Harness, token: &str) -> bool {
        harness.sign_in(token).await.is_ok()
    }

    async fn move_to(&mut self, block: Uuid, parent: BlockParent) -> Result<(), Error> {
        self.move_keyed(block, parent, KeySet::default()).await
    }

    /// Moves `block` to `parent` with `keys`, by a parent record this account signs
    /// (`signed_parent`).
    async fn move_keyed(
        &mut self,
        block: Uuid,
        parent: BlockParent,
        keys: KeySet,
    ) -> Result<(), Error> {
        let record = self.signed_parent(block, parent).await;
        let mut ops = vec![be_block_protocol::Op::Move {
            block: block,
            parent: parent,
            placement: record,
        }];
        if !keys.is_empty() {
            ops.push(Op::PutKeys(PutKeys::Rows(keys)));
        }
        self.call(|peer| peer.write_all::<()>(ops)).await
    }

    /// Where `parent` is, as a parent record names it, with the root of its log as served.
    async fn placement_of(&mut self, parent: BlockParent) -> be_block_keys::Placement {
        use be_block_keys::Placement;
        match parent {
            BlockParent::Root => Placement::Top,
            BlockParent::Detached => Placement::Detached,
            BlockParent::Block(above) => {
                let log = decode_steps(
                    self.call(|peer| {
                        peer.read::<Vec<Vec<u8>>>(be_block_protocol::Query::Log(Some(above)))
                    })
                    .await
                    .unwrap_or_default(),
                );
                Placement::Block {
                    block: above.into_bytes(),
                    root: log.first().map_or([0; 32], be_block_keys::Step::hash),
                }
            }
        }
    }

    /// The parent record a new block's log takes after its root (`root`), placing it at
    /// `parent`: its creator's, on the log's own authority.
    async fn placement(&mut self, block: Uuid, parent: BlockParent) -> Vec<u8> {
        let Some(account) = self.account else {
            return Vec::new();
        };
        let placement = self.placement_of(parent).await;
        let root = be_block_keys::Entry::decode(&self.root(block)).expect("a root reads");
        be_block_keys::Entry::sign(
            &test_secret(account),
            account.into_bytes(),
            block.into_bytes(),
            Some(root.hash()),
            false,
            be_block_keys::Record::Parent(be_block_keys::Parent {
                placement,
                authority: be_block_keys::Authority::Log,
            }),
        )
        .encode()
    }

    /// The parent record moving `block` to `parent`, signed by this account at the head of the
    /// block's log on the authority `authority` finds for Edit; empty when the log is not served.
    async fn signed_parent(&mut self, block: Uuid, parent: BlockParent) -> Vec<u8> {
        let Some(me) = self.account else {
            return Vec::new();
        };
        let log = decode_steps(
            self.call(|peer| peer.read::<Vec<Vec<u8>>>(be_block_protocol::Query::Log(Some(block))))
                .await
                .unwrap_or_default(),
        );
        let Some(head) = log.last().map(be_block_keys::Step::hash) else {
            return Vec::new();
        };
        let placement = self.placement_of(parent).await;
        let authority = self
            .authority(block, &log, be_block_keys::Level::Edit)
            .await;
        be_block_keys::Entry::sign(
            &test_secret(me),
            me.into_bytes(),
            block.into_bytes(),
            Some(head),
            false,
            be_block_keys::Record::Parent(be_block_keys::Parent {
                placement,
                authority,
            }),
        )
        .encode()
    }

    /// Where this account's `level` on `block`, whose log is `log`, comes from: the log itself,
    /// else the nearest block above whose log gives it, else administering the organization.
    /// Each log is checked alone, so one resting on authority from outside gives nothing here;
    /// the server checks the record whole.
    async fn authority(
        &mut self,
        block: Uuid,
        log: &[be_block_keys::Step],
        level: be_block_keys::Level,
    ) -> be_block_keys::Authority {
        use be_block_keys::{Authority, LogKind, Step};
        let Some(me) = self.account else {
            return Authority::Log;
        };
        let check = |kind, of: [u8; 16], entries: &[Step]| {
            let pinned = entries.first()?.hash();
            be_block_keys::check_log(kind, of, pinned, entries, &|_| None).ok()
        };
        if check(LogKind::Block, block.into_bytes(), log).is_some_and(|checked| {
            checked
                .holding(LogKind::Block, me.into_bytes(), level)
                .is_some()
        }) {
            return Authority::Log;
        }
        // The chain up to the nearest block above that gives it, each as of its head now.
        let mut at = block;
        let mut chain = Vec::new();
        while let Some(BlockParent::Block(parent)) = self
            .try_read_block(at)
            .await
            .ok()
            .and_then(|summary| summary.parent)
        {
            at = parent;
            let above = decode_steps(
                self.call(|peer| {
                    peer.read::<Vec<Vec<u8>>>(be_block_protocol::Query::Log(Some(parent)))
                })
                .await
                .unwrap_or_default(),
            );
            let Some(head) = above.last() else {
                break;
            };
            chain.push(be_block_keys::Link {
                block: parent.into_bytes(),
                head: head.hash(),
            });
            if check(LogKind::Block, parent.into_bytes(), &above).is_some_and(|checked| {
                checked
                    .holding(LogKind::Block, me.into_bytes(), level)
                    .is_some()
            }) {
                return Authority::Inherited(chain);
            }
        }
        let organization = decode_steps(
            self.call(|peer| peer.read::<Vec<Vec<u8>>>(Query::Log(None)))
                .await
                .unwrap_or_default(),
        );
        match organization.last() {
            Some(head) => Authority::Organization(head.hash()),
            None => Authority::Log,
        }
    }

    async fn grant(
        &mut self,
        block: Uuid,
        account: Uuid,
        access: be_block_graph::Access,
    ) -> Result<(), Error> {
        let signed = self.signed_grant(block, account, access).await;
        self.call(|peer| {
            peer.write::<()>(be_block_protocol::Op::AppendLog(
                be_block_protocol::LogRecord::Grant {
                    block: block,
                    account: account,
                    access: access,
                    signed: signed.clone(),
                },
            ))
        })
        .await
    }

    /// Sets `account`'s role as a device does (`Peer::set_role`): making or unmaking an
    /// administrator of an organization with a log carries the record this client's test keys
    /// sign, naming the account's.
    async fn set_role(
        &mut self,
        account: Uuid,
        role: be_block_protocol::OrganizationRole,
    ) -> Result<(), Error> {
        let op = self.role_op(account, role).await;
        self.call(|peer| peer.write::<()>(op)).await
    }

    /// The op giving `account` `role`, with the administrator record the caller signs when it
    /// makes or unmakes an administrator of an organization with a log.
    async fn role_op(&mut self, account: Uuid, role: be_block_protocol::OrganizationRole) -> Op {
        use be_block_keys::{Administrator, Record};
        let administrator = role == be_block_protocol::OrganizationRole::Administrator;
        let checked = self.organization_log().await;
        let signed = match (checked, self.account) {
            (Some((log, checked)), Some(me))
                if checked.administrators.contains_key(&account.into_bytes()) != administrator =>
            {
                checked
                    .next(
                        &test_secret(me),
                        me.into_bytes(),
                        log,
                        Record::Administrator(Administrator {
                            account: account.into_bytes(),
                            identity: test_secret(account).identity(),
                            administrator,
                        }),
                    )
                    .map(|entry| entry.encode())
            }
            _ => None,
        };
        Op::AppendLog(be_block_protocol::LogRecord::Role {
            account,
            role,
            signed,
        })
    }

    /// A member record signing `account`, with `identity`, in or out of the organization open's
    /// log at its head, signed with this client's test keys (`Peer::member_record`): none in an
    /// organization with no log.
    async fn member_record(
        &mut self,
        account: Uuid,
        identity: be_block_keys::PublicIdentity,
        member: bool,
        agent: bool,
    ) -> Option<Vec<u8>> {
        use be_block_keys::{Member, Record};
        let me = self.account?;
        let (log, checked) = self.organization_log().await?;
        let entry = checked.next(
            &test_secret(me),
            me.into_bytes(),
            log,
            Record::Member(Member {
                account: account.into_bytes(),
                identity: member.then_some(identity),
                member,
                agent,
                invitation: None,
            }),
        )?;
        Some(entry.encode())
    }

    /// The organization open's log as served to this client, checked from its root: its id and
    /// what it gives. None in an organization with no log.
    async fn organization_log(&mut self) -> Option<([u8; 16], be_block_keys::Checked)> {
        let steps = decode_steps(
            self.call(|peer| peer.read::<Vec<Vec<u8>>>(Query::Log(None)))
                .await
                .ok()?,
        );
        let root = steps.first()?.entry()?.clone();
        let checked = be_block_keys::check_log(
            be_block_keys::LogKind::Organization,
            root.log,
            root.hash(),
            &steps,
            &|_| None,
        )
        .ok()?;
        Some((root.log, checked))
    }

    /// Makes an agent of this client's in the organization open, with test keys of its own, as
    /// a device does (`create_agent` in be-host-stack): signed in by a member record in a
    /// company organization. Answers it.
    async fn agent(
        &mut self,
        organization: Uuid,
        name: &str,
    ) -> Result<be_block_protocol::CreatedAgent, Error> {
        let secret = be_block_keys::RecoveryPhrase::generate().secret();
        let identity = secret.identity();
        let account = agent_id(
            self.account.expect("a client makes agents once signed in"),
            &identity,
        );
        remember_secret(account, secret);
        let member = self.member_record(account, identity, true, true).await;
        let name = name.to_owned();
        self.call(|peer| {
            peer.write_in::<be_block_protocol::CreatedAgent>(
                Some(organization),
                be_block_protocol::Op::CreateAgent {
                    account: account,
                    name: name.clone(),
                    recovery: identity.public.0,
                    signing: identity.signing.0,
                    contact: Vec::new(),
                    record: Vec::new(),
                    escrow: Vec::new(),
                    bound_to: None,
                    member: member.clone(),
                },
            )
        })
        .await
    }

    /// Removes `account` from the organization open as a device does (`Peer::remove_member`):
    /// an agent with `delete-agent`, signed out of the log when it names it; a person signed out
    /// of the log by `append-log`, signed with this client's test keys whatever it may do (the
    /// server judges it).
    async fn remove_member(&mut self, account: Uuid) -> Result<(), Error> {
        // Read from the database, as a device knows from the members it lists.
        let agent = metadata_database(self.directory.as_ref().expect("signed in"))
            .query_row(
                "SELECT COUNT(*) FROM agents WHERE account_id = ?1",
                [account.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
            > 0;
        let log = self.organization_log().await;
        let named = log
            .as_ref()
            .and_then(|(_, checked)| checked.member(account.into_bytes()));
        let revocations = self.removal_revocations(account, false).await;
        let op = match agent {
            true => {
                let signed = match named {
                    Some(identity) => self.member_record(account, identity, false, true).await,
                    None => None,
                };
                Op::DeleteAgent {
                    account,
                    signed,
                    revocations,
                }
            }
            false => {
                let me = self
                    .account
                    .expect("a client removes members once signed in");
                let Some((organization, checked)) = log else {
                    panic!("a person is removed from an organization with a log");
                };
                let record = be_block_keys::Record::Member(be_block_keys::Member {
                    account: account.into_bytes(),
                    identity: None,
                    member: false,
                    agent: false,
                    invitation: None,
                });
                let lowers = checked.lowers(&record);
                let signed = be_block_keys::Entry::sign(
                    &test_secret(me),
                    me.into_bytes(),
                    organization,
                    Some(checked.head),
                    lowers,
                    record,
                )
                .encode();
                Op::AppendLog(be_block_protocol::LogRecord::Member {
                    signed,
                    revocations,
                })
            }
        };
        self.call(|peer| peer.write::<()>(op.clone())).await
    }

    /// Removes `account` as this client's contact, with the revocations a device signs
    /// (`Peer::remove_contact`).
    async fn remove_contact(&mut self, account: Uuid) -> Result<(), Error> {
        let revocations = self.removal_revocations(account, true).await;
        self.call(|peer| {
            peer.write::<()>(be_block_protocol::Op::RemoveContact {
                account: account,
                revocations: revocations.clone(),
            })
        })
        .await
    }

    /// The revocations removing `account` makes in the organization open's logs, signed with
    /// this client's test keys; none when no organization is open.
    async fn removal_revocations(&mut self, account: Uuid, contact: bool) -> Vec<Vec<u8>> {
        let pairs = self
            .call(|peer| {
                peer.read::<Vec<be_block_protocol::Revocation>>(
                    be_block_protocol::Query::Revocations {
                        account: account,
                        contact: contact,
                    },
                )
            })
            .await
            .unwrap_or_default();
        let mut signed = Vec::new();
        for revoked in pairs {
            if let Some(record) = self
                .signed_grant(revoked.block, revoked.account, be_block_graph::Access::None)
                .await
            {
                signed.push(record);
            }
        }
        signed
    }

    /// The grant record a device sends with `set-access` (`Peer::grant`), signed with this
    /// client's test keys and naming the grantee's (for anyone inside, an opening): as the
    /// block's manager, else as an administrator of the organization. None for a revocation the
    /// log does not need, or a block the server lists no log for this client.
    async fn signed_grant(
        &mut self,
        block: Uuid,
        account: Uuid,
        access: be_block_graph::Access,
    ) -> Option<Vec<u8>> {
        use be_block_keys::{Authority, Grant, Opening, Record, Step};
        let opening = account == be_block_graph::ANYONE_INSIDE;
        let me = self.account?;
        let decode = decode_steps;
        let log = decode(
            self.call(|peer| peer.read::<Vec<Vec<u8>>>(be_block_protocol::Query::Log(Some(block))))
                .await
                .ok()?,
        );
        let level = crate::access::level(access);
        // The last grant the log names for them; the server checks the whole log.
        let named =
            log.iter()
                .rev()
                .filter_map(Step::entry)
                .find_map(|entry| match &entry.record {
                    Record::Grant(grant) if grant.grantee == account.into_bytes() => {
                        Some(grant.level)
                    }
                    Record::Opening(opened) if opening => Some(opened.level),
                    _ => None,
                });
        if level == be_block_keys::Level::None
            && named.is_none_or(|named| named == be_block_keys::Level::None)
        {
            return None;
        }
        let own = !opening && account == me && level == be_block_keys::Level::None;
        let authority = match own {
            true => Authority::Log,
            false => {
                self.authority(block, &log, be_block_keys::Level::Manage)
                    .await
            }
        };
        // As `Checked::lowers` finds it: below the level the log last named.
        let lowers = named.is_some_and(|named| level < named);
        let entry = be_block_keys::Entry::sign(
            &test_secret(me),
            me.into_bytes(),
            block.into_bytes(),
            Some(log.last()?.hash()),
            lowers,
            match opening {
                true => Record::Opening(Opening { level, authority }),
                false => Record::Grant(Grant {
                    grantee: account.into_bytes(),
                    identity: test_secret(account).identity(),
                    level,
                    authority,
                }),
            },
        );
        Some(entry.encode())
    }
}

/// A served log's steps (`be_block_keys::Step`), root first; any that does not read is left out.
fn decode_steps(served: Vec<Vec<u8>>) -> Vec<be_block_keys::Step> {
    served
        .iter()
        .filter_map(|step| be_block_keys::Step::decode(step).ok())
        .collect()
}

fn poll<F: Future + Unpin>(future: &mut F) -> std::task::Poll<F::Output> {
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    std::pin::Pin::new(future).poll(&mut context)
}

fn refused_as<T>(response: &Result<T, Error>, expected: ErrorCode) -> bool {
    matches!(response, Err(Error { code, .. }) if *code == expected)
}

async fn http(
    harness: &Harness,
    method: &str,
    path: &str,
    body: &str,
    origin: Option<&str>,
) -> (u16, String) {
    let address = harness.address;
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    let origin = origin
        .map(|origin| format!("Origin: {origin}\r\n"))
        .unwrap_or_default();
    stream.write_all(format!("{method} {path} HTTP/1.1\r\nHost: {address}\r\n{origin}Content-Length: {}\r\nContent-Type: application/x-www-form-urlencoded\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).await.unwrap();
    let response = String::from_utf8(bytes).unwrap();
    let status = response.split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, response)
}
async fn begin_session(harness: &Harness) -> (String, String) {
    let begun = harness
        .once(|peer| client::server::device_login(peer, DeviceLoginStep::Begin))
        .await
        .expect("a browser authorization session starts");
    let DeviceLoginAnswer::Begun(begun) = begun else {
        panic!("beginning a sign-in answered {begun:?}");
    };
    (begun.code, begun.secret)
}

/// Polls a browser sign-in once, as a one-shot call.
/// Begins a browser sign-in (`device-login(begin)`), answering what it began.
fn begin_login(
    peer: &At,
) -> impl Future<Output = Answered<be_block_protocol::BrowserLogin>> + use<> {
    let begun = client::server::device_login(peer, DeviceLoginStep::Begin);
    async move {
        Ok(begun.await?.map(|answer| match answer {
            DeviceLoginAnswer::Begun(begun) => begun,
            other => panic!("beginning a sign-in answered {other:?}"),
        }))
    }
}

async fn poll_login(
    harness: &Harness,
    code: &str,
    secret: &str,
) -> Result<be_block_protocol::BrowserLoginState, Error> {
    let (code, secret) = (code.to_owned(), secret.to_owned());
    let polled = harness
        .once(|peer| client::server::device_login(peer, DeviceLoginStep::Poll { code, secret }))
        .await?;
    match polled {
        DeviceLoginAnswer::Polled(state) => Ok(state),
        other => panic!("polling a sign-in answered {other:?}"),
    }
}

/// Posts `body` to `path` and answers the status and the body of the answer.
async fn post_bytes(harness: &Harness, path: &str, body: &[u8]) -> (u16, Vec<u8>) {
    let address = harness.address;
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream
        .write_all(
            format!(
                "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    stream.write_all(body).await.unwrap();
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).await.unwrap();
    let end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("an HTTP answer");
    let head = String::from_utf8_lossy(&bytes[..end]).to_string();
    let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
    (status, bytes[end + 4..].to_vec())
}

fn framed(frames: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    for frame in frames {
        body.extend_from_slice(&(frame.len() as u32).to_be_bytes());
        body.extend_from_slice(frame);
    }
    body
}

fn unframed(mut body: &[u8]) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    while body.len() >= 4 {
        let size = u32::from_be_bytes(body[..4].try_into().unwrap()) as usize;
        frames.push(body[4..4 + size].to_vec());
        body = &body[4 + size..];
    }
    frames
}

impl Harness {
    /// Makes one call as a one-shot post (`POST /api/be/call`), as a client signing in does.
    async fn once<T: 'static, F: Future<Output = Answered<T>>>(
        &self,
        start: impl FnOnce(&At) -> F,
    ) -> Result<T, Error> {
        let (sending, mut sent) = tokio::sync::mpsc::unbounded_channel();
        let peer = Peer::new(be_wasm_ops::Side::Guest, move |frame| {
            let _ = sending.send(frame);
        });
        let at = At {
            peer: peer.clone(),
            organization: None,
        };
        let mut pending = Box::pin(start(&at));
        let _ = poll(&mut pending);
        let call = sent.try_recv().expect("the call was started");
        let (status, answer) =
            post_bytes(self, "/api/be/call", &framed(&[one_shot_hello(), call])).await;
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&answer));
        let frames = unframed(&answer);
        assert_eq!(build_of(&frames[0]).as_deref(), Some(BUILD));
        for frame in &frames[1..] {
            peer.receive(frame).unwrap();
        }
        pending.await.expect("the call was answered")
    }
}
async fn fixture_account(
    directory: &std::path::Path,
    email: &str,
    name: &str,
    organization: &str,
) -> Result<(Uuid, Uuid), ServerError> {
    let store = ServerStore::open(directory)?;
    let (profile, _) = store.register(email, name).await?;
    let company = signed_organization(&store, profile.account, organization).await?;
    Ok((profile.account, company.id))
}

/// Proves the browser session is the account's owner, as the portal's code to its email does.
async fn prove(store: &ServerStore, account: Uuid, email: &str, browser: &str) {
    use crate::store::codes::{self, CodePurpose, CodeRequest};
    let attempt = codes::new_attempt();
    let mailed = store
        .request_code(CodeRequest {
            purpose: CodePurpose::Proof,
            email,
            account: Some(account),
            display_name: "",
            invite: "",
            device_code: "",
            attempt: &attempt,
            address: "127.0.0.1",
        })
        .await
        .unwrap()
        .unwrap();
    store
        .enter_code(
            &attempt,
            &mailed.code,
            browser,
            "127.0.0.1",
            |_, _| true,
            ServerConfig::OPEN,
        )
        .await
        .unwrap();
}

#[derive(Default)]
struct CapturedMail(std::sync::Mutex<Vec<mail::Email>>);
impl mail::Delivery for CapturedMail {
    fn send(&self, email: mail::Email) {
        self.0.lock().unwrap().push(email);
    }
}
impl Harness {
    /// A fresh invite code with no limits, as `be-server invite-code create` makes.
    fn invite(&self) -> String {
        crate::store::invite_codes::create(&self.directory, None, None)
            .unwrap()
            .0
    }

    /// The code and the link's attempt in the latest code mailed to `email`.
    fn mailed_code(&self, email: &str) -> (String, String) {
        let messages = self.mail.0.lock().unwrap();
        let body = &messages
            .iter()
            .rev()
            .find(|message| message.recipient == email && message.body.starts_with("Your code is "))
            .unwrap()
            .body;
        let code = body["Your code is ".len()..]
            .split('.')
            .next()
            .unwrap()
            .to_owned();
        let attempt = body
            .split("attempt=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap()
            .to_owned();
        (code, attempt)
    }

    /// Lets every browser's proof go stale, as `codes::FRESH_PROOF` passing would.
    fn stale_proofs(&self) {
        metadata_database(&self.directory)
            .execute("UPDATE sessions SET proved = 0", [])
            .unwrap();
    }

    /// How many messages went to `email` with `subject`.
    fn mailed(&self, email: &str, subject: &str) -> usize {
        self.mail
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|message| message.recipient == email && message.subject == subject)
            .count()
    }

    /// How many messages went to `email`.
    fn mails_to(&self, email: &str) -> usize {
        self.mail
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|message| message.recipient == email)
            .count()
    }

    fn mail_token(&self, email: &str) -> String {
        let messages = self.mail.0.lock().unwrap();
        let body = &messages
            .iter()
            .rev()
            .find(|message| message.recipient == email)
            .unwrap()
            .body;
        body.split("?token=")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_string()
    }
}

#[derive(Default)]
struct Browser {
    cookie: String,
    /// The `be3-attempt` cookie of a pending code.
    attempt: String,
    /// The host it asks for and its pages' origin: the server's address unless set.
    host: String,
}
impl Browser {
    async fn request(
        &mut self,
        harness: &Harness,
        method: &str,
        path: &str,
        body: &str,
    ) -> (u16, String) {
        let address = harness.address;
        let host = match self.host.is_empty() {
            true => address.to_string(),
            false => self.host.clone(),
        };
        let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
        stream.write_all(format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nCookie: be3-account={}; be3-attempt={}\r\nContent-Length: {}\r\nContent-Type: application/x-www-form-urlencoded\r\nConnection: close\r\n\r\n{body}", self.cookie, self.attempt, body.len()).as_bytes()).await.unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.unwrap();
        let response = String::from_utf8(bytes).unwrap();
        if let Some(cookie) = response.split("Set-Cookie: be3-account=").nth(1) {
            self.cookie = cookie.split(';').next().unwrap().into();
        }
        if let Some(attempt) = response.split("Set-Cookie: be3-attempt=").nth(1) {
            self.attempt = attempt.split(';').next().unwrap().into();
        }
        (
            response.split_whitespace().nth(1).unwrap().parse().unwrap(),
            response,
        )
    }
    async fn get(&mut self, harness: &Harness, path: &str) -> String {
        let (status, page) = self.request(harness, "GET", path, "").await;
        assert_eq!(status, 200, "{page}");
        page
    }
    async fn submit(
        &mut self,
        harness: &Harness,
        page_path: &str,
        action: &str,
        fields: &[(&str, &str)],
    ) -> (u16, String) {
        let page = self.get(harness, page_path).await;
        let csrf = csrf(&page);
        let body = format!(
            "csrf={csrf}&{}",
            fields
                .iter()
                .map(|(key, value)| format!("{}={}", portal::encode(key), portal::encode(value)))
                .collect::<Vec<_>>()
                .join("&")
        );
        self.request(harness, "POST", action, &body).await
    }
    /// Asks for a sign-in code for `email` from the sign-in page.
    async fn ask_code(&mut self, harness: &Harness, email: &str) -> (u16, String) {
        self.submit(
            harness,
            "/auth/login",
            "/auth/code/send",
            &[("email", email)],
        )
        .await
    }
    /// Enters a code on the page that asks for it.
    async fn enter_code(&mut self, harness: &Harness, code: &str) -> (u16, String) {
        self.submit(
            harness,
            "/auth/enter",
            "/auth/enter",
            &[("email_code", code)],
        )
        .await
    }
    /// Begins signing in with the fake provider `id` (`query` adds `?intent=` or `?code=`) and
    /// answers where the server sends the browser: the provider's authorization URL.
    /// A link or a proof carries the check the settings page's link does.
    async fn start_provider(&mut self, harness: &Harness, id: &str, query: &str) -> String {
        let check = match query.contains("intent=") && !query.contains("check=") {
            true => format!(
                "&check={}",
                crate::store::providers::start_check(&self.cookie)
            ),
            false => String::new(),
        };
        let (status, response) = self
            .request(
                harness,
                "GET",
                &format!("/auth/oidc/{id}/start{query}{check}"),
                "",
            )
            .await;
        assert_eq!(status, 303, "{response}");
        location(&response)
    }
    /// Comes back from the provider to the server, as the provider sends the browser.
    async fn provider_callback(&mut self, harness: &Harness, back: &str) -> (u16, String) {
        self.request(harness, "GET", back, "").await
    }
    /// Signs in with the fake provider `id` as `grant` says, and answers the callback's answer.
    async fn provider_sign_in(
        &mut self,
        harness: &Harness,
        provider: &crate::fake_provider::FakeProvider,
        id: &str,
        query: &str,
        grant: crate::fake_provider::Grant,
    ) -> (u16, String) {
        let url = self.start_provider(harness, id, query).await;
        let back = provider.authorize(&url, grant);
        self.provider_callback(harness, &back).await
    }
    /// Signs in with a code mailed to `email`.
    async fn sign_in_by_code(&mut self, harness: &Harness, email: &str) {
        assert_eq!(self.ask_code(harness, email).await.0, 303);
        let (code, _) = harness.mailed_code(email);
        let (status, page) = self.enter_code(harness, &code).await;
        assert_eq!(status, 303, "{page}");
        assert!(!self.cookie.is_empty());
    }
    /// A browser at `localhost`, the RP ID passkeys are made for when no `--auth-origin` is set.
    fn at_localhost(harness: &Harness) -> Self {
        Self {
            host: format!("localhost:{}", harness.address.port()),
            ..Self::default()
        }
    }
    /// The origin its pages are at, which an authenticator is told.
    fn origin(&self) -> String {
        format!("http://{}", self.host)
    }
    /// Answers the passkey ceremony the page at `page_path` offers as the portal's wasm does:
    /// `answer` plays the authenticator with the options, and the answer is posted with the
    /// page's form token.
    async fn passkey(
        &mut self,
        harness: &Harness,
        page_path: &str,
        answer: impl FnOnce(&str) -> String,
    ) -> (u16, String) {
        let page = page_of(&self.get(harness, page_path).await);
        let offer = page.passkey.expect("the page offers a passkey");
        let body = format!(
            "csrf={}&code={}&credential={}",
            portal::encode(&page.csrf),
            portal::encode(&page.code),
            portal::encode(&answer(&offer.options)),
        );
        self.request(harness, "POST", offer.step.action(), &body)
            .await
    }
    /// Adds a passkey from the settings page, which needs a fresh proof, and answers the
    /// authenticator that holds it.
    async fn add_passkey(
        &mut self,
        harness: &Harness,
    ) -> crate::fake_authenticator::FakeAuthenticator {
        let origin = self.origin();
        let mut made = None;
        let (status, response) = self
            .passkey(harness, "/auth/settings", |options| {
                let (authenticator, answer) =
                    crate::fake_authenticator::FakeAuthenticator::create(options, &origin, None);
                made = Some(authenticator);
                answer
            })
            .await;
        assert_eq!(status, 303, "{response}");
        assert_eq!(location(&response), "/auth/settings?saved=1");
        made.unwrap()
    }
    /// Signs in with `authenticator`'s passkey from the sign-in page, with no email.
    async fn sign_in_with_passkey(
        &mut self,
        harness: &Harness,
        authenticator: &mut crate::fake_authenticator::FakeAuthenticator,
    ) -> (u16, String) {
        let origin = self.origin();
        self.passkey(harness, "/auth/login", |options| {
            authenticator.get(options, &origin)
        })
        .await
    }
    /// Proves it is the account's owner with a code to its email.
    async fn prove(&mut self, harness: &Harness, email: &str) {
        let (status, page) = self
            .submit(harness, "/auth/settings", "/auth/proof", &[])
            .await;
        assert_eq!(status, 303, "{page}");
        let (code, _) = harness.mailed_code(email);
        let (status, page) = self.enter_code(harness, &code).await;
        assert_eq!(status, 303, "{page}");
    }
}
/// Where a response sends the browser.
fn location(response: &str) -> String {
    response
        .split("Location: ")
        .nth(1)
        .and_then(|rest| rest.split("\r\n").next())
        .unwrap_or_else(|| panic!("no Location in {response}"))
        .to_owned()
}
/// A harness whose server has the fake provider as `test`, authoritative for email or not.
async fn with_provider(authoritative_email: bool) -> (Harness, crate::fake_provider::FakeProvider) {
    let provider = crate::fake_provider::FakeProvider::start().await;
    let harness = Harness::configured(ServerConfig {
        providers: vec![provider.provider("test", authoritative_email)].into(),
        ..ServerConfig::OPEN
    })
    .await;
    (harness, provider)
}
/// How many accounts people have on the server, leaving out the server's own.
fn accounts(harness: &Harness) -> i64 {
    metadata_database(&harness.directory)
        .query_row(
            "SELECT count(*) FROM accounts WHERE id NOT IN (SELECT account_id FROM server_account)",
            [],
            |row| row.get(0),
        )
        .unwrap()
}
fn page_of(response: &str) -> be_server_portal::Page {
    be_server_portal::Page::from_json(response.split("\r\n\r\n").nth(1).unwrap()).unwrap()
}
fn csrf(page: &str) -> String {
    page.split("\"csrf\":\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .into()
}

/// A push service in the test, on loopback: it takes each POST, answers it with what `answer`
/// gives for its path (status 0: never), and hands the request to the test.
struct TestPushService {
    url: String,
    received: tokio::sync::mpsc::UnboundedReceiver<PushRequest>,
}

#[derive(Debug)]
struct PushRequest {
    path: String,
    headers: std::collections::HashMap<String, String>,
    body: Vec<u8>,
}

impl TestPushService {
    async fn start(answer: impl Fn(&str) -> (u16, String) + Send + Sync + 'static) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (sending, received) = tokio::sync::mpsc::unbounded_channel();
        let answer = Arc::new(answer);
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let (sending, answer) = (sending.clone(), Arc::clone(&answer));
                tokio::spawn(async move {
                    let Some(request) = read_request(&mut stream).await else {
                        return;
                    };
                    let (status, body) = answer(&request.path);
                    if status == 0 {
                        // A service that takes the request and never answers.
                        // The task, and its stream with it, waits forever.
                        let _ = sending.send(request);
                        return std::future::pending::<()>().await;
                    }
                    let response = format!(
                        "HTTP/1.1 {status} Answered\r\nContent-Type: application/json\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.shutdown().await;
                    let _ = sending.send(request);
                });
            }
        });
        Self { url, received }
    }

    async fn next(&mut self) -> PushRequest {
        self.received
            .recv()
            .await
            .expect("the push service stays open")
    }
}

async fn read_request(stream: &mut tokio::net::TcpStream) -> Option<PushRequest> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let read = stream.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        bytes.extend_from_slice(&buffer[..read]);
        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut request = httparse::Request::new(&mut headers);
        let httparse::Status::Complete(start) = request.parse(&bytes).ok()? else {
            continue;
        };
        let headers: std::collections::HashMap<String, String> = request
            .headers
            .iter()
            .map(|header| {
                (
                    header.name.to_ascii_lowercase(),
                    String::from_utf8_lossy(header.value).into_owned(),
                )
            })
            .collect();
        let length: usize = headers
            .get("content-length")
            .and_then(|length| length.parse().ok())
            .unwrap_or(0);
        let path = request.path?.to_owned();
        while bytes.len() < start + length {
            let read = stream.read(&mut buffer).await.ok()?;
            if read == 0 {
                return None;
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
        return Some(PushRequest {
            path,
            headers,
            body: bytes[start..start + length].to_vec(),
        });
    }
}

/// A device taking pushes: its P-256 key and authentication secret (RFC 8291). Its key is
/// ring's ephemeral kind, so it decrypts one push.
struct TestDevice {
    private: ring::agreement::EphemeralPrivateKey,
    public: Vec<u8>,
    auth: [u8; 16],
}

impl TestDevice {
    fn new() -> Self {
        let random = ring::rand::SystemRandom::new();
        let private =
            ring::agreement::EphemeralPrivateKey::generate(&ring::agreement::ECDH_P256, &random)
                .unwrap();
        let public = private.compute_public_key().unwrap().as_ref().to_vec();
        let mut auth = [0; 16];
        ring::rand::SecureRandom::fill(&random, &mut auth).unwrap();
        Self {
            private,
            public,
            auth,
        }
    }

    fn endpoint(
        &self,
        service: be_block_protocol::PushService,
        address: String,
    ) -> be_block_protocol::PushEndpoint {
        be_block_protocol::PushEndpoint {
            service,
            address,
            p256dh: self.public.clone(),
            auth: self.auth.to_vec(),
        }
    }

    /// Decrypts a push's body into what it carries.
    fn open(self, body: &[u8]) -> be_block_protocol::PushMessage {
        let sender = be_lib_web_push::sender_key(body).expect("a push names its sender's key");
        let peer = ring::agreement::UnparsedPublicKey::new(&ring::agreement::ECDH_P256, sender);
        let plain = ring::agreement::agree_ephemeral(self.private, &peer, |secret| {
            be_lib_web_push::open(secret, &self.public, &self.auth, body)
        })
        .unwrap()
        .expect("the push decrypts with the device's keys");
        be_block_protocol::PushMessage::decode(&plain).expect("a push carries a push message")
    }
}

impl TestClient {
    async fn register_endpoint(&mut self, endpoint: be_block_protocol::PushEndpoint) {
        let registered = self
            .call(|peer| {
                peer.write::<()>(be_block_protocol::Op::SetPushEndpoint(
                    be_block_protocol::PushEndpointChange::Register(endpoint),
                ))
            })
            .await;
        assert!(registered.is_ok(), "{registered:?}");
    }

    async fn set_mute(&mut self, block: Uuid, mode: be_block_protocol::NotifyMode) {
        let set = self
            .call(|peer| {
                peer.write::<()>(be_block_protocol::Op::SetMute {
                    block: block,
                    mode: mode,
                })
            })
            .await;
        assert!(set.is_ok(), "{set:?}");
    }

    async fn notify(&mut self, block: Uuid, sealed: &[u8], mentions: Vec<Uuid>) {
        let sealed = sealed.to_vec();
        let notified = self
            .call(|peer| {
                peer.send(
                    be_block_protocol::Target::Push {
                        block: block,
                        mentions: mentions,
                        keys: Vec::new(),
                    },
                    sealed,
                )
            })
            .await;
        assert!(notified.is_ok(), "{notified:?}");
    }

    /// Notifies with the keys of what it tells, which the server tells once.
    async fn tell(&mut self, block: Uuid, sealed: &[u8], keys: &[&str]) {
        let sealed = sealed.to_vec();
        let keys = keys
            .iter()
            .map(|key| be_block_protocol::told_key(block, key))
            .collect();
        let notified = self
            .call(|peer| {
                peer.send(
                    be_block_protocol::Target::Push {
                        block: block,
                        mentions: Vec::new(),
                        keys: keys,
                    },
                    sealed,
                )
            })
            .await;
        assert!(notified.is_ok(), "{notified:?}");
    }

    /// Signs out, which leaves the server's record of who is connected before it answers.
    async fn sign_out(&mut self) {
        let logged_out = self
            .call(|peer| peer.write::<()>(be_block_protocol::Op::RevokeToken(None)))
            .await;
        assert!(logged_out.is_ok(), "{logged_out:?}");
    }
}

/// A member of `owner`'s personal organization who is `owner`'s contact and may view `block`.
async fn viewer_of(
    harness: &Harness,
    owner: &mut TestClient,
    block: Uuid,
    email: &str,
) -> (TestClient, Uuid) {
    let mut member = harness.member(email).await;
    let account = member.id();
    member.personal().await;
    let link = owner.link(None).await;
    let accepted = member.accept(link, &[]).await;
    assert!(accepted.is_ok(), "{accepted:?}");
    let granted = owner
        .grant(block, account, be_block_graph::Access::View)
        .await;
    assert!(granted.is_ok(), "{granted:?}");
    (member, account)
}

fn pushing_locally() -> ServerConfig {
    ServerConfig {
        push: push::PushConfig {
            allow_local_endpoints: true,
            fcm: None,
            hosts: vec!["127.0.0.1".to_owned()].into(),
        },
        ..ServerConfig::OPEN
    }
}

/// The account id of the agent `maker` makes with `identity` (`be_block_keys::agent_account`).
fn agent_id(maker: Uuid, identity: &be_block_keys::PublicIdentity) -> Uuid {
    Uuid::from_bytes(be_block_keys::agent_account(maker.into_bytes(), identity))
}

/// Keys tests give an agent whose keys they never use: `seed` throughout, so agents made with
/// other seeds get other ids.
fn unused_keys(seed: u8) -> be_block_keys::PublicIdentity {
    be_block_keys::PublicIdentity {
        public: be_block_keys::RecoveryPublic([seed; 32]),
        signing: be_block_keys::SigningPublic([seed; 32]),
    }
}

/// A seal by `TYPE_BUILD` in the order's first run.
fn of_build(base: be_block_protocol::JournalBase) -> be_block_protocol::JournalSeal {
    in_run(base, TYPE_BUILD, 0)
}

/// A seal by the build `tag` in the order's `run`.
fn in_run(
    base: be_block_protocol::JournalBase,
    tag: Hash,
    run: u64,
) -> be_block_protocol::JournalSeal {
    be_block_protocol::JournalSeal { tag, run, base }
}
