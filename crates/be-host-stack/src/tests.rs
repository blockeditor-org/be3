use std::time::Duration;

use be_block::{BlockContent, LiveEdit};
use be_type_checklist::{Checklist, ChecklistContent};
use be_type_example_counter::{Counter, CounterContent};
use uuid::Uuid;

use super::*;
use crate::embedded_server;

mod a_block_a_shared_one_links_to_shows_as_no_access_and_nowhere_else;
mod a_block_another_build_saved_is_offered_for_reset_and_a_held_one_is_reset;
mod a_block_deleted_from_unfiled_remembers_it_and_apps_never_see_its_id;
mod a_block_file_brings_a_tree_back_as_a_copy;
mod a_block_its_parent_stops_listing_is_deleted_until_it_is_listed_again;
mod a_block_named_before_it_arrives_is_created_with_that_name;
mod a_block_opens_from_what_this_device_saved_before_its_session_is_joined;
mod a_block_shared_with_a_new_agent_at_once_is_sealed_to_it;
mod a_block_with_a_plain_form_is_read_as_a_named_file;
mod a_branch_is_combined_into_another;
mod a_catch_up_moves_its_watermark_only_past_blocks_that_were_put;
mod a_chat_message_notifies_the_members_away_who_want_it_through_the_server;
mod a_chat_message_sent_offline_into_a_closed_chat_notifies_once_as_the_outbox_drains;
mod a_chat_message_sent_with_two_editors_open_notifies_once;
mod a_chat_message_told_before_a_restart_is_not_told_again;
mod a_chat_message_unsent_across_a_restart_notifies_once_after_reconnecting;
mod a_checklist_and_a_counter_are_held_by_one_peer;
mod a_child_moved_into_a_block_is_added_to_its_content;
mod a_child_reordered_in_a_folder_is_listed_in_its_new_place;
mod a_commit_in_one_checkout_is_brought_into_another;
mod a_conflicting_edit_keeps_every_side_until_one_is_chosen;
mod a_counter_lives_in_the_new_stack_and_survives_a_reconnect;
mod a_defined_index_keeps_the_host_busy_until_its_root_is_read_again;
mod a_deleted_block_remembers_where_it_was_until_it_is_restored;
mod a_duplicated_block_carries_what_its_source_held;
mod a_folder_lists_a_new_child_before_its_edit_is_sealed;
mod a_fork_pulls_and_pushes_through_its_upstream;
mod a_half_open_connection_is_noticed_and_the_peer_reconnects;
mod a_heartbeat_the_server_never_answers_holds_up_no_edit;
mod a_held_block_stays_open_when_its_editors_close;
mod a_link_carrying_a_share_opens_the_block_the_moment_it_is_accepted;
mod a_merge_conflict_shows_both_versions_and_keeps_the_one_chosen;
mod a_new_device_gets_the_identity_secret_by_typing_its_code_on_an_open_one;
mod a_part_an_editor_puts_is_held_for_its_block_once_the_server_has_it;
mod a_paste_too_large_to_relay_is_acknowledged_to_its_editor;
mod a_peer_rejoins_what_was_open_when_the_server_comes_back;
mod a_peers_edit_during_a_gesture_here_reaches_editors_beneath_it;
mod a_picked_file_becomes_a_block_of_the_type_that_claims_it;
mod a_restore_on_a_follower_keeps_the_state_it_replaced;
mod a_saved_change_is_undone_and_later_edits_stay;
mod a_seal_the_connection_loses_is_kept_and_published_after_reconnect;
mod a_search_index_is_tried_again_when_the_reason_it_failed_has_passed;
mod a_search_index_made_before_the_server_has_its_root_is_made_again_without_an_error;
mod a_second_checkout_copies_the_tree_under_the_same_local_ids;
mod a_server_gone_silent_while_contacts_sync_holds_up_no_open_or_edit;
mod a_session_owned_here_follows_a_peer_that_claims_it;
#[cfg(not(target_os = "android"))]
mod a_terminal_host_answers_this_peer_through_the_blocks_session;
#[cfg(not(target_os = "android"))]
mod a_terminal_host_takes_over_a_session_this_peer_opened_first;
mod a_terminal_hosts_be_is_the_deployments_or_one_built_here;
mod a_transient_version_failure_is_retried_on_the_backoff_and_on_reconnecting;
mod a_type_module_that_stops_is_started_again_with_the_edits_it_had_not_sealed;
mod a_version_status_that_failed_waits_for_its_inputs_or_its_kind_of_news;
mod an_account_unlocks_with_its_recovery_phrase;
mod an_apps_signature_verifies_and_a_forged_one_does_not;
mod an_edit_counts_as_unsaved_from_when_it_is_sent_until_it_is_sealed;
mod an_edit_in_a_block_opened_offline_is_lacking_until_it_is_published;
mod an_edit_made_across_a_takeover_is_kept;
mod an_edit_made_offline_is_lacking_until_the_drain_publishes_it;
mod an_edit_made_offline_survives_a_restart_and_reaches_the_server_on_reconnect;
mod an_edit_made_offline_to_a_note_changed_elsewhere_is_held_once_the_merge_publishes;
mod an_empty_search_request_is_not_a_change;
mod an_idle_peer_never_wakes_its_worker;
mod an_imported_tree_shows_at_once_and_moves_once_it_is_uploaded;
mod an_invite_link_joins_the_organization_and_makes_the_two_contacts;
mod an_organizations_unsent_edits_are_discarded_once_the_grace_period_passes;
mod an_owner_cut_off_before_it_commits_leaves_its_edits_to_the_next_owner;
mod an_unknown_content_type_has_no_content_in_the_new_stack;
mod backing_off_waits_longer_after_each_failure_up_to_a_ceiling;
mod blocks_moved_renamed_and_deleted_offline_reach_the_server_on_reconnect;
mod closing_a_block_while_the_server_is_silent_holds_up_no_edit;
mod flushing_seals_what_the_sessions_hold_and_leaves_them_live;
mod news_of_an_older_head_after_a_flush_keeps_the_head_the_replica_reported;
mod presence_crosses_between_this_peer_and_another;
mod removing_an_agent_the_server_no_longer_has_succeeds;
mod replacing_content_reaches_a_block_open_or_not;
mod revision_history_previews_and_restores_saved_content;
mod rewrites_of_one_line_on_two_offline_devices_merge_with_a_conflict;
mod stopping_returns_while_a_connect_goes_unanswered;
mod taking_a_share_back_rotates_its_keys_and_reseals_the_open_block;
mod the_host_catches_up_what_other_devices_wrote_and_masks_what_left_the_tree;
mod the_search_index_queue_forgets_what_can_no_longer_be_indexed;
mod the_search_text_cap_cuts_on_a_character;
mod two_accounts_become_contacts_by_a_link_and_share_a_block;
mod two_devices_that_edit_one_note_offline_merge_once_both_reconnect;
mod two_peers_of_one_organization_share_a_counter;
mod typing_shows_while_a_silent_server_holds_a_seal_and_stopping_keeps_it_here;
mod undo_steps_back_through_what_this_peer_did;
mod what_is_typed_is_found_through_the_workspaces_search_index;

