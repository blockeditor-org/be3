use std::{path::PathBuf, time::Duration};

use be_block_commit::{Commit, CommitId, CommitStore};
use be_block_graph::BlockParent;
use be_block_protocol::{
    Authenticated, BlockChange, BlockEvent, BlockSummary, ClientId, Error, ErrorCode, HistoryEntry,
    Hold, KeyKind, KeyRow, KeySet, Listing, ObjectLength, OrganizationRole, PairingMessage,
    SessionEvent, SessionState, UploadUrl, WrappedBy, WrappedKey, client,
};
use be_block_session::{Resume, resume, takeover_needs_merge};
use be_block_store::{ChunkerConfig, ContentKey, Hash, Keyring, MemoryStore, ObjectStore, Vault};
use be_wasm_ops::net::{Peer, Pending, Stream};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::oneshot;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message as WsMessage,
};
use uuid::Uuid;

use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
mod a_backup_restores_into_a_server_that_serves_the_same_blocks;
mod a_block_agent_is_deleted_with_its_block_or_its_grant;
mod a_block_agent_is_refused_while_its_block_is_in_the_trash;
mod a_block_agent_never_becomes_an_administrator;
mod a_block_agent_reaches_its_block_and_nothing_else;
mod a_block_agent_reaches_no_further_than_its_creator;
mod a_block_agents_long_poll_session_is_replaced_too;
mod a_block_agents_new_connection_replaces_the_open_one;
mod a_block_holds_a_bounded_number_of_agents_of_its_own;
mod a_block_in_unfiled_stays_live_without_a_listing_until_it_is_deleted;
mod a_block_linked_before_it_exists_is_announced_only_once_it_does;
mod a_block_shared_from_a_hidden_folder_is_listed_as_shared;
mod a_blocks_references_are_listed_and_announced_together;
mod a_browser_preflight_to_the_s3_endpoint_is_answered;
mod a_browser_sign_in_waits_on_polls_and_holds_no_connection;
mod a_bunny_storage_zone_holds_a_backup;
mod a_client_speaks_the_protocol_over_http_posts;
mod a_closed_server_also_refuses_browser_registration_and_login;
mod a_closed_server_refuses_new_accounts_and_sign_ins_but_not_tokens;
mod a_connection_hears_only_of_the_blocks_it_listed;
mod a_connection_holds_a_bounded_number_of_listings;
mod a_connection_holds_a_bounded_number_of_streams_and_sessions;
mod a_connection_keeps_only_so_many_refused_listings;
mod a_connection_reads_more_blocks_than_it_may_hold_listings;
mod a_contact_card_the_server_swaps_does_not_open;
mod a_detached_subtree_is_collected_and_its_objects_freed;
mod a_device_registers_only_an_endpoint_the_server_would_push_to;
mod a_download_does_not_wait_behind_another_organizations_store_call;
mod a_grant_or_a_key_for_someone_not_a_contact_is_refused;
mod a_graph_change_reaches_only_the_members_who_may_see_it;
mod a_kind_open_inside_passes_from_one_viewer_inside_to_another;
mod a_message_arriving_slowly_is_not_taken_for_silence;
mod a_mode_set_on_one_device_is_announced_to_the_others;
mod a_move_never_takes_a_workspace_or_more_access_without_manage;
mod a_move_past_the_new_payers_quota_is_refused;
mod a_new_recovery_key_reseals_every_key_sealed_to_the_account;
mod a_part_held_until_committed_lasts_until_a_commit_takes_it_or_its_grace_passes;
mod a_pending_grant_is_applied_when_its_link_is_accepted;
mod a_publish_pushes_the_new_storage_usage_to_its_payer;
mod a_push_hosts_cap_is_keyed_by_its_allowlist_entry_and_statuses_are_judged;
mod a_push_reaches_members_who_are_not_connected_and_want_it;
mod a_push_service_refusing_the_server_or_redirecting_does_not_cost_the_right_endpoints;
mod a_push_service_that_never_answers_delays_only_its_own_account;
mod a_range_get_reads_part_of_an_s3_object;
mod a_rebase_by_another_build_marks_the_journal_lost_until_the_next_commit;
mod a_refused_call_keeps_the_cached_graph;
mod a_relay_to_a_client_that_left_the_session_is_dropped_without_an_error;
mod a_remembered_account_approves_a_manually_entered_code_and_can_switch_accounts;
mod a_removed_agents_blocks_stay_charged_to_its_creator;
mod a_reserved_object_counts_only_once_its_bytes_arrive;
mod a_rotation_gives_every_block_below_it_a_new_key_or_nothing;
mod a_search_index_takes_exactly_the_access_of_the_block_it_indexes;
mod a_server_without_a_portal_answers_its_pages_as_json;
mod a_session_hands_ownership_over_without_a_merge;
mod a_settings_block_changes_only_with_manage_on_its_parent;
mod a_settings_kind_opens_to_anyone_who_views_a_block_inside;
mod a_slow_store_call_does_not_delay_another_connections_ping;
mod a_stale_publish_is_rejected_with_the_current_head;
mod a_type_with_a_short_retention_keeps_only_its_recent_states;
mod a_watcher_is_told_when_a_block_is_detached;
mod detaching_a_block_rotates_the_keys_of_who_no_longer_views_it;
mod a_watcher_is_told_when_the_head_moves;
mod an_account_creates_a_bounded_number_of_agents;
mod an_account_quota_refuses_what_would_pass_it_but_not_a_big_file_within;
mod an_administrator_sees_a_members_unfiled_as_their_my_files;
mod an_agent_signs_in_with_its_token_until_it_is_removed;
mod an_agent_stores_against_its_creators_quota;
mod an_editor_changes_the_prefixes_and_tags_but_no_other_kind;
mod an_editor_sees_only_the_blocks_it_made_or_was_given;
mod an_endpoint_its_push_service_says_is_gone_is_dropped;
mod an_endpoint_on_a_host_taken_off_the_list_is_dropped_at_send;
mod an_endpoint_that_keeps_failing_for_a_day_is_dropped;
mod an_http_answer_leaves_what_would_overflow_it_for_the_next_post;
mod an_invitation_never_takes_the_id_of_a_contact_link;
mod an_invited_account_joins_the_organization_and_a_logout_ends_the_session;
mod an_object_uploads_and_downloads_with_presigned_s3_urls;
mod an_s3_upload_that_is_not_its_key_stores_nothing;
mod an_unauthenticated_connection_cannot_touch_blocks;
mod an_unauthenticated_connection_is_closed_unless_it_signs_in;
mod an_upload_is_used_once_and_keeps_its_object_until_collected;
mod anonymous_mail_requests_never_fill_the_global_cap;
mod another_account_cannot_reach_a_block_by_its_id_or_its_objects_by_their_hashes;
mod another_build_is_greeted_and_turned_away;
mod block_sizes_add_up_their_subtrees_and_follow_moves;
mod blocks_publish_and_read_back_from_a_bucket;
mod blocks_publish_and_read_back_through_the_server;
mod browser_forms_enforce_origin_policy_and_deny_sign_in;
mod browser_pages_escape_codes_and_keep_available_actions;
mod clients_not_signed_in_hold_only_their_share;
mod closing_a_kind_rotates_it_for_who_read_it_inside;
mod contacts_are_made_by_a_link_and_finished_by_its_owner;
mod email_changes_wait_for_verification_and_notify_the_old_address;
mod every_call_states_who_may_make_it;
mod every_graph_change_advances_a_blocks_version;
mod every_member_connection_hears_how_the_graph_changes;
mod expired_mail_and_browser_sessions_fail_and_mail_issuance_is_bounded;
mod fcm_sends_a_data_message_with_the_service_accounts_token;
mod history_records_each_commits_parents;
mod key_rows_are_served_only_to_who_may_see_their_block;
mod local_profiles_need_only_a_name_and_are_unavailable_on_remote_servers;
mod losing_every_block_inside_rotates_a_kind_open_inside;
mod mail_links_are_bounded_per_address_and_unknown_resets_cost_the_same;
mod members_manage_only_the_agents_they_created;
mod metadata_travels_with_the_content_once_a_block_has_any;
mod moving_a_block_drops_the_wraps_of_its_old_scope;
mod moving_a_block_out_of_what_is_shared_rotates_its_keys;
mod nobody_makes_or_writes_to_another_accounts_unfiled;
mod objects_a_block_holds_outlive_its_commits_until_it_is_collected;
mod objects_are_served_only_from_blocks_the_reader_can_see;
mod old_database_backups_thin_out_by_age;
mod one_address_holds_only_so_many_connections_and_http_sessions;
mod one_shot_calls_only_sign_in_and_are_rate_limited;
mod only_a_manager_lets_go_of_a_blocks_settings;
mod only_a_manager_shares_and_a_lower_grant_reduces_nothing;
mod only_members_who_may_view_and_did_not_mute_a_block_are_notified;
mod pairing_messages_reach_only_the_same_accounts_other_connections;
mod password_changes_rotate_the_browser_and_revoke_other_sessions;
mod password_hashing_refuses_past_its_queue_and_each_addresss_share;
mod password_reset_is_generic_and_revokes_live_sessions_and_pending_approvals;
mod passwords_are_validated_on_the_server_without_truncation_or_echoing;
mod proxy_account_authentication_never_discloses_the_token;
mod publishing_prunes_the_commit_it_replaces_but_keeps_bookmarks_and_merge_parents;
mod references_keep_the_order_their_block_lists_them_in;
mod registering_joins_the_one_personal_organization_as_an_editor;
mod registration_checks_its_invite_code_before_hashing_the_password;
mod registration_needs_an_invite_code_spent_when_the_account_is_made;
mod registration_requires_verification_and_links_are_purpose_bound_and_single_use;
mod relayed_session_traffic_passes_through_the_server_sealed;
mod removing_a_contact_revokes_what_the_two_shared;
mod repeated_failed_sign_ins_lock_the_account_out;
mod requests_sent_before_a_close_are_still_carried_out;
mod revoking_a_session_announces_only_a_change;
mod revoking_an_http_session_removes_its_access;
mod rotating_an_agents_token_revokes_every_older_one;
mod senders_are_capped_overall_and_per_push_host;
mod sessions_are_scoped_to_the_account_and_form_tokens_to_the_browser;
mod shared_chunks_survive_until_the_last_commit_releases_them;
mod sign_in_failures_lock_a_pair_before_an_address_or_an_email_and_decay;
mod signing_keys_are_served_only_for_accounts_the_caller_knows;
mod the_access_list_leaves_administrators_to_the_organization_line;
mod the_browser_authorization_requires_the_private_secret_and_works_once;
mod the_portal_serves_its_assets_immutably_beside_a_hydrating_page;
mod the_session_registry_forgets_the_longest_empty_sessions_first;
mod the_trees_owner_pays_for_what_anyone_makes_in_it;
mod unsigned_or_expired_s3_requests_are_refused;
mod uploads_no_block_takes_are_collected_after_their_grace;
mod what_one_account_keeps_on_the_server_is_bounded;

