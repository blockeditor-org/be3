use std::{path::PathBuf, time::Duration};

use be_commit::{Commit, CommitId, CommitStore};
use be_graph::BlockParent;
use be_ops::net::{Peer, Pending, Stream};
use be_protocol::{
    Authenticated, BlockChange, BlockEvent, BlockSummary, ClientId, Error, ErrorCode, HistoryEntry,
    KeyKind, KeyRow, KeySet, Listing, OrganizationRole, PairingMessage, SessionEvent, SessionState,
    WrappedBy, WrappedKey, client,
};
use be_session::{Resume, resume, takeover_needs_merge};
use be_store::{ChunkerConfig, ContentKey, Hash, Keyring, MemoryStore, ObjectStore, Vault};
use futures_util::{SinkExt, StreamExt};
use tokio::{net::TcpListener, sync::oneshot};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message as WsMessage,
};
use uuid::Uuid;

use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
mod a_backup_restores_into_a_server_that_serves_the_same_blocks;
mod a_block_linked_before_it_exists_is_announced_only_once_it_does;
mod a_block_shared_from_a_hidden_folder_is_listed_as_shared;
mod a_blocks_references_are_listed_and_announced_together;
mod a_bunny_storage_zone_holds_a_backup;
mod a_client_speaks_the_protocol_over_http_posts;
mod a_closed_server_also_refuses_browser_registration_and_login;
mod a_closed_server_refuses_new_accounts_and_sign_ins_but_not_tokens;
mod a_connection_hears_only_of_the_blocks_it_listed;
mod a_contact_card_the_server_swaps_does_not_open;
mod a_detached_subtree_is_collected_and_its_objects_freed;
mod a_grant_or_a_key_for_someone_not_a_contact_is_refused;
mod a_graph_change_reaches_only_the_members_who_may_see_it;
mod a_kind_open_inside_passes_from_one_viewer_inside_to_another;
mod a_move_never_takes_a_workspace_or_more_access_without_manage;
mod a_new_recovery_key_reseals_every_key_sealed_to_the_account;
mod a_pending_grant_is_applied_when_its_link_is_accepted;
mod a_remembered_account_approves_a_manually_entered_code_and_can_switch_accounts;
mod a_rotation_gives_every_block_below_it_a_new_key_or_nothing;
mod a_server_without_a_portal_answers_its_pages_as_json;
mod a_session_hands_ownership_over_without_a_merge;
mod a_settings_block_changes_only_with_manage_on_its_parent;
mod a_settings_kind_opens_to_anyone_who_views_a_block_inside;
mod a_stale_publish_is_rejected_with_the_current_head;
mod a_watcher_is_told_when_the_head_moves;
mod an_agent_signs_in_with_its_token_until_it_is_removed;
mod an_editor_changes_the_prefixes_and_tags_but_no_other_kind;
mod an_editor_sees_only_the_blocks_it_made_or_was_given;
mod an_invited_account_joins_the_organization_and_a_logout_ends_the_session;
mod an_unauthenticated_connection_cannot_touch_blocks;
mod another_build_is_greeted_and_turned_away;
mod blocks_publish_and_read_back_through_the_server;
mod browser_authorization_survives_disconnecting_before_approval;
mod browser_forms_enforce_origin_policy_and_deny_sign_in;
mod browser_pages_escape_codes_and_keep_available_actions;
mod closing_a_kind_rotates_it_for_who_read_it_inside;
mod contacts_are_made_by_a_link_and_finished_by_its_owner;
mod email_changes_wait_for_verification_and_notify_the_old_address;
mod every_graph_change_advances_a_blocks_version;
mod every_member_connection_hears_how_the_graph_changes;
mod expired_mail_and_browser_sessions_fail_and_mail_issuance_is_bounded;
mod history_records_each_commits_parents;
mod key_rows_are_served_only_to_who_may_see_their_block;
mod legacy_local_profile_sessions_remain_usable;
mod local_profiles_need_only_a_name_and_are_unavailable_on_remote_servers;
mod losing_every_block_inside_rotates_a_kind_open_inside;
mod members_manage_only_the_agents_they_created;
mod metadata_travels_with_the_content_once_a_block_has_any;
mod moving_a_block_drops_the_wraps_of_its_old_scope;
mod moving_a_block_out_of_what_is_shared_rotates_its_keys;
mod objects_a_block_holds_outlive_its_commits_until_it_is_collected;
mod objects_are_served_only_from_blocks_the_reader_can_see;
mod old_database_backups_thin_out_by_age;
mod only_a_manager_lets_go_of_a_blocks_settings;
mod only_a_manager_shares_and_a_lower_grant_reduces_nothing;
mod pairing_messages_reach_only_the_same_accounts_other_connections;
mod password_changes_rotate_the_browser_and_revoke_other_sessions;
mod password_reset_is_generic_and_revokes_live_sessions_and_pending_approvals;
mod passwords_are_validated_on_the_server_without_truncation_or_echoing;
mod proxy_account_authentication_never_discloses_the_token;
mod pruning_history_keeps_pinned_commits;
mod references_keep_the_order_their_block_lists_them_in;
mod registering_joins_the_one_personal_organization_as_an_editor;
mod registration_requires_verification_and_links_are_purpose_bound_and_single_use;
mod relayed_session_traffic_passes_through_the_server_sealed;
mod removing_a_contact_revokes_what_the_two_shared;
mod repeated_failed_sign_ins_lock_the_account_out;
mod requests_sent_before_a_close_are_still_carried_out;
mod revoking_an_http_session_removes_its_access;
mod sessions_are_scoped_to_the_account_and_form_tokens_to_the_browser;
mod shared_chunks_survive_until_the_last_commit_releases_them;
mod the_access_list_leaves_administrators_to_the_organization_line;
mod the_browser_authorization_requires_the_private_secret_and_works_once;
mod the_portal_serves_its_assets_immutably_beside_a_hydrating_page;