struct Editor<C: LiveEdit> {
    block: Uuid,
    id: u64,
    mirror: be_sdk_type_guest::editor::EditorMirror<C>,
    received: Vec<be_sdk_type_guest::editor::ToEditor>,
}

impl<C: LiveEdit + Clone + Default> Editor<C> {
    fn attach(block: Uuid) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        attach(block, C::CONTENT_TYPE, id, true);
        let mut editor = Self {
            block,
            id,
            mirror: be_sdk_type_guest::editor::EditorMirror::default(),
            received: Vec::new(),
        };
        editor.send(be_sdk_type_guest::editor::FromEditor::Resend);
        editor
    }

    fn send(&mut self, message: be_sdk_type_guest::editor::FromEditor) {
        from_editor(
            self.block,
            self.id,
            &be_sdk_type_api::encode_frame(&message),
        );
    }

    fn pump(&mut self) {
        for (block, frame) in take_editor_frames(self.id) {
            if block != self.block {
                continue;
            }
            let Some(message) = unframe(&frame)
                .and_then(|bytes| be_sdk_type_guest::editor::ToEditor::decode(&bytes))
            else {
                continue;
            };
            self.received.push(message.clone());
            if let Some(answer) = self.mirror.receive(message) {
                self.send(answer);
            }
        }
    }

    fn wait(&mut self, _what: &str, done: impl Fn(&C) -> bool) {
        let id = self.id;
        loop {
            self.pump();
            if self.mirror.content().is_some_and(&done) {
                return;
            }
            crate::wait_until(|shared| shared.editor_frames.contains_key(&id).then_some(()));
        }
    }

    /// Waits for frames until `done` holds of the editor.
    fn until(&mut self, _what: &str, done: impl Fn(&Self) -> bool) {
        let id = self.id;
        loop {
            self.pump();
            if done(self) {
                return;
            }
            crate::wait_until(|shared| shared.editor_frames.contains_key(&id).then_some(()));
        }
    }

    fn content(&self) -> &C {
        self.mirror
            .content()
            .expect("the editor was sent its content")
    }

    fn operate(&mut self, operation: C::Op) {
        let message = self
            .mirror
            .operate(operation)
            .expect("an editor operates once it has content");
        self.send(message);
    }

    fn take_received(&mut self) -> Vec<be_sdk_type_guest::editor::ToEditor> {
        self.pump();
        std::mem::take(&mut self.received)
    }
}

impl<C: LiveEdit> Drop for Editor<C> {
    fn drop(&mut self) {
        detach(self.block, self.id);
    }
}

pub struct Harness {
    directory: PathBuf,
    server: Option<embedded_server::EmbeddedServer>,
    url: String,
    account: Uuid,
    token: String,
    organization: Uuid,
    phrase: be_block_keys::RecoveryPhrase,
    lease: Duration,
    push: be_server::push::PushConfig,
    relay: Option<be_block_fault_relay::Relay>,
    clock: be_block_client::Clock,
    _one_stack: std::sync::MutexGuard<'static, ()>,
}

