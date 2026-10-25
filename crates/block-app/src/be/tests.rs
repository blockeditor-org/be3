use std::time::Duration;

use be_block::{BlockContent, LiveEdit};
use checklist_types::{Checklist, ChecklistContent};
use counter_types::{Counter, CounterContent};
use uuid::Uuid;

use super::*;
use crate::platform;

mod a_block_a_shared_one_links_to_shows_as_no_access_and_nowhere_else;
mod a_block_another_build_saved_is_offered_for_reset_and_a_held_one_is_reset;
mod a_block_file_brings_a_tree_back_as_a_copy;
mod a_block_its_parent_stops_listing_is_deleted_until_it_is_listed_again;
mod a_block_named_before_it_arrives_is_created_with_that_name;
mod a_block_shared_with_a_new_agent_at_once_is_sealed_to_it;
mod a_block_with_a_plain_form_is_read_as_a_named_file;
mod a_branch_is_combined_into_another;
mod a_checklist_and_a_counter_are_held_by_one_peer;
mod a_child_moved_into_a_block_is_added_to_its_content;
mod a_commit_in_one_checkout_is_brought_into_another;
mod a_conflicting_edit_keeps_every_side_until_one_is_chosen;
mod a_counter_lives_in_the_new_stack_and_survives_a_reconnect;
mod a_deleted_block_remembers_where_it_was_until_it_is_restored;
mod a_duplicated_block_carries_what_its_source_held;
mod a_folder_lists_a_new_child_before_its_edit_is_sealed;
mod a_fork_pulls_and_pushes_through_its_upstream;
mod a_half_open_connection_is_noticed_and_the_peer_reconnects;
mod a_held_block_stays_open_when_its_editors_close;
mod a_link_carrying_a_share_opens_the_block_the_moment_it_is_accepted;
mod a_merge_conflict_shows_both_versions_and_keeps_the_one_chosen;
mod a_new_device_gets_the_identity_secret_by_typing_its_code_on_an_open_one;
mod a_paste_too_large_to_relay_is_acknowledged_to_its_editor;
mod a_peer_rejoins_what_was_open_when_the_server_comes_back;
mod a_peers_edit_during_a_gesture_here_reaches_editors_beneath_it;
mod a_saved_change_is_undone_and_later_edits_stay;
mod a_seal_the_connection_loses_is_kept_and_published_after_reconnect;
mod a_second_checkout_copies_the_tree_under_the_same_local_ids;
mod a_session_owned_here_follows_a_peer_that_claims_it;
#[cfg(not(target_os = "android"))]
mod a_terminal_host_answers_this_peer_through_the_blocks_session;
#[cfg(not(target_os = "android"))]
mod a_terminal_host_takes_over_a_session_this_peer_opened_first;
mod a_type_module_that_stops_is_started_again_with_the_edits_it_had_not_sealed;
mod an_account_unlocks_with_its_recovery_phrase;
mod an_edit_made_across_a_takeover_is_kept;
mod an_edit_made_offline_survives_a_restart_and_reaches_the_server_on_reconnect;
mod an_idle_peer_never_wakes_its_worker;
mod an_imported_tree_shows_at_once_and_moves_once_it_is_uploaded;
mod an_invite_link_joins_the_organization_and_makes_the_two_contacts;
mod an_organizations_unsent_edits_are_discarded_once_the_grace_period_passes;
mod an_unknown_content_type_has_no_content_in_the_new_stack;
mod backing_off_waits_longer_after_each_failure_up_to_a_ceiling;
mod blocks_moved_renamed_and_deleted_offline_reach_the_server_on_reconnect;
mod flushing_seals_what_the_sessions_hold_and_leaves_them_live;
mod presence_crosses_between_this_peer_and_another;
mod replacing_content_reaches_a_block_open_or_not;
mod revision_history_previews_and_restores_saved_content;
mod rewrites_of_one_line_on_two_offline_devices_merge_with_a_conflict;
mod stopping_returns_while_a_connect_goes_unanswered;
mod taking_a_share_back_rotates_its_keys_and_reseals_the_open_block;
mod two_accounts_become_contacts_by_a_link_and_share_a_block;
mod two_devices_that_edit_one_note_offline_merge_once_both_reconnect;
mod two_peers_of_one_organization_share_a_counter;
mod undo_steps_back_through_what_this_peer_did;

struct Editor<C: LiveEdit> {
    block: Uuid,
    id: u64,
    mirror: block_type_guest::editor::EditorMirror<C>,
    received: Vec<block_type_guest::editor::ToEditor>,
}