const CONTENT: Uuid = Uuid::from_u128(0x7465_7874);
const KEY: Uuid = Uuid::from_u128(0x006b_6579);

const BUILD: &str = "a test build";

struct Harness {
    mail: Arc<CapturedMail>,
    url: String,
    directory: PathBuf,
    shutdown: Option<oneshot::Sender<()>>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Harness {
    async fn start() -> Self {
        Self::at(std::env::temp_dir().join(format!("be-server-test-{}", Uuid::new_v4()))).await
    }

    async fn at(directory: PathBuf) -> Self {
        Self::at_with_config(directory, ServerConfig::OPEN).await
    }

    async fn configured(config: ServerConfig) -> Self {
        Self::at_with_config(
            std::env::temp_dir().join(format!("be-server-test-{}", Uuid::new_v4())),
            config,
        )
        .await
    }

    async fn at_with_config(directory: PathBuf, config: ServerConfig) -> Self {
        build_id::install(BUILD);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown, receiver) = oneshot::channel();
        let data_dir = directory.clone();
        let mail = Arc::new(CapturedMail::default());
        let delivery = mail.clone();
        let handle = tokio::spawn(async move {
            let _ = serve_with_mail(listener, data_dir, config, delivery, receiver).await;
        });
        Self {
            mail,
            url: format!("ws://{address}"),
            directory,
            shutdown: Some(shutdown),
            handle: Some(handle),
        }
    }

    async fn client(&self) -> TestClient {
        let mut client = TestClient::connect(&self.url).await;
        client.directory = Some(self.directory.clone());
        client
    }

    async fn greeted(&self, build: &str) -> Socket {
        greeted(&self.url, build).await
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
            .call(|peer| client::account::authenticate(peer, token))
            .await;
        assert!(response.is_ok(), "{response:?}");
        client.open(organization).await;
        client
    }