impl Harness {
    pub fn start() -> Self {
        Self::leasing(be_server::ServerConfig::OPEN.lease)
    }

    /// A harness whose server pushes to Web Push endpoints on loopback (`PushService`).
    fn pushing() -> Self {
        let mut harness = Self::start();
        harness.push = be_server::push::PushConfig {
            allow_local_endpoints: true,
            fcm: None,
            hosts: vec!["127.0.0.1".to_owned()].into(),
        };
        let address = harness.address();
        harness.stop_server();
        harness.start_server(&address);
        harness
    }

    pub fn lapsing() -> Self {
        Self::leasing(Duration::ZERO)
    }

    fn relayed() -> Self {
        let mut harness = Self::start();
        let server = harness
            .address()
            .parse()
            .expect("the embedded server listens on an address");
        harness.relay = Some(be_block_fault_relay::Relay::start(server));
        harness
    }

    fn on_manual_clock(mut self) -> Self {
        self.clock = be_block_client::Clock::manual();
        self
    }

    fn relay(&self) -> &be_block_fault_relay::Relay {
        self.relay
            .as_ref()
            .expect("the harness was started with a relay")
    }

    fn open_until_it_joins(&self, block: Uuid, content_type: Uuid) {
        let mut joins = self
            .relay()
            .count(|method| method == be_block_protocol::__args::SESSIONS_SESSION_JOIN);
        open(block, content_type);
        runtime()
            .block_on(joins.wait_for(|joins| *joins > 0))
            .expect("opening the block asked to join its session");
    }

    fn miss_a_ping(&self) {
        let mut due = crate::wait_until(|shared| {
            shared
                .connected
                .then(|| shared.connection_due.clone())
                .flatten()
        });
        runtime().block_on(async {
            loop {
                let at = *due.borrow_and_update();
                let now = self.clock.now();
                if at > now {
                    self.clock.advance(at - now);
                }
                if due.changed().await.is_err() {
                    return;
                }
            }
        });
        wait_until("noticed the connection was gone", |shared| {
            !shared.connected
        });
    }

    fn leasing(lease: Duration) -> Self {
        static ONE_STACK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let one_stack = ONE_STACK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        be_lib_build_id::install("test");
        let directory = std::env::temp_dir().join(format!("be-host-be-test-{}", Uuid::new_v4()));
        let runtime = runtime();
        let (profile, token) = runtime.block_on(async {
            be_server::ServerStore::open(directory.join("server"))
                .unwrap()
                .register("counter@example.com", "Counter", "correct horse battery")
                .await
                .unwrap()
        });
        let session = crate::accounts::Session {
            account: profile.account,
            email: profile.email,
            display_name: profile.display_name,
            token,
        };
        let server = embedded_server::start_embedded_server_at(
            "127.0.0.1:0",
            directory.join("server"),
            lease,
            be_server::push::PushConfig::DEFAULT,
        )
        .expect("the embedded block server starts");
        let url = server.url.clone();
        let managed = url.clone();
        let (account, token, organization) = runtime.block_on(async {
            let organization = crate::accounts::create_organization(
                managed,
                session.token.clone(),
                "Counters".to_owned(),
            )
            .await
            .expect("the organization is created");
            (session.account, session.token, organization.id)
        });
        Self {
            directory,
            server: Some(server),
            url,
            account,
            token,
            organization,
            phrase: be_block_keys::RecoveryPhrase::generate(),
            lease,
            push: be_server::push::PushConfig::DEFAULT,
            relay: None,
            clock: be_block_client::Clock::real(),
            _one_stack: one_stack,
        }
    }

    fn address(&self) -> String {
        self.url
            .split_once("://")
            .map(|(_, host)| host.to_owned())
            .expect("the embedded server url has a scheme")
    }

    fn stop_server(&mut self) {
        self.server = None;
    }

    fn start_server(&mut self, address: &str) {
        self.server = Some(
            embedded_server::start_embedded_server_at(
                address,
                self.directory.join("server"),
                self.lease,
                self.push.clone(),
            )
            .expect("the embedded block server starts again"),
        );
    }

    pub fn connect(&self) {
        start(self.config(self.directory.join("objects")));
    }

    fn config(&self, data_dir: PathBuf) -> Config {
        Config {
            server_url: match &self.relay {
                Some(relay) => format!("http://{}", relay.address()),
                None => self.url.clone(),
            },
            token: self.token.clone(),
            account: self.account,
            organization: self.organization,
            identity: self.identity(),
            display_name: "Counter".to_owned(),
            email: "counter@example.com".to_owned(),
            data_dir,
            clock: self.clock.clone(),
        }
    }

    fn identity(&self) -> [u8; 32] {
        self.phrase.secret().to_bytes()
    }

    fn outside_config(&self) -> be_block_client::PeerConfig {
        be_block_client::PeerConfig::new(
            crate::accounts::socket_url(&self.url),
            be_block_keys::RecoverySecret::from_bytes(self.identity()),
            be_block_client::Credentials::Token(self.token.clone()),
        )
        .organization(Some(self.organization))
    }

