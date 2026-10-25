use std::{path::PathBuf, time::Duration};

use be_commit::{Commit, CommitId, CommitStore};
use be_graph::BlockParent;
use be_protocol::{
    BlockSummary, ClientMessage, ErrorCode, HistoryEntry, KeyKind, KeyRow, KeySet,
    OrganizationRole, ServerMessage, SessionState, WrappedBy, WrappedKey, encode as encode_message,
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
mod revoking_an_http_session_removes_its_access;
mod sessions_are_scoped_to_the_account_and_form_tokens_to_the_browser;
mod shared_chunks_survive_until_the_last_commit_releases_them;
mod the_access_list_leaves_administrators_to_the_organization_line;
mod the_browser_authorization_requires_the_private_secret_and_works_once;

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
        let response = client
            .send(|request| ClientMessage::Authenticate {
                request,
                token: token.to_owned(),
            })
            .await;
        assert!(
            matches!(response, ServerMessage::Authenticated { .. }),
            "{response:?}"
        );
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

struct TestClient {
    directory: Option<PathBuf>,
    socket: Socket,
    next: u64,
    notifications: Vec<ServerMessage>,
    token: String,
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
        Self {
            directory: None,
            socket,
            next: 0,
            notifications: Vec::new(),
            token: String::new(),
        }
    }

    async fn send(&mut self, build: impl FnOnce(u64) -> ClientMessage) -> ServerMessage {
        self.next += 1;
        let request = self.next;
        self.socket
            .send(WsMessage::Binary(encode_message(&build(request)).unwrap()))
            .await
            .unwrap();
        loop {
            let message = tokio::time::timeout(Duration::from_secs(10), self.socket.next())
                .await
                .expect("the server answered in time")
                .expect("the connection stayed open")
                .unwrap();
            let WsMessage::Binary(bytes) = message else {
                continue;
            };
            let response: ServerMessage = be_protocol::decode(&bytes).unwrap();
            match response.request() {
                Some(id) if id == request => return response,
                Some(_) => panic!("the server answered a request that was not outstanding"),
                None => self.notifications.push(response),
            }
        }
    }

    async fn notification(&mut self) -> ServerMessage {
        loop {
            let next = self.next_notification().await;
            if !matches!(
                next,
                ServerMessage::BlockChanged { .. } | ServerMessage::BlockRemoved { .. }
            ) {
                return next;
            }
        }
    }

    async fn graph_change(&mut self) -> ServerMessage {
        loop {
            let next = self.next_notification().await;
            if matches!(
                next,
                ServerMessage::BlockChanged { .. } | ServerMessage::BlockRemoved { .. }
            ) {
                return next;
            }
        }
    }

    async fn next_notification(&mut self) -> ServerMessage {
        if !self.notifications.is_empty() {
            return self.notifications.remove(0);
        }
        loop {
            let message = tokio::time::timeout(Duration::from_secs(10), self.socket.next())
                .await
                .expect("a notification arrived in time")
                .expect("the connection stayed open")
                .unwrap();
            if let WsMessage::Binary(bytes) = message {
                let response: ServerMessage = be_protocol::decode(&bytes).unwrap();
                if response.request().is_none() {
                    return response;
                }
            }
        }
    }

    async fn register(&mut self, email: &str) -> Uuid {
        let (_, token) = ServerStore::open(self.directory.as_ref().unwrap())
            .unwrap()
            .register(email, email, "correct horse battery")
            .await
            .unwrap();
        let response = self
            .send(|request| ClientMessage::Authenticate { request, token })
            .await;
        let ServerMessage::Authenticated { account, token, .. } = response else {
            panic!("registration failed: {response:?}");
        };
        self.token = token;
        account
    }

    async fn organization(&mut self, name: &str) -> Uuid {
        let response = self
            .send(|request| ClientMessage::CreateOrganization {
                request,
                name: name.into(),
            })
            .await;
        let ServerMessage::Organizations { organizations, .. } = response else {
            panic!("organization creation failed: {response:?}");
        };
        let organization = organizations[0].id;
        self.open(organization).await;
        organization
    }

    async fn open(&mut self, organization: Uuid) {
        let response = self
            .send(|request| ClientMessage::OpenOrganization {
                request,
                organization,
            })
            .await;
        assert!(
            matches!(response, ServerMessage::OrganizationOpened { .. }),
            "{response:?}"
        );
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
            .send(|request| ClientMessage::CreateBlock {
                request,
                block,
                content_type: CONTENT,
                parent,
                metadata: Vec::new(),
                references: Vec::new(),
                key,
                keys,
            })
            .await;
        assert!(
            matches!(response, ServerMessage::Block { .. }),
            "{response:?}"
        );
    }

    async fn keys(&mut self, blocks: Option<Vec<Uuid>>) -> KeySet {
        let response = self
            .send(|request| ClientMessage::GetKeys { request, blocks })
            .await;
        let ServerMessage::Keys { keys, .. } = response else {
            panic!("reading keys failed: {response:?}");
        };
        keys
    }

    async fn upload(&mut self, store: &MemoryStore, hashes: &[Hash]) {
        let objects = hashes
            .iter()
            .map(|hash| be_protocol::ByteBuf::from(store.get(*hash).unwrap().unwrap()))
            .collect();
        let response = self
            .send(|request| ClientMessage::PutObjects { request, objects })
            .await;
        assert!(
            matches!(&response, ServerMessage::Stored { hashes: stored, .. } if stored == hashes),
            "{response:?}"
        );
    }

    async fn read_block(&mut self, block: Uuid) -> BlockSummary {
        let response = self
            .send(|request| ClientMessage::ReadBlock { request, block })
            .await;
        let ServerMessage::Block { block, .. } = response else {
            panic!("read failed: {response:?}");
        };
        block
    }

    async fn list(&mut self, parent: BlockParent) -> Vec<BlockSummary> {
        let response = self
            .send(|request| ClientMessage::ListChildren { request, parent })
            .await;
        let ServerMessage::Blocks { blocks, .. } = response else {
            panic!("listing failed: {response:?}");
        };
        blocks
    }

    async fn publish(
        &mut self,
        author: &Author,
        block: Uuid,
        commit: CommitId,
        chunks: Vec<Hash>,
        expected: Option<CommitId>,
        time: i64,
    ) -> ServerMessage {
        self.upload(&author.store, &author.objects_of(commit, &chunks))
            .await;
        self.send(|request| ClientMessage::Publish {
            request,
            block,
            commit,
            expected,
            parents: expected.into_iter().collect(),
            chunks,
            time,
            pinned: false,
            references: Vec::new(),
            metadata: Vec::new(),
            objects: Vec::new(),
        })
        .await
    }

    async fn join_session(&mut self, block: Uuid) -> (be_protocol::ClientId, SessionState) {
        let response = self
            .send(|request| ClientMessage::JoinSession { request, block })
            .await;
        let ServerMessage::Session { client, state, .. } = response else {
            panic!("joining failed: {response:?}");
        };
        (client, state)
    }

    async fn history(&mut self, block: Uuid) -> Vec<HistoryEntry> {
        let response = self
            .send(|request| ClientMessage::ListHistory { request, block })
            .await;
        let ServerMessage::History { entries, .. } = response else {
            panic!("history failed: {response:?}");
        };
        entries
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
            .send(|request| ClientMessage::Publish {
                request,
                block: parent,
                commit: head,
                expected: None,
                parents: Vec::new(),
                chunks,
                time: 1_000,
                pinned: false,
                references: children,
                metadata: Vec::new(),
                objects: Vec::new(),
            })
            .await;
        assert!(
            matches!(response, ServerMessage::Published { .. }),
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
        let response = self
            .send(|request| ClientMessage::ListOrganizations { request })
            .await;
        let ServerMessage::Organizations { organizations, .. } = response else {
            panic!("listing organizations failed: {response:?}");
        };
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
            .send(|request| ClientMessage::CreateContactLink {
                request,
                link,
                card: b"the owner's card".to_vec(),
                code: b"the code, sealed for the owner".to_vec(),
                grant,
            })
            .await;
        assert!(matches!(response, ServerMessage::Ok { .. }), "{response:?}");
        link
    }

    async fn accept(&mut self, link: Uuid, key: &[u8]) -> ServerMessage {
        self.send(|request| ClientMessage::AcceptContactLink {
            request,
            link,
            card: b"the accepter's card".to_vec(),
            contact: b"the owner's key, sealed for the accepter".to_vec(),
            key: key.to_vec(),
        })
        .await
    }

    async fn contacts(&mut self) -> Vec<be_protocol::ContactRow> {
        let response = self
            .send(|request| ClientMessage::ListContacts { request })
            .await;
        let ServerMessage::Contacts { contacts, .. } = response else {
            panic!("listing contacts failed: {response:?}");
        };
        contacts
    }

    async fn move_to(&mut self, block: Uuid, parent: BlockParent) -> ServerMessage {
        self.send(|request| ClientMessage::SetParent {
            request,
            block,
            parent,
            keys: KeySet::default(),
        })
        .await
    }

    async fn grant(
        &mut self,
        block: Uuid,
        account: Uuid,
        access: be_graph::Access,
    ) -> ServerMessage {
        self.send(|request| ClientMessage::SetAccess {
            request,
            block,
            account,
            access,
        })
        .await
    }
}

fn refused_as(response: &ServerMessage, expected: ErrorCode) -> bool {
    matches!(response, ServerMessage::Failed { code, .. } if *code == expected)
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
    let ServerMessage::BrowserLogin { code, secret, .. } = client
        .send(|request| ClientMessage::BeginBrowserLogin { request })
        .await
    else {
        panic!("a browser authorization session starts")
    };
    (code, secret)
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
fn csrf(page: &str) -> String {
    page.split("name=\"csrf\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .into()
}