    async fn restart(mut self) -> Self {
        self.halt().await;
        Self::at(self.directory.clone()).await
    }

    async fn halt(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take() {
            handle.abort();
            let _ = handle.await;
        }
    }

    async fn stop(mut self) {
        self.halt().await;
        let _ = std::fs::remove_dir_all(&self.directory);
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
        let peer = Peer::new(be_ops::Side::Guest, move |frame| {
            let _ = sending.send(frame);
        });
        let (closing, closed) = oneshot::channel();
        let pump = tokio::spawn(pump(socket, peer.clone(), outbound, closed));
        let heard = vec![
            Heard::Changes(client::blocks::changes(&peer)),
            Heard::Paired(client::account::paired(&peer)),
            Heard::Contacts(client::contacts::contacts_changed(&peer)),
            Heard::KeyWork(client::keys::key_work_changed(&peer)),
        ];
        Self {
            directory: None,
            peer,
            heard,
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

    async fn call<T: be_ops::DeserializeOwned>(
        &mut self,
        start: impl FnOnce(&Peer) -> Pending<Result<T, Error>>,
    ) -> Result<T, Error> {
        let answered = start(&self.peer).await.expect("the connection stayed open");
        self.collect();
        answered
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
            Some(SessionEvent::Joined { client, state }) => Ok((client, state)),
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
            .call(|peer| client::account::authenticate(peer, token))
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
        let response = self
            .call(|peer| {
                client::blocks::create_block(
                    peer,
                    block,
                    CONTENT,
                    parent,
                    Vec::new(),
                    Vec::new(),
                    key,
                    keys,
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
        let response = self
            .call(|peer| client::objects::put_objects(peer, objects))
            .await;
        assert!(
            matches!(&response, Ok(stored) if stored == hashes),
            "{response:?}"
        );
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
    ) -> Result<be_protocol::PublishOutcome, Error> {
        self.upload(&author.store, &author.objects_of(commit, &chunks))
            .await;
        self.call(|peer| {
            client::blocks::publish(
                peer,
                block,
                commit,
                expected,
                expected.into_iter().collect(),
                chunks,
                time,
                false,
                Vec::new(),
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
                    chunks,
                    1_000,
                    false,
                    children,
                    Vec::new(),
                    Vec::new(),
                )
            })
            .await;
        assert!(
            matches!(response, Ok(be_protocol::PublishOutcome::Published(_))),
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
        let chunks = manifest.chunk_hashes();
        let commit = self
            .commits
            .put(
                KEY,
                &Commit::new(manifest, self.account, time).with_parent(parent),
            )
            .unwrap();
        (commit, chunks)
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
            .find(|organization| organization.kind == be_protocol::OrganizationKind::Personal)
            .expect("registering joined the personal organization")
            .id;
        self.open(personal).await;
        personal
    }

    async fn link(&mut self, grant: Option<be_protocol::PendingGrant>) -> Uuid {
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

    async fn contacts(&mut self) -> Vec<be_protocol::ContactRow> {
        self.call(|peer| client::contacts::list_contacts(peer))
            .await
            .expect("listing contacts failed")
    }

    async fn move_to(&mut self, block: Uuid, parent: BlockParent) -> Result<(), Error> {
        self.call(|peer| client::blocks::set_parent(peer, block, parent, KeySet::default()))
            .await
    }

    async fn grant(
        &mut self,
        block: Uuid,
        account: Uuid,
        access: be_graph::Access,
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
    let address = harness.url.trim_start_matches("ws://");
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
async fn begin_session(client: &mut TestClient) -> (String, String) {
    let begun = client
        .call(|peer| client::account::begin_browser_login(peer))
        .await
        .expect("a browser authorization session starts");
    (begun.code, begun.secret)
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
        let address = harness.url.trim_start_matches("ws://");
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
fn page_of(response: &str) -> be_portal::Page {
    be_portal::Page::from_json(response.split("\r\n\r\n").nth(1).unwrap()).unwrap()
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