const CONTENT: Uuid = Uuid::from_u128(0x7465_7874);
const KEY: Uuid = Uuid::from_u128(0x006b_6579);

const BUILD: &str = "a test build";

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

    async fn client(&self) -> TestClient {
        let mut client = TestClient::connect(&self.socket()).await;
        client.directory = Some(self.directory.clone());
        client
    }

    async fn greeted(&self, build: &str) -> Socket {
        greeted(&self.socket(), build).await
    }

    async fn member(&self, email: &str) -> TestClient {
        let mut client = self.client().await;
        client.register(email).await;
        client
    }

    async fn second_connection(&self, token: &str, organization: Uuid) -> TestClient {
        let mut client = self.client().await;
        let token = token.to_owned();
        let response = client
            .call(|peer| client::account::authenticate(peer, Some(token)))
            .await;
        assert!(response.is_ok(), "{response:?}");
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

#[derive(Clone, Debug, PartialEq)]
enum Notice {
    BlockChanged {
        block: BlockSummary,
    },
    BlockRemoved {
        block: Uuid,
        revision: u64,
    },
    HeadChanged {
        block: Uuid,
        head: CommitId,
        author: Uuid,
    },
    BlockDeleted {
        block: Uuid,
    },
    SessionChanged {
        block: Uuid,
        state: SessionState,
    },
    Relayed {
        block: Uuid,
        from: ClientId,
        payload: Vec<u8>,
    },
    Paired {
        from: ClientId,
        payload: Vec<u8>,
    },
    ContactsChanged,
    KeyWorkChanged,
}

enum Heard {
    Changes(Stream<BlockChange>),
    Paired(Stream<PairingMessage>),
    Contacts(Stream<()>),
    KeyWork(Stream<()>),
    Watch(Uuid, Stream<BlockEvent>),
    Session(Uuid, Stream<SessionEvent>),
}

impl Heard {
    fn poll(&mut self, context: &mut std::task::Context<'_>) -> std::task::Poll<Option<Notice>> {
        use std::task::Poll;
        match self {
            Heard::Changes(stream) => stream.poll_recv(context).map(|change| {
                change.map(|change| match change {
                    BlockChange::Changed(block) => Notice::BlockChanged { block },
                    BlockChange::Removed { block, revision } => {
                        Notice::BlockRemoved { block, revision }
                    }
                })
            }),
            Heard::Paired(stream) => stream.poll_recv(context).map(|message| {
                message.map(|PairingMessage { from, payload }| Notice::Paired { from, payload })
            }),
            Heard::Contacts(stream) => stream
                .poll_recv(context)
                .map(|changed| changed.map(|()| Notice::ContactsChanged)),
            Heard::KeyWork(stream) => stream
                .poll_recv(context)
                .map(|changed| changed.map(|()| Notice::KeyWorkChanged)),
            Heard::Watch(block, stream) => {
                let block = *block;
                stream.poll_recv(context).map(|event| {
                    event.map(|event| match event {
                        BlockEvent::Moved { head, author } => Notice::HeadChanged {
                            block,
                            head,
                            author,
                        },
                        BlockEvent::Deleted => Notice::BlockDeleted { block },
                    })
                })
            }
            Heard::Session(block, stream) => {
                let block = *block;
                loop {
                    return match stream.poll_recv(context) {
                        Poll::Ready(Some(SessionEvent::Changed(state))) => {
                            Poll::Ready(Some(Notice::SessionChanged { block, state }))
                        }
                        Poll::Ready(Some(SessionEvent::Relayed { from, payload })) => {
                            Poll::Ready(Some(Notice::Relayed {
                                block,
                                from,
                                payload,
                            }))
                        }
                        Poll::Ready(Some(_)) => continue,
                        other => other.map(|_| None),
                    };
                }
            }
        }
    }
}

struct TestClient {
    directory: Option<PathBuf>,
    peer: Peer,
    heard: Vec<Heard>,
    listening: bool,
    /// Every method this client started a call of.
    started: Arc<std::sync::Mutex<std::collections::BTreeSet<be_wasm_ops::MethodId>>>,
    sessions: std::collections::HashMap<Uuid, client::sessions::Session>,
    notifications: Vec<Notice>,
    token: String,
    closing: Option<oneshot::Sender<()>>,
    pump: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for TestClient {
    fn drop(&mut self) {
        self.closing.take();
    }
}

impl TestClient {
    async fn connect(url: &str) -> Self {
        let mut socket = greeted(url, BUILD).await;
        assert_eq!(
            next_binary(&mut socket)
                .await
                .as_deref()
                .and_then(read_hello)
                .as_deref(),
            Some(BUILD),
            "the server did not greet a client of its own build"
        );
        Self::over(socket)
    }

    fn over(socket: Socket) -> Self {
        let (sending, outbound) = tokio::sync::mpsc::unbounded_channel();
        let started = Arc::new(std::sync::Mutex::new(std::collections::BTreeSet::new()));
        let starting = Arc::clone(&started);
        let peer = Peer::new(be_wasm_ops::Side::Guest, move |frame| {
            if let Ok(be_wasm_ops::Frame::Start { method, .. }) = be_wasm_ops::decode_frame(&frame)
            {
                starting.lock().unwrap().insert(method);
            }
            let _ = sending.send(frame);
        });
        let (closing, closed) = oneshot::channel();
        let pump = tokio::spawn(pump(socket, peer.clone(), outbound, closed));
        Self {
            directory: None,
            peer,
            heard: Vec::new(),
            listening: false,
            started,
            sessions: std::collections::HashMap::new(),
            notifications: Vec::new(),
            token: String::new(),
            closing: Some(closing),
            pump: Some(pump),
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

    /// Makes a call; once one signs the connection in, opens the streams every client follows,
    /// which the server refuses before then.
    async fn call<T: 'static, E: be_wasm_ops::wire::Decode<Result<T, Error>>>(
        &mut self,
        start: impl FnOnce(&Peer) -> Pending<Result<T, Error>, E>,
    ) -> Result<T, Error> {
        let answered = start(&self.peer).await.expect("the connection stayed open");
        if let Ok(answer) = &answered
            && (answer as &dyn std::any::Any).is::<Authenticated>()
        {
            self.listen();
        }
        self.collect();
        answered
    }

    fn listen(&mut self) {
        if std::mem::replace(&mut self.listening, true) {
            return;
        }
        let peer = &self.peer;
        self.heard.extend([
            Heard::Changes(client::blocks::changes(peer)),
            Heard::Paired(client::account::paired(peer)),
            Heard::Contacts(client::contacts::contacts_changed(peer)),
            Heard::KeyWork(client::keys::key_work_changed(peer)),
        ]);
    }

    async fn round_trip(&mut self) {
        client::connection::ping(&self.peer)
            .await
            .expect("the connection stayed open");
        self.collect();
    }

    fn collect(&mut self) {
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let mut index = 0;
        while index < self.heard.len() {
            match self.heard[index].poll(&mut context) {
                std::task::Poll::Ready(Some(notice)) => self.notifications.push(notice),
                std::task::Poll::Ready(None) => {
                    self.heard.remove(index);
                }
                std::task::Poll::Pending => index += 1,
            }
        }
    }

    async fn watch(&mut self, block: Uuid) {
        self.heard.push(Heard::Watch(
            block,
            client::blocks::watch(&self.peer, block),
        ));
        self.round_trip().await;
    }

    async fn try_join(&mut self, block: Uuid) -> Result<(ClientId, SessionState), Error> {
        let session = client::sessions::Session::new(&self.peer, block);
        let mut events = session.join();
        let joined = match events.recv().await {
            Some(SessionEvent::Joined { client, state, .. }) => Ok((client, state)),
            Some(SessionEvent::Refused(error)) => Err(error),
            other => panic!("joining answered {other:?}"),
        };
        if joined.is_ok() {
            self.sessions.insert(block, session);
            self.heard.push(Heard::Session(block, events));
        }
        self.collect();
        joined
    }

    async fn join_session(&mut self, block: Uuid) -> (ClientId, SessionState) {
        self.try_join(block).await.expect("joining failed")
    }

    async fn heartbeat(
        &mut self,
        block: Uuid,
        generation: u64,
        clean_at: Option<CommitId>,
    ) -> Result<SessionState, Error> {
        let beat = self.sessions[&block].heartbeat(generation, clean_at);
        let answered = beat.await.expect("the connection stayed open");
        self.collect();
        answered
    }

    async fn relay(
        &mut self,
        block: Uuid,
        to: Option<ClientId>,
        payload: Vec<u8>,
    ) -> Result<(), Error> {
        let relayed = self.sessions[&block].relay(to, payload);
        let answered = relayed.await.expect("the connection stayed open");
        self.collect();
        answered
    }

    async fn notification(&mut self) -> Notice {
        loop {
            let next = self.next_notification().await;
            if !matches!(
                next,
                Notice::BlockChanged { .. } | Notice::BlockRemoved { .. }
            ) {
                return next;
            }
        }
    }

    async fn graph_change(&mut self) -> Notice {
        loop {
            let next = self.next_notification().await;
            if matches!(
                next,
                Notice::BlockChanged { .. } | Notice::BlockRemoved { .. }
            ) {
                return next;
            }
        }
    }

    async fn next_notification(&mut self) -> Notice {
        self.collect();
        if !self.notifications.is_empty() {
            return self.notifications.remove(0);
        }
        std::future::poll_fn(|context| {
            let mut index = 0;
            while index < self.heard.len() {
                match self.heard[index].poll(context) {
                    std::task::Poll::Ready(Some(notice)) => {
                        return std::task::Poll::Ready(notice);
                    }
                    std::task::Poll::Ready(None) => {
                        self.heard.remove(index);
                    }
                    std::task::Poll::Pending => index += 1,
                }
            }
            std::task::Poll::Pending
        })
        .await
    }

    async fn register(&mut self, email: &str) -> Uuid {
        let (_, token) = ServerStore::open(self.directory.as_ref().unwrap())
            .unwrap()
            .register(email, email, "correct horse battery")
            .await
            .unwrap();
        let response = self
            .call(|peer| client::account::authenticate(peer, Some(token)))
            .await;
        let Ok(Authenticated { account, token, .. }) = response else {
            panic!("registration failed: {response:?}");
        };
        self.token = token;
        account
    }

    async fn organization(&mut self, name: &str) -> Uuid {
        let response = self
            .call(|peer| client::organizations::create_organization(peer, name.into()))
            .await;
        let Ok(organization) = response else {
            panic!("organization creation failed: {response:?}");
        };
        let organization = organization.id;
        self.open(organization).await;
        organization
    }

    async fn open(&mut self, organization: Uuid) {
        let response = self
            .call(|peer| client::organizations::open_organization(peer, organization))
            .await;
        assert!(response.is_ok(), "{response:?}");
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
        let response = self
            .call(|peer| {
                client::blocks::create_block(
                    peer,
                    block,
                    content_type,
                    parent,
                    Vec::new(),
                    Vec::new(),
                    key,
                    keys,
                    false,
                )
            })
            .await;
        assert!(response.is_ok(), "{response:?}");
    }

    async fn keys(&mut self, blocks: Option<Vec<Uuid>>) -> KeySet {
        self.call(|peer| client::keys::get_keys(peer, blocks))
            .await
            .expect("reading keys failed")
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
    async fn reserve(&mut self, objects: &[Vec<u8>]) -> Result<Vec<UploadUrl>, Error> {
        let lengths = objects
            .iter()
            .map(|bytes| ObjectLength {
                hash: Hash::of(bytes),
                length: bytes.len() as u64,
            })
            .collect();
        self.call(|peer| client::objects::upload_urls(peer, lengths))
            .await
    }

    /// Uploads `objects` as a client does: asks for URLs, then PUTs each one's bytes.
    async fn put(&mut self, objects: Vec<Vec<u8>>) -> Result<(), Error> {
        let urls = self.reserve(&objects).await?;
        assert_eq!(urls.len(), objects.len());
        for (url, bytes) in urls.into_iter().zip(objects) {
            let status = put_to(url, bytes).await;
            assert_eq!(status, 200, "the upload was refused");
        }
        Ok(())
    }

    /// Downloads an object as a client does: asks for its URL, then GETs it. None when the
    /// server answers no URL; a URL that is not found is a failure.
    async fn download(&mut self, hash: Hash) -> Result<Option<Vec<u8>>, Error> {
        let mut urls = self
            .call(|peer| client::objects::download_urls(peer, vec![hash]))
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
        self.call(|peer| client::blocks::read_block(peer, block))
            .await
            .expect("read failed")
    }

    async fn list(&mut self, parent: BlockParent) -> Vec<BlockSummary> {
        self.call(|peer| client::blocks::list_blocks(peer, Listing::Children(parent)))
            .await
            .expect("listing failed")
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
        self.upload(&author.store, &author.objects_of(commit, &chunks))
            .await;
        let before = author.held_by(expected);
        self.call(|peer| {
            client::blocks::publish(
                peer,
                block,
                commit,
                expected,
                expected.into_iter().collect(),
                objects_change(&before, chunks),
                time,
                false,
                listing(&[], Vec::new()),
                Vec::new(),
                Vec::new(),
            )
        })
        .await
    }

    async fn history(&mut self, block: Uuid) -> Vec<HistoryEntry> {
        self.call(|peer| client::blocks::list_history(peer, block))
            .await
            .expect("history failed")
    }
}

/// PUTs `bytes` to an upload URL with its headers, answering the status.
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
                client::blocks::publish(
                    peer,
                    parent,
                    head,
                    None,
                    Vec::new(),
                    objects_change(&[], chunks),
                    1_000,
                    false,
                    listing(&[], children),
                    Vec::new(),
                    Vec::new(),
                )
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
                    keys.insert(KEY, ContentKey::from_bytes([5; 32]));
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

async fn greeted(url: &str, build: &str) -> Socket {
    let (mut socket, _) = connect_async(url).await.unwrap();
    socket.send(WsMessage::Binary(hello(build))).await.unwrap();
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
            .call(|peer| client::organizations::list_organizations(peer))
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
            .call(|peer| {
                client::contacts::create_contact_link(
                    peer,
                    link,
                    b"the owner's card".to_vec(),
                    b"the code, sealed for the owner".to_vec(),
                    grant,
                )
            })
            .await;
        assert!(response.is_ok(), "{response:?}");
        link
    }

    async fn accept(&mut self, link: Uuid, key: &[u8]) -> Result<(), Error> {
        self.call(|peer| {
            client::contacts::accept_contact_link(
                peer,
                link,
                b"the accepter's card".to_vec(),
                b"the owner's key, sealed for the accepter".to_vec(),
                key.to_vec(),
            )
        })
        .await
    }

    async fn contacts(&mut self) -> Vec<be_block_protocol::ContactRow> {
        self.call(|peer| client::contacts::list_contacts(peer))
            .await
            .expect("listing contacts failed")
    }

    /// Makes an agent bound to `block`; answers its account and token.
    async fn block_agent(
        &mut self,
        organization: Uuid,
        block: Uuid,
    ) -> Result<(Uuid, String), Error> {
        self.call(|peer| {
            client::members::create_agent(
                peer,
                organization,
                "Terminal host".into(),
                [7; 32],
                [8; 32],
                Vec::new(),
                Vec::new(),
                Some(block),
            )
        })
        .await
        .map(|created| (created.agent.account, created.token))
    }

    /// Whether `token` still signs a new connection in.
    async fn signs_in(harness: &Harness, token: &str) -> bool {
        let mut client = harness.client().await;
        let token = token.to_owned();
        client
            .call(|peer| client::account::authenticate(peer, Some(token)))
            .await
            .is_ok()
    }

    async fn move_to(&mut self, block: Uuid, parent: BlockParent) -> Result<(), Error> {
        self.call(|peer| client::blocks::set_parent(peer, block, parent, KeySet::default()))
            .await
    }

    async fn grant(
        &mut self,
        block: Uuid,
        account: Uuid,
        access: be_block_graph::Access,
    ) -> Result<(), Error> {
        self.call(|peer| client::blocks::set_access(peer, block, account, access))
            .await
    }
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
        .once(|peer| client::account::begin_browser_login(peer))
        .await
        .expect("a browser authorization session starts");
    (begun.code, begun.secret)
}

/// Polls a browser sign-in once, as a one-shot call.
async fn poll_login(
    harness: &Harness,
    code: &str,
    secret: &str,
) -> Result<be_block_protocol::BrowserLoginState, Error> {
    let (code, secret) = (code.to_owned(), secret.to_owned());
    harness
        .once(|peer| client::account::poll_browser_login(peer, code, secret))
        .await
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
    async fn once<T: 'static, E: be_wasm_ops::wire::Decode<Result<T, Error>>>(
        &self,
        start: impl FnOnce(&Peer) -> Pending<Result<T, Error>, E>,
    ) -> Result<T, Error> {
        let (sending, mut sent) = tokio::sync::mpsc::unbounded_channel();
        let peer = Peer::new(be_wasm_ops::Side::Guest, move |frame| {
            let _ = sending.send(frame);
        });
        let mut pending = start(&peer);
        let _ = poll(&mut pending);
        let call = sent.try_recv().expect("the call was started");
        let (status, answer) =
            post_bytes(self, "/api/be/call", &framed(&[hello(BUILD), call])).await;
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&answer));
        let frames = unframed(&answer);
        assert_eq!(read_hello(&frames[0]).as_deref(), Some(BUILD));
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
    password: &str,
    organization: &str,
) -> Result<(Uuid, Uuid), ServerError> {
    let store = ServerStore::open(directory)?;
    let (profile, _) = store.register(email, name, password).await?;
    let company = store
        .create_organization(profile.account, organization)
        .await?;
    Ok((profile.account, company.id))
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
        let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
        stream.write_all(format!("{method} {path} HTTP/1.1\r\nHost: {address}\r\nOrigin: http://{address}\r\nCookie: be3-account={}\r\nContent-Length: {}\r\nContent-Type: application/x-www-form-urlencoded\r\nConnection: close\r\n\r\n{body}", self.cookie, body.len()).as_bytes()).await.unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.unwrap();
        let response = String::from_utf8(bytes).unwrap();
        if let Some(cookie) = response.split("Set-Cookie: be3-account=").nth(1) {
            self.cookie = cookie.split(';').next().unwrap().into();
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
    async fn login(&mut self, harness: &Harness, email: &str, password: &str) {
        let (status, page) = self
            .submit(
                harness,
                "/auth/login",
                "/auth/login",
                &[("email", email), ("password", password)],
            )
            .await;
        assert_eq!(status, 303, "{page}");
        assert!(!self.cookie.is_empty());
    }
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
            .call(|peer| client::notifications::register_endpoint(peer, endpoint))
            .await;
        assert!(registered.is_ok(), "{registered:?}");
    }

    async fn set_notify_mode(&mut self, block: Uuid, mode: be_block_protocol::NotifyMode) {
        let set = self
            .call(|peer| client::notifications::set_notify_mode(peer, block, mode))
            .await;
        assert!(set.is_ok(), "{set:?}");
    }

    async fn notify(&mut self, block: Uuid, sealed: &[u8], mentions: Vec<Uuid>) {
        let sealed = sealed.to_vec();
        let notified = self
            .call(|peer| client::notifications::notify(peer, block, sealed, mentions))
            .await;
        assert!(notified.is_ok(), "{notified:?}");
    }

    /// Signs out, which leaves the server's record of who is connected before it answers.
    async fn sign_out(&mut self) {
        let logged_out = self.call(|peer| client::account::logout(peer)).await;
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
    let mut member = harness.client().await;
    let account = member.register(email).await;
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