    async fn outside_peer(
        &self,
    ) -> std::sync::Arc<be_block_client::Peer<be_block_store::MemoryStore>> {
        outside_peer(self.outside_config()).await
    }
}

async fn outside_peer(
    config: be_block_client::PeerConfig,
) -> std::sync::Arc<be_block_client::Peer<be_block_store::MemoryStore>> {
    let peer = be_block_client::Peer::connect(config, be_block_store::MemoryStore::new())
        .await
        .expect("a second peer joins the organization");
    std::sync::Arc::new(peer)
}

impl Drop for Harness {
    fn drop(&mut self) {
        be_block_commit::pin_now(None);
        if std::thread::panicking() {
            abandon();
        } else {
            stop_and_wait();
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

struct Outside {
    runtime: tokio::runtime::Runtime,
    local: tokio::task::LocalSet,
    session: Option<be_tool_cli::replica::Session<be_block_store::MemoryStore, CounterContent>>,
}

struct Seen {
    count: Option<i64>,
    presence: Vec<be_sdk_type_guest::editor::PeerPresence>,
}

impl Outside {
    fn counter(harness: &Harness, block: Uuid) -> Self {
        Self::open(harness, block, true)
    }

    fn joining(harness: &Harness, block: Uuid) -> Self {
        Self::open(harness, block, false)
    }

    fn open(harness: &Harness, block: Uuid, create: bool) -> Self {
        let runtime = runtime();
        let local = tokio::task::LocalSet::new();
        let config = harness.outside_config();
        let session = local.block_on(&runtime, async move {
            let peer = outside_peer(config).await;
            if create {
                peer.ensure::<CounterContent>(block, be_block_graph::BlockParent::Root)
                    .await
                    .expect("the counter is created");
            }
            be_tool_cli::replica::Session::open(peer, block, super::types::sources().await)
                .await
                .expect("the outside peer opens the counter")
        });
        Self {
            runtime,
            local,
            session: Some(session),
        }
    }

    fn session(
        &mut self,
    ) -> &mut be_tool_cli::replica::Session<be_block_store::MemoryStore, CounterContent> {
        self.session
            .as_mut()
            .expect("the outside peer has not left")
    }

    fn peer(&mut self) -> std::sync::Arc<be_block_client::Peer<be_block_store::MemoryStore>> {
        std::sync::Arc::clone(self.session().peer())
    }

    fn is_owner(&mut self) -> bool {
        self.session().is_owner()
    }

    fn claim(&mut self) {
        let Self {
            runtime,
            local,
            session,
        } = self;
        let session = session.as_mut().expect("the outside peer has not left");
        local.block_on(runtime, async {
            session.claim();
            while !session.is_owner() {
                let woken = session.next().await;
                let heard = matches!(woken, be_tool_cli::replica::Woken::Event(_));
                session
                    .handle(woken)
                    .await
                    .expect("the outside peer is still connected");
                if heard {
                    session.claim();
                }
            }
        });
    }

    fn add(&mut self, by: i64) {
        self.session().edit(Counter::add(by));
        let Self {
            runtime,
            local,
            session,
        } = self;
        let session = session.as_mut().expect("the outside peer has not left");
        local.block_on(runtime, session.settle());
    }

    fn show(&mut self, kind: Uuid, value: Option<Vec<u8>>) {
        self.session().show(kind, value);
        let Self {
            runtime,
            local,
            session,
        } = self;
        let session = session.as_mut().expect("the outside peer has not left");
        local.block_on(runtime, session.settle());
    }

    fn seal(&mut self) {
        let Self {
            runtime,
            local,
            session,
        } = self;
        let session = session.as_mut().expect("the outside peer has not left");
        local.block_on(runtime, session.seal());
    }

    fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.local.block_on(&self.runtime, future)
    }

    fn leave(&mut self) {
        let session = self.session.take().expect("the outside peer has not left");
        self.local.block_on(&self.runtime, session.close());
    }

    fn until(&mut self, _what: &str, ready: impl Fn(&Seen) -> bool) {
        let Self {
            runtime,
            local,
            session,
        } = self;
        let session = session.as_mut().expect("the outside peer has not left");
        local.block_on(runtime, async {
            loop {
                let seen = Seen {
                    count: session.content().map(|counter| counter.root().value()),
                    presence: session.presence().to_vec(),
                };
                if ready(&seen) {
                    return;
                }
                let woken = session.next().await;
                session
                    .handle(woken)
                    .await
                    .expect("the outside peer is still connected");
            }
        });
    }
}

fn count_of(block: Uuid) -> Option<i64> {
    let held = content(block)?;
    (held.content_type == CounterContent::CONTENT_TYPE)
        .then(|| CounterContent::decode(held.bytes()).ok())
        .flatten()
        .map(|counter| counter.root().value())
}

fn counted(shared: &Shared, block: Uuid) -> Option<i64> {
    let held = shared.blocks.get(&block)?;
    (held.content_type == CounterContent::CONTENT_TYPE)
        .then(|| CounterContent::decode(held.bytes()).ok())
        .flatten()
        .map(|counter| counter.root().value())
}

#[track_caller]
fn wait_for_count(block: Uuid, expected: i64) {
    crate::wait_until(|shared| (counted(shared, block) == Some(expected)).then_some(()));
}

fn ask_for(queries: Vec<Query>) {
    let owner = Owner::Editor(u64::MAX);
    let mut wanted = with_graph(|graph| graph.wanted(owner).to_vec()).unwrap_or_default();
    wanted.extend(queries);
    want(owner, wanted);
}

#[track_caller]
fn wait_until(_what: &str, ready: impl Fn(&Shared) -> bool) {
    crate::wait_until(|shared| ready(shared).then_some(()));
}

fn add(block: Uuid, by: i64) {
    operate(block, CounterContent::encode_operation(&Counter::add(by)));
}

fn version_status(shared: &Shared, block: Uuid) -> Option<&be_sdk_api::VersionStatus> {
    shared.versions.get(&block).map(|state| &state.status)
}

fn settled(shared: &Shared, block: Uuid) -> bool {
    version_status(shared, block).is_some_and(|status| !status.busy)
}

fn checkout_state(shared: &Shared, block: Uuid) -> Option<be_block::Checkout> {
    let held = shared.blocks.get(&block)?;
    be_block::CheckoutContent::decode(held.bytes())
        .ok()
        .map(|content| content.root())
}

fn text_of(shared: &Shared, block: Uuid) -> Option<String> {
    let held = shared.blocks.get(&block)?;
    be_type_text::TextContent::decode(held.bytes())
        .ok()
        .map(|text| be_type_text::TextBlock::text(&text))
}

const TYPIST: u64 = 1 << 63 | 7;

fn type_text(block: Uuid, at: usize, text: &str) {
    let mut editor = Editor::<be_type_text::TextContent>::attach(block);
    type_into(&mut editor, at, text);
}

fn type_into(editor: &mut Editor<be_type_text::TextContent>, at: usize, text: &str) {
    editor.wait("was sent the text", |_| true);
    let edit = be_type_text::TextBlock::insert(editor.content(), TYPIST, at, text.as_bytes())
        .expect("there is somewhere to type");
    editor.operate(edit);
}

fn checkouts(shared: &Shared) -> Vec<Uuid> {
    shared
        .graph
        .query(super::graph::Query::Roots)
        .into_iter()
        .filter(|node| node.content_type == be_block::CheckoutContent::CONTENT_TYPE)
        .map(|node| node.id)
        .collect()
}

struct Versioned {
    folder: Uuid,
    note: Uuid,
    repository: Uuid,
    checkout: Uuid,
}

fn start_versioning(text: &str) -> Versioned {
    wait_until("loaded the graph", |shared| {
        shared.graph.ready(super::graph::Query::Roots)
    });
    let (folder, note, repository) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    ask_for(vec![
        Query::Roots,
        Query::Parents(folder),
        Query::Subtree(folder),
    ]);
    let mut listing = be_type_workspace::FolderContent::default();
    listing.apply(&be_type_workspace::Folder::default().add(note));
    create(
        folder,
        be_type_workspace::FolderContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block::BlockMetadata::named("Notes"),
        Some(listing.encode()),
        false,
    );
    create(
        note,
        be_type_text::TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(folder),
        be_block::BlockMetadata::named("Note"),
        Some(be_type_text::TextBlock::of(text).encode()),
        false,
    );
    create(
        repository,
        be_block::RepositoryContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block::BlockMetadata::named("History"),
        Some(be_block::RepositoryContent::default().encode()),
        false,
    );
    version_since(repository, be_block::RepositoryContent::CONTENT_TYPE, None);
    version(
        repository,
        be_sdk_api::VersionCommand::Adopt {
            block_id: folder.into_bytes(),
        },
    );
    wait_until("versioned the folder", |shared| {
        settled(shared, repository)
            && version_status(shared, repository).is_some_and(|status| !status.branches.is_empty())
    });
    let checkout = node(folder)
        .and_then(|node| node.parent?.block())
        .expect("the folder moved into its checkout");
    Versioned {
        folder,
        note,
        repository,
        checkout,
    }
}

fn second_checkout(versioned: &Versioned) -> Uuid {
    version(
        versioned.repository,
        be_sdk_api::VersionCommand::NewCheckout {
            branch: be_block::MAIN_BRANCH.to_owned(),
        },
    );
    wait_until("made a second checkout", |shared| {
        settled(shared, versioned.repository) && checkouts(shared).len() == 2
    });
    let second = with_shared(checkouts)
        .unwrap_or_default()
        .into_iter()
        .find(|checkout| *checkout != versioned.checkout)
        .expect("a second checkout exists");
    ask_for(vec![Query::Subtree(second)]);
    let copied = copy_of(versioned.note, second);
    wait_until("listed the second checkout", |shared| {
        shared.graph.get(copied).is_some()
    });
    assert_eq!(
        with_shared(|shared| version_status(shared, versioned.repository)
            .and_then(|status| status.error.clone())),
        Some(None)
    );
    second
}

fn copy_of(block: Uuid, checkout: Uuid) -> Uuid {
    be_block::version_control::masked(block, be_block::version_control::scope_mask(checkout))
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts")
}

struct Someone {
    session: crate::accounts::Session,
    phrase: be_block_keys::RecoveryPhrase,
}

impl Someone {
    fn register(data: &std::path::Path, _url: &str, email: &str, name: &str) -> Self {
        let (profile, token) = runtime()
            .block_on(be_server::ServerStore::open(data).unwrap().register(
                email,
                name,
                "correct horse battery",
            ))
            .unwrap();
        let session = crate::accounts::Session {
            account: profile.account,
            email: profile.email,
            display_name: profile.display_name,
            token,
        };
        Self {
            session,
            phrase: be_block_keys::RecoveryPhrase::generate(),
        }
    }

    fn me(&self) -> crate::accounts::Me {
        crate::accounts::Me {
            account: self.session.account,
            identity: self.phrase.secret().to_bytes(),
            display_name: self.session.display_name.clone(),
            email: self.session.email.clone(),
        }
    }

    async fn peer(
        &self,
        url: &str,
        organization: Uuid,
    ) -> std::sync::Arc<be_block_client::Peer<be_block_store::MemoryStore>> {
        let peer = be_block_client::Peer::connect(
            be_block_client::PeerConfig::new(
                crate::accounts::socket_url(url),
                self.phrase.secret(),
                be_block_client::Credentials::Token(self.session.token.clone()),
            )
            .organization(Some(organization)),
            be_block_store::MemoryStore::new(),
        )
        .await
        .expect("the account opens the organization");
        std::sync::Arc::new(peer)
    }
}

impl Harness {
    fn personal(&self) -> Uuid {
        runtime()
            .block_on(crate::accounts::organizations(
                self.url.clone(),
                self.token.clone(),
            ))
            .expect("the organizations load")
            .into_iter()
            .find(|organization| organization.kind == be_block_protocol::OrganizationKind::Personal)
            .expect("registering joined the personal organization")
            .id
    }

    fn connect_personal(&self) -> Uuid {
        let personal = self.personal();
        self.start_in(personal);
        personal
    }

    /// Starts the stack in `organization` (the personal one), from what it kept before.
    fn start_in(&self, personal: Uuid) {
        start(Config {
            organization: personal,
            ..self.config(self.directory.join("personal"))
        });
        wait_until("loaded the graph", |shared| {
            shared.graph.ready(super::graph::Query::Roots)
        });
    }
}

fn made_link(offer: Option<(Uuid, be_block_graph::Access)>) -> be_block_keys::InviteCode {
    let (sender, receiver) = std::sync::mpsc::channel();
    make_contact_link(offer, move |made| {
        let _ = sender.send(made);
    });
    let link = receiver
        .recv()
        .expect("the worker answered")
        .expect("the link was made");
    crate::invite_link::InviteLink::parse_contact(&link)
        .expect("the link parses")
        .code
}

fn written(block: Uuid, text: &str) {
    create(
        block,
        be_type_text::TextContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block::BlockMetadata::named("Plan"),
        Some(be_type_text::TextBlock::of(text).encode()),
        false,
    );
    flush();
}

async fn read_text(
    peer: &be_block_client::Peer<be_block_store::MemoryStore>,
    block: Uuid,
) -> String {
    let text = peer
        .open::<be_type_text::TextContent>(block)
        .await
        .expect("the block opens with the keys it was given")
        .expect("the block has content");
    be_type_text::TextBlock::text(&text)
}

struct Device {
    runtime: tokio::runtime::Runtime,
    refs: be_block_client::RefsDb,
    store: be_block_store::MemoryStore,
    account: Uuid,
    url: String,
    config: Box<dyn Fn() -> be_block_client::PeerConfig>,
    peer: std::cell::RefCell<std::sync::Arc<be_block_client::Peer<be_block_store::MemoryStore>>>,
}

impl Device {
    fn online(harness: &Harness) -> Self {
        let runtime = runtime();
        let refs = be_block_client::RefsDb::memory();
        let store = be_block_store::MemoryStore::new();
        let (url, identity, token, organization) = (
            harness.config(PathBuf::new()).socket_url(),
            harness.identity(),
            harness.token.clone(),
            harness.organization,
        );
        let config = {
            let url = url.clone();
            move || {
                be_block_client::PeerConfig::new(
                    url.clone(),
                    be_block_keys::RecoverySecret::from_bytes(identity),
                    be_block_client::Credentials::Token(token.clone()),
                )
                .organization(Some(organization))
            }
        };
        let peer = runtime
            .block_on(be_block_client::Peer::connect(
                config().refs(refs.clone()),
                store.clone(),
            ))
            .expect("the other device signs in");
        Self {
            runtime,
            refs,
            store,
            account: harness.account,
            url,
            config: Box::new(config),
            peer: std::cell::RefCell::new(std::sync::Arc::new(peer)),
        }
    }

    fn read(&self, block: Uuid) -> String {
        let peer = std::sync::Arc::clone(&self.peer.borrow());
        self.runtime.block_on(read_text(&peer, block))
    }

    fn save_offline(&self, block: Uuid, text: &str) {
        let offline = be_block_client::Peer::cached(
            (self.config)().refs(self.refs.clone()),
            self.account,
            self.store.clone(),
        )
        .expect("the other device starts from what it saved");
        let base = offline.local_head(block);
        let saved =
            self.runtime
                .block_on(offline.save(block, &be_type_text::TextBlock::of(text), base));
        assert!(
            matches!(saved, Err(be_block_client::ClientError::Disconnected(_))),
            "the other device saved while it was meant to be offline: {saved:?}"
        );
        assert_ne!(offline.local_head(block), base, "the edit was not kept");
        *self.peer.borrow_mut() = std::sync::Arc::new(offline);
    }

    fn reconnect(&self, harness: &Harness) {
        let peer = std::sync::Arc::clone(&self.peer.borrow());
        let token = harness.token.clone();
        let drained = self.runtime.block_on(async {
            let connection =
                be_block_client::Connection::connect(&self.url, &be_block_client::Clock::real())
                    .await
                    .expect("the other device reaches the server again");
            let token = be_block_client::ask(be_block_protocol::client::account::authenticate(
                &connection,
                Some(token),
            ))
            .await
            .expect("the other device signs in again")
            .token;
            peer.attach(connection, token)
                .await
                .expect("the other device opens the workspace again");
            peer.drain(|_| false)
                .await
                .expect("the other device sends what waited")
        });
        assert!(
            drained
                .iter()
                .all(|outcome| matches!(outcome, be_block_client::Drained::Published { .. })),
            "the other device could not send what it wrote offline: {drained:?}"
        );
    }
}

/// A Web Push service on loopback that takes every push the server sends and answers it
/// delivered: which endpoint each went to, and how urgent the server said it was.
struct PushService {
    url: String,
    pushes: std::sync::mpsc::Receiver<(String, String)>,
}

impl PushService {
    fn start() -> Self {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("the push service listens");
        let url = format!(
            "http://{}",
            listener
                .local_addr()
                .expect("the push service has an address")
        );
        let (sender, pushes) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                if let Some(push) = read_push(&mut stream) {
                    let _ = std::io::Write::write_all(
                        &mut stream,
                        b"HTTP/1.1 201 Created\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                    );
                    if sender.send(push).is_err() {
                        return;
                    }
                }
            }
        });
        Self { url, pushes }
    }