impl<C: LiveEdit + Clone + Default> Editor<C> {
    fn attach(block: Uuid) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        attach(block, C::CONTENT_TYPE, id, true);
        let mut editor = Self {
            block,
            id,
            mirror: block_type_guest::editor::EditorMirror::default(),
            received: Vec::new(),
        };
        editor.send(block_type_guest::editor::FromEditor::Resend);
        editor
    }

    fn send(&mut self, message: block_type_guest::editor::FromEditor) {
        from_editor(self.block, self.id, &block_type_api::encode_frame(&message));
    }

    fn pump(&mut self) {
        for (block, frame) in take_editor_frames(self.id) {
            if block != self.block {
                continue;
            }
            let Some(message) = unframe(&frame)
                .and_then(|bytes| block_type_guest::editor::ToEditor::decode(&bytes))
            else {
                continue;
            };
            self.received.push(message.clone());
            if let Some(answer) = self.mirror.receive(message) {
                self.send(answer);
            }
        }
    }

    fn wait(&mut self, what: &str, done: impl Fn(&C) -> bool) {
        let id = self.id;
        let deadline = std::time::Instant::now() + PATIENCE;
        loop {
            self.pump();
            if self.mirror.content().is_some_and(&done) {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the editor never {what}: it says {:?}",
                status().error
            );
            wait_for(QUIET / 20, |shared| {
                shared.editor_frames.contains_key(&id).then_some(())
            });
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

    fn take_received(&mut self) -> Vec<block_type_guest::editor::ToEditor> {
        self.pump();
        std::mem::take(&mut self.received)
    }
}

impl<C: LiveEdit> Drop for Editor<C> {
    fn drop(&mut self) {
        detach(self.block, self.id);
    }
}

const PATIENCE: Duration = Duration::from_secs(20);
const QUIET: Duration = Duration::from_secs(2);

pub(crate) struct Harness {
    directory: PathBuf,
    server: Option<platform::EmbeddedServer>,
    url: String,
    account: Uuid,
    token: String,
    organization: Uuid,
    phrase: be_keys::RecoveryPhrase,
    lease: Duration,
    relay: Option<fault_relay::Relay>,
    clock: be_client::Clock,
    _one_stack: std::sync::MutexGuard<'static, ()>,
}

impl Harness {
    pub(crate) fn start() -> Self {
        Self::leasing(be_server::ServerConfig::OPEN.lease)
    }

    pub(crate) fn lapsing() -> Self {
        Self::leasing(Duration::ZERO)
    }

    fn relayed() -> Self {
        let mut harness = Self::start();
        let server = harness
            .address()
            .parse()
            .expect("the embedded server listens on an address");
        harness.relay = Some(fault_relay::Relay::start(server));
        harness.clock = be_client::Clock::manual();
        harness
    }

    fn relay(&self) -> &fault_relay::Relay {
        self.relay
            .as_ref()
            .expect("the harness was started with a relay")
    }

    fn leasing(lease: Duration) -> Self {
        static ONE_STACK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let one_stack = ONE_STACK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        build_id::install("test");
        let directory = std::env::temp_dir().join(format!("block-app-be-test-{}", Uuid::new_v4()));
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
        let server =
            platform::start_embedded_server_at("127.0.0.1:0", directory.join("server"), lease)
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
            phrase: be_keys::RecoveryPhrase::generate(),
            lease,
            relay: None,
            clock: be_client::Clock::real(),
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
            platform::start_embedded_server_at(address, self.directory.join("server"), self.lease)
                .expect("the embedded block server starts again"),
        );
    }

    pub(crate) fn connect(&self) {
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

    fn outside_config(&self) -> be_client::PeerConfig {
        be_client::PeerConfig::new(
            crate::accounts::socket_url(&self.url),
            be_keys::RecoverySecret::from_bytes(self.identity()),
            be_client::Credentials::Token(self.token.clone()),
        )
        .organization(Some(self.organization))
    }

    async fn outside_peer(&self) -> std::sync::Arc<be_client::Peer<be_store::MemoryStore>> {
        outside_peer(self.outside_config()).await
    }
}

async fn outside_peer(
    config: be_client::PeerConfig,
) -> std::sync::Arc<be_client::Peer<be_store::MemoryStore>> {
    let peer = be_client::Peer::connect(config, be_store::MemoryStore::new())
        .await
        .expect("a second peer joins the organization");
    std::sync::Arc::new(peer)
}

impl Drop for Harness {
    fn drop(&mut self) {
        stop();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

struct Outside {
    runtime: tokio::runtime::Runtime,
    local: tokio::task::LocalSet,
    session: Option<be_cli::replica::Session<be_store::MemoryStore, CounterContent>>,
}

struct Seen {
    count: Option<i64>,
    presence: Vec<block_type_guest::editor::PeerPresence>,
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
                peer.ensure::<CounterContent>(block, be_graph::BlockParent::Root)
                    .await
                    .expect("the counter is created");
            }
            be_cli::replica::Session::open(peer, block, super::types::sources().await)
                .await
                .expect("the outside peer opens the counter")
        });
        Self {
            runtime,
            local,
            session: Some(session),
        }
    }

    fn session(&mut self) -> &mut be_cli::replica::Session<be_store::MemoryStore, CounterContent> {
        self.session
            .as_mut()
            .expect("the outside peer has not left")
    }

    fn peer(&mut self) -> std::sync::Arc<be_client::Peer<be_store::MemoryStore>> {
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
        let reached = local.block_on(runtime, async {
            tokio::time::timeout(PATIENCE, async {
                while !session.is_owner() {
                    session.claim();
                    let _ = tokio::time::timeout(Duration::from_millis(100), async {
                        while !session.is_owner() {
                            let woken = session.next().await;
                            session
                                .handle(woken)
                                .await
                                .expect("the outside peer is still connected");
                        }
                    })
                    .await;
                }
            })
            .await
        });
        assert!(reached.is_ok(), "the outside peer never took the session");
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

    fn until(&mut self, what: &str, ready: impl Fn(&Seen) -> bool) {
        let Self {
            runtime,
            local,
            session,
        } = self;
        let session = session.as_mut().expect("the outside peer has not left");
        let reached = local.block_on(runtime, async {
            tokio::time::timeout(PATIENCE, async {
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
            })
            .await
        });
        assert!(reached.is_ok(), "the outside peer never {what}");
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

fn wait_for_count(block: Uuid, expected: i64) {
    let reached = wait_for(PATIENCE, |shared| {
        (counted(shared, block) == Some(expected)).then_some(())
    });
    assert!(
        reached.is_some(),
        "the new stack never reported {expected} for {block}: it holds {:?}, and the stack says {:?}",
        count_of(block),
        status().error
    );
}

fn ask_for(queries: Vec<Query>) {
    let owner = Owner::Editor(u64::MAX);
    let mut wanted = with_graph(|graph| graph.wanted(owner).to_vec()).unwrap_or_default();
    wanted.extend(queries);
    want(owner, wanted);
}

fn wait_until(what: &str, ready: impl Fn(&Shared) -> bool) {
    let reached = wait_for(PATIENCE, |shared| ready(shared).then_some(()));
    assert!(
        reached.is_some(),
        "the new stack never {what}: it says {:?}",
        status().error
    );
}

fn add(block: Uuid, by: i64) {
    operate(block, CounterContent::encode_operation(&Counter::add(by)));
}

fn version_status(shared: &Shared, block: Uuid) -> Option<&block_plugin_api::VersionStatus> {
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
    text_block_types::TextContent::decode(held.bytes())
        .ok()
        .map(|text| text_block_types::TextBlock::text(&text))
}

const TYPIST: u64 = 1 << 63 | 7;

fn type_text(block: Uuid, at: usize, text: &str) {
    let mut editor = Editor::<text_block_types::TextContent>::attach(block);
    type_into(&mut editor, at, text);
}

fn type_into(editor: &mut Editor<text_block_types::TextContent>, at: usize, text: &str) {
    editor.wait("was sent the text", |_| true);
    let edit = text_block_types::TextBlock::insert(editor.content(), TYPIST, at, text.as_bytes())
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
    let mut listing = workspace_ui_types::FolderContent::default();
    listing.apply(&workspace_ui_types::Folder::default().add(note));
    create(
        folder,
        workspace_ui_types::FolderContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::named("Notes"),
        Some(listing.encode()),
    );
    create(
        note,
        text_block_types::TextContent::CONTENT_TYPE,
        be_graph::BlockParent::Block(folder),
        be_block::BlockMetadata::named("Note"),
        Some(text_block_types::TextBlock::of(text).encode()),
    );
    create(
        repository,
        be_block::RepositoryContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::named("History"),
        Some(be_block::RepositoryContent::default().encode()),
    );
    version_since(repository, be_block::RepositoryContent::CONTENT_TYPE, None);
    version(
        repository,
        block_plugin_api::VersionCommand::Adopt {
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
        block_plugin_api::VersionCommand::NewCheckout {
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
    phrase: be_keys::RecoveryPhrase,
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
            phrase: be_keys::RecoveryPhrase::generate(),
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
    ) -> std::sync::Arc<be_client::Peer<be_store::MemoryStore>> {
        let peer = be_client::Peer::connect(
            be_client::PeerConfig::new(
                crate::accounts::socket_url(url),
                self.phrase.secret(),
                be_client::Credentials::Token(self.session.token.clone()),
            )
            .organization(Some(organization)),
            be_store::MemoryStore::new(),
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
            .find(|organization| organization.kind == be_protocol::OrganizationKind::Personal)
            .expect("registering joined the personal organization")
            .id
    }

    fn connect_personal(&self) -> Uuid {
        let personal = self.personal();
        start(Config {
            organization: personal,
            ..self.config(self.directory.join("personal"))
        });
        wait_until("loaded the graph", |shared| {
            shared.graph.ready(super::graph::Query::Roots)
        });
        personal
    }
}

fn made_link(offer: Option<(Uuid, be_graph::Access)>) -> be_keys::InviteCode {
    let (sender, receiver) = std::sync::mpsc::channel();
    make_contact_link(offer, move |made| {
        let _ = sender.send(made);
    });
    let link = receiver
        .recv_timeout(PATIENCE)
        .expect("the worker answered")
        .expect("the link was made");
    crate::invite_link::InviteLink::parse_contact(&link)
        .expect("the link parses")
        .code
}

fn written(block: Uuid, text: &str) {
    create(
        block,
        text_block_types::TextContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::named("Plan"),
        Some(text_block_types::TextBlock::of(text).encode()),
    );
    flush();
}

async fn read_text(peer: &be_client::Peer<be_store::MemoryStore>, block: Uuid) -> String {
    let text = peer
        .open::<text_block_types::TextContent>(block)
        .await
        .expect("the block opens with the keys it was given")
        .expect("the block has content");
    text_block_types::TextBlock::text(&text)
}

struct Device {
    runtime: tokio::runtime::Runtime,
    refs: be_client::RefsDb,
    store: be_store::MemoryStore,
    account: Uuid,
    url: String,
    config: Box<dyn Fn() -> be_client::PeerConfig>,
    peer: std::cell::RefCell<std::sync::Arc<be_client::Peer<be_store::MemoryStore>>>,
}

impl Device {
    fn online(harness: &Harness) -> Self {
        let runtime = runtime();
        let refs = be_client::RefsDb::memory();
        let store = be_store::MemoryStore::new();
        let (url, identity, token, organization) = (
            harness.config(PathBuf::new()).socket_url(),
            harness.identity(),
            harness.token.clone(),
            harness.organization,
        );
        let config = {
            let url = url.clone();
            move || {
                be_client::PeerConfig::new(
                    url.clone(),
                    be_keys::RecoverySecret::from_bytes(identity),
                    be_client::Credentials::Token(token.clone()),
                )
                .organization(Some(organization))
            }
        };
        let peer = runtime
            .block_on(be_client::Peer::connect(
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
        let offline = be_client::Peer::cached(
            (self.config)().refs(self.refs.clone()),
            self.account,
            self.store.clone(),
        )
        .expect("the other device starts from what it saved");
        let base = offline.local_head(block);
        let saved = self.runtime.block_on(offline.save(
            block,
            &text_block_types::TextBlock::of(text),
            base,
        ));
        assert!(
            matches!(saved, Err(be_client::ClientError::Disconnected(_))),
            "the other device saved while it was meant to be offline: {saved:?}"
        );
        assert_ne!(offline.local_head(block), base, "the edit was not kept");
        *self.peer.borrow_mut() = std::sync::Arc::new(offline);
    }

    fn reconnect(&self, harness: &Harness) {
        let peer = std::sync::Arc::clone(&self.peer.borrow());
        let token = harness.token.clone();
        let drained = self.runtime.block_on(async {
            let connection = be_client::Connection::connect(&self.url, &be_client::Clock::real())
                .await
                .expect("the other device reaches the server again");
            let be_protocol::ServerMessage::Authenticated { token, .. } = connection
                .request(|request| be_protocol::ClientMessage::Authenticate { request, token })
                .await
                .expect("the other device signs in again")
            else {
                panic!("the server did not sign the other device in");
            };
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
                .all(|outcome| matches!(outcome, be_client::Drained::Published { .. })),
            "the other device could not send what it wrote offline: {drained:?}"
        );
    }
}