    /// The endpoint of the member called `name`, with a device key the server encrypts to (the
    /// generator of P-256: no test reads what a push carries, only where it went).
    fn endpoint(&self, name: &str) -> be_block_protocol::PushEndpoint {
        const GENERATOR: [u8; 65] = [
            0x04, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63,
            0xa4, 0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39,
            0x45, 0xd8, 0x98, 0xc2, 0x96, 0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e,
            0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e, 0x16, 0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e,
            0xce, 0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf, 0x51, 0xf5,
        ];
        be_block_protocol::PushEndpoint {
            service: be_block_protocol::PushService::WebPush,
            address: format!("{}/{name}", self.url),
            p256dh: GENERATOR.to_vec(),
            auth: vec![7; 16],
        }
    }

    /// The next push: the member it went to and its urgency.
    fn next(&self) -> (String, String) {
        self.pushes
            .recv_timeout(Duration::from_secs(60))
            .expect("a push arrived")
    }
}

/// Reads one request: its path, less the leading slash, and its `urgency` header.
fn read_push(stream: &mut std::net::TcpStream) -> Option<(String, String)> {
    use std::io::Read;
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    let end = loop {
        let read = stream.read(&mut buffer).ok()?;
        if read == 0 {
            return None;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let head = String::from_utf8_lossy(&bytes[..end]).into_owned();
    let mut lines = head.lines();
    let path = lines
        .next()?
        .split(' ')
        .nth(1)?
        .trim_start_matches('/')
        .to_owned();
    let header = |name: &str| {
        head.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case(name)
                .then(|| value.trim().to_owned())
        })
    };
    let length: usize = header("content-length")?.parse().ok()?;
    while bytes.len() < end + length {
        let read = stream.read(&mut buffer).ok()?;
        if read == 0 {
            return None;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    Some((path, header("urgency").unwrap_or_default()))
}

/// A chat in this account's personal organization that `away` (notified of every message) and
/// the `others` can see, opened from an earlier message by each, and the push service their
/// devices take pushes at, each at its name. Every member but the ones `others` keeps connected
/// is signed out, so the server pushes to them.
struct ChatRoom {
    block: Uuid,
    personal: Uuid,
    me: Uuid,
    members: Vec<(&'static str, Uuid)>,
    _connected: Vec<Member>,
    pushes: PushService,
}

/// A member of a chat besides `away`: its name, the mode it sets, if any, and whether it stays
/// connected.
type Other = (&'static str, Option<be_block_protocol::NotifyMode>, bool);

impl ChatRoom {
    fn start(harness: &Harness, others: &[Other]) -> Self {
        let pushes = PushService::start();
        let personal = harness.connect_personal();
        let block = Uuid::new_v4();
        let mut joined = Vec::new();
        let mut chat = be_type_chat::ChatContent::default();
        for (name, mode, connected) in [("away", None, false)].iter().chain(others) {
            let member = Member::join(harness, personal, &format!("{name}@example.com"), name);
            let (_, said) = chat.send(
                block,
                be_type_chat::Message::new(member.account(), 1, format!("{name} is in")),
            );
            chat.apply(&said);
            joined.push((*name, *mode, *connected, member));
        }
        create(
            block,
            be_type_chat::ChatContent::CONTENT_TYPE,
            be_block_graph::BlockParent::Root,
            be_block::BlockMetadata::named("Lunch"),
            Some(chat.encode()),
            false,
        );
        flush();
        let mut members = Vec::new();
        let mut kept = Vec::new();
        for (name, mode, connected, member) in joined {
            member.may_view(block);
            member.takes_pushes(&pushes, name);
            if let Some(mode) = mode {
                member.mode(block, mode);
            }
            members.push((name, member.account()));
            match connected {
                true => kept.push(member),
                false => member.signs_out(),
            }
        }
        Self {
            block,
            personal,
            me: harness.account,
            members,
            _connected: kept,
            pushes,
        }
    }

    /// Starts the host again after `stop`, from what it kept.
    fn restart(&self, harness: &Harness) {
        harness.start_in(self.personal);
    }

    fn account(&self, name: &str) -> Uuid {
        self.members
            .iter()
            .find(|(member, _)| *member == name)
            .map(|(_, account)| *account)
            .expect("the chat has the member")
    }

    fn editor(&self) -> Editor<be_type_chat::ChatContent> {
        let mut editor = Editor::attach(self.block);
        editor.wait("was sent the chat", |_| true);
        editor
    }

    /// Sends `body` from this account, replying to `to`'s first message when given.
    fn send(&self, editor: &mut Editor<be_type_chat::ChatContent>, body: &str, to: Option<Uuid>) {
        let content = editor.content();
        let quoted = to.and_then(|author| {
            content.messages().into_iter().find(|id| {
                content
                    .message(*id)
                    .is_some_and(|message| message.author == author)
            })
        });
        let message = be_type_chat::Message::new(self.me, 2, body).replying_to(quoted);
        let (_, edit) = content.send(self.block, message);
        editor.operate(edit);
    }

    /// The next push to `away`, which must not be urgent: one about a message that does not
    /// mention them.
    fn told_away(&self) {
        assert_eq!(self.pushes.next(), ("away".to_owned(), "normal".to_owned()));
    }

    /// Replies to `away`'s message, which tells them urgently: the next push they get is that
    /// one, so no notification of an earlier message is still on its way.
    fn told_nothing_more(&self, editor: &mut Editor<be_type_chat::ChatContent>) {
        self.send(editor, "See you there", Some(self.account("away")));
        assert_eq!(
            self.pushes.next(),
            ("away".to_owned(), "high".to_owned()),
            "an earlier message told away again"
        );
    }
}

/// Another account, a contact of this one in its personal organization, with the runtime its
/// connection runs on.
struct Member {
    someone: Someone,
    runtime: tokio::runtime::Runtime,
    peer: std::sync::Arc<be_block_client::Peer<be_block_store::MemoryStore>>,
}

impl Member {
    fn join(harness: &Harness, personal: Uuid, email: &str, name: &str) -> Self {
        let someone =
            Someone::register(&harness.directory.join("server"), &harness.url, email, name);
        let code = made_link(None);
        let runtime = runtime();
        let peer = runtime.block_on(someone.peer(&harness.url, personal));
        runtime
            .block_on(be_block_client::contacts::open_link(
                &peer.connection(),
                &code,
            ))
            .expect("the member opens the link");
        runtime
            .block_on(be_block_client::contacts::accept_link(
                &peer.connection(),
                &someone.phrase.secret(),
                &someone.me().card(),
                &code,
            ))
            .expect("the member accepts");
        let account = someone.session.account;
        wait_until("confirmed the member as a contact", |shared| {
            shared
                .contacts
                .iter()
                .any(|contact| contact.account == account && contact.public.is_some())
        });
        Self {
            someone,
            runtime,
            peer,
        }
    }

    fn account(&self) -> Uuid {
        self.someone.session.account
    }

    fn may_view(&self, block: Uuid) {
        set_access(block, self.account(), be_block_graph::Access::View);
        flush();
        assert_eq!(status().error, None);
    }

    fn takes_pushes(&self, pushes: &PushService, name: &str) {
        self.runtime
            .block_on(self.peer.register_endpoint(pushes.endpoint(name)))
            .expect("the member registers a push endpoint");
    }

    fn mode(&self, block: Uuid, mode: be_block_protocol::NotifyMode) {
        self.runtime
            .block_on(self.peer.set_notify_mode(block, mode))
            .expect("the member sets its mode");
    }

    /// Signs out, which the server counts as not connected before it answers.
    fn signs_out(self) {
        self.runtime
            .block_on(be_block_protocol::client::account::logout(
                &self.peer.connection(),
            ))
            .expect("the logout is answered")
            .expect("the member signs out");
    }
}
