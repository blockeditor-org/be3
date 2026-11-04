use std::{
    collections::{BTreeSet, HashMap},
    net::IpAddr,
    sync::Arc,
};

use be_block_graph::{Access, BlockParent};
use be_block_protocol::{
    AccountStorage, Authenticated, BlockChange, BlockEvent, BlockSummary, BrowserLogin, BrowserLoginState,
    Collected,
    Contents, CreatedAgent, Error, ErrorCode, Listing, OrganizationOpened, PairingMessage,
    RecoveryKey, SessionEvent, SessionState, embed,
};
use be_block_session::Claim;
use be_wasm_ops::{Backlog, Incoming, OpError, OpId, Outlets, Reply, Started, net::Peer};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    CONTENTS_BYTES, Identity, PublishOutcome, ServerConfig, ServerError, ServerStore, Services,
    SessionRegistry, WatchHub,
    address::{Anonymous, Eviction},
    browser_auth, contacts,
    limits::{
        MAX_PAIRING_BYTES, MAX_RELAY_BYTES, MAX_SESSIONS_PER_CONNECTION, MAX_STREAMS_PER_CONNECTION,
    },
    store,
    watch::{Place, Push},
};

/// Items a stream keeps beyond the credit its consumer granted. A client that lets one fill up
/// is disconnected, as is one whose hub queue (`PUSHES`) fills.
const STREAM_BUFFER: usize = 4096;
/// Bytes every stream of one connection keeps together beyond the credit granted; a client that
/// holds back more, by not reading what it is sent, is disconnected too.
pub(crate) const STREAM_BUFFER_BYTES: usize = 4 * be_block_protocol::MAX_FRAME_BYTES;
const PUSHES: usize = 4096;

/// What a call needs before its handler runs. Every call of `be3:server` has one, stated in `rule`
/// alone, and the dispatcher (`Client::admit`) applies it; a handler then checks only what its rule
/// cannot say (a move's destination, a grant's grantee), under the graph lock.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Rule {
    /// Anyone, signed in or not: the calls that sign in.
    Anyone,
    SignedIn,
    /// Signed in, with an organization open.
    Organization,
    /// An administrator of the organization open.
    Administrator,
    /// An administrator of the organization named.
    AdministratorOf(Uuid),
    /// At least this access to every block named, in the organization open. A block the caller
    /// may not know exists is refused as one that does not.
    Blocks(Vec<Uuid>, Access),
    /// At least this access to the block of a live session.
    Session(OpId, Access),
}

fn block(block: Uuid, level: Access) -> Rule {
    Rule::Blocks(vec![block], level)
}

/// The one place each call's requirement is stated. Its matches name every call, so a new call
/// does not compile until it has a rule.
pub(crate) fn rule(call: &embed::Call) -> Rule {
    use embed::{Call, account, blocks, contacts, keys, members, objects, organizations};
    match call {
        Call::Connection(embed::connection::Call::Ping { .. }) => Rule::Anyone,
        Call::Account(call) => match call {
            account::Call::BeginBrowserLogin { .. }
            | account::Call::PollBrowserLogin { .. }
            | account::Call::CreateLocalProfile { .. }
            | account::Call::Authenticate { .. } => Rule::Anyone,
            account::Call::StorageUsage { .. } | account::Call::StorageChanged { .. } => {
                Rule::Organization
            }
            account::Call::Logout { .. }
            | account::Call::Pair { .. }
            | account::Call::Paired { .. } => Rule::SignedIn,
        },
        Call::Organizations(call) => match call {
            organizations::Call::Invite { organization, .. } => {
                Rule::AdministratorOf(*organization)
            }
            organizations::Call::ListOrganizations { .. }
            | organizations::Call::CreateOrganization { .. }
            | organizations::Call::OpenOrganization { .. }
            | organizations::Call::GetInvitation { .. }
            | organizations::Call::AcceptInvitation { .. } => Rule::SignedIn,
        },
        Call::Members(call) => match call {
            members::Call::ListMembers { .. }
            | members::Call::SetRole { .. }
            | members::Call::RemoveMember { .. } => Rule::Administrator,
            members::Call::CreateAgent { .. }
            | members::Call::ListAgents { .. }
            | members::Call::RemoveAgent { .. } => Rule::SignedIn,
        },
        Call::Contacts(call) => match call {
            contacts::Call::CreateContactLink { .. }
            | contacts::Call::GetContactLink { .. }
            | contacts::Call::AcceptContactLink { .. }
            | contacts::Call::ListContactLinks { .. }
            | contacts::Call::DeleteContactLink { .. }
            | contacts::Call::PutContact { .. }
            | contacts::Call::ListContacts { .. }
            | contacts::Call::RemoveContact { .. }
            | contacts::Call::ContactsChanged { .. } => Rule::SignedIn,
        },
        Call::Objects(call) => match call {
            objects::Call::HoldObjects { block: held, .. } => block(*held, Access::Edit),
            objects::Call::PutObjects { .. }
            | objects::Call::GetObject { .. }
            | objects::Call::MissingObjects { .. } => Rule::Organization,
        },
        Call::Blocks(call) => match call {
            blocks::Call::CreateBlock { parent, .. } => match parent {
                BlockParent::Block(parent) => block(*parent, Access::Edit),
                BlockParent::Root | BlockParent::Detached => Rule::Organization,
            },
            blocks::Call::SetMetadata { block: of, .. }
            | blocks::Call::Publish { block: of, .. }
            | blocks::Call::SetParent { block: of, .. }
            | blocks::Call::PruneHistory { block: of, .. }
            | blocks::Call::DeleteBlock { block: of, .. } => block(*of, Access::Edit),
            blocks::Call::SetAccess { block: of, .. } => block(*of, Access::KnowExists),
            blocks::Call::ReadContents { blocks, .. } => Rule::Blocks(blocks.clone(), Access::View),
            blocks::Call::ListHistory { block: of, .. } | blocks::Call::Watch { block: of, .. } => {
                block(*of, Access::View)
            }
            blocks::Call::ListAccess { block: of, .. } => block(*of, Access::Manage),
            // Reading a block is listing it (`Connection::list`): the listing is registered
            // before the store answers, which refuses less than `KnowExists` as not found.
            blocks::Call::ReadBlock { .. }
            | blocks::Call::ListBlocks { .. }
            | blocks::Call::Unlist { .. }
            | blocks::Call::CollectDetached { .. } => Rule::Organization,
            blocks::Call::Changes { .. } => Rule::SignedIn,
        },
        Call::Sessions(call) => match call {
            embed::sessions::Call::SessionNew { .. } => Rule::SignedIn,
            embed::sessions::Call::SessionJoin { this, .. }
            | embed::sessions::Call::SessionHeartbeat { this, .. }
            | embed::sessions::Call::SessionRelay { this, .. } => {
                Rule::Session(this.op(), Access::View)
            }
            embed::sessions::Call::SessionClaimOwnership { this, .. } => {
                Rule::Session(this.op(), Access::Edit)
            }
        },
        Call::Keys(call) => match call {
            keys::Call::GetRecoveryKey { .. }
            | keys::Call::SetRecoveryKey { .. }
            | keys::Call::ResealKeys { .. } => Rule::SignedIn,
            keys::Call::CreateEscrow { .. } => Rule::Administrator,
            keys::Call::Rotate { roots, .. } => Rule::Blocks(roots.clone(), Access::Edit),
            keys::Call::GetKeys { .. }
            | keys::Call::PutKeys { .. }
            | keys::Call::GetKeyWork { .. }
            | keys::Call::PinEscrow { .. } => Rule::Organization,
            keys::Call::KeyWorkChanged { .. } => Rule::SignedIn,
        },
    }
}

/// Whether a session's connection may make `call` first: only a call that signs it in. Signing
/// in through the browser is one-shot calls (`http.rs`, `one_shot`), so no connection waits on it.
pub(crate) fn signs_in(call: &embed::Call, local_profiles: bool) -> bool {
    match call {
        embed::Call::Account(embed::account::Call::Authenticate { .. }) => true,
        embed::Call::Account(embed::account::Call::CreateLocalProfile { .. }) => local_profiles,
        _ => false,
    }
}

fn login_disabled() -> ServerError {
    ServerError::Refused(
        ErrorCode::LoginDisabled,
        "This server is not accepting sign-ins.".into(),
    )
}

fn seconds(duration: std::time::Duration) -> u32 {
    u32::try_from(duration.as_secs()).unwrap_or(u32::MAX)
}

fn administrators_only() -> ServerError {
    ServerError::Refused(
        ErrorCode::PermissionDenied,
        "only an administrator of the organization may do this".into(),
    )
}

fn not_joined() -> ServerError {
    ServerError::Refused(
        ErrorCode::PermissionDenied,
        "join the session before speaking in it".into(),
    )
}

pub(crate) enum Late {
    /// The usage of the account this connection follows may have changed.
    Storage,
}

fn wire(error: ServerError) -> Error {
    Error {
        code: error.code(),
        message: error.to_string(),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Route {
    Watch(Place),
    Changes,
    Paired,
    Contacts,
    KeyWork,
    Storage,
    Events { session: OpId, block: Place },
}

struct Streams {
    outlets: Outlets<Route>,
    sessions: HashMap<OpId, Uuid>,
}

impl Default for Streams {
    fn default() -> Self {
        Self {
            outlets: Outlets::within(STREAM_BUFFER_BYTES),
            sessions: HashMap::new(),
        }
    }
}

impl Streams {
    fn send<T: be_wasm_ops::Serialize>(
        &mut self,
        connection: &mut be_wasm_ops::Connection,
        item: T,
        to: impl FnMut(&Route) -> bool,
    ) -> Result<(), ServerError> {
        self.outlets
            .send(connection, &item, to)
            .map_err(|_| ServerError::Socket("the client is not reading what it is sent".into()))
    }

    fn open(
        &mut self,
        connection: &mut be_wasm_ops::Connection,
        stream: be_wasm_ops::Producer<impl Sized>,
        route: Route,
    ) {
        self.open_with(connection, stream, route, Backlog::Queue(STREAM_BUFFER));
    }

    /// Opens a stream, or refuses it once the connection has `MAX_STREAMS_PER_CONNECTION`.
    fn open_with(
        &mut self,
        connection: &mut be_wasm_ops::Connection,
        stream: be_wasm_ops::Producer<impl Sized>,
        route: Route,
        backlog: Backlog,
    ) {
        match self.outlets.len() < MAX_STREAMS_PER_CONNECTION {
            true => self.outlets.insert(stream, route, backlog),
            false => stream.fail(connection, OpError::Denied),
        }
    }

    fn events(&self, session: OpId) -> Option<(OpId, Place)> {
        self.outlets.routes().find_map(|(op, route)| match route {
            Route::Events { session: of, block } if *of == session => Some((op, *block)),
            _ => None,
        })
    }

    fn in_session(&self, block: Place) -> bool {
        self.outlets
            .routes()
            .any(|(_, route)| matches!(route, Route::Events { block: of, .. } if *of == block))
    }
}

pub(crate) struct Connection {
    pub(crate) browser_auth: Arc<browser_auth::Sessions>,
    pub(crate) store: Arc<ServerStore>,
    pub(crate) hub: Arc<WatchHub>,
    pub(crate) registry: Arc<SessionRegistry>,
    pub(crate) client: u64,
    pub(crate) account: Option<Uuid>,
    pub(crate) token: Option<String>,
    /// The `ServerStore::session_changes` version `token` last resolved at.
    checked: Option<u64>,
    pub(crate) header_token: Option<String>,
    pub(crate) identity: Option<Identity>,
    pub(crate) config: ServerConfig,
    /// The address the connection comes from; see [`crate::Services::client_address`].
    pub(crate) address: IpAddr,
    /// Since when the connection has had no account.
    anonymous_since: tokio::time::Instant,
    /// The connection's place among those not signed in, until it signs in.
    anonymous: Option<Anonymous>,
    /// Tells when `anonymous` was let go of for a newer connection.
    pub(crate) eviction: Arc<Eviction>,
    /// Whether the connection serves a session, which must sign in with its first call, or one
    /// one-shot call (`http.rs`, `one_shot`).
    one_shot: bool,
    /// Set once the connection should be closed after what it has answered: its first call was
    /// not one that signs in, or it logged out.
    pub(crate) closing: bool,
}

pub(crate) struct Client {
    pub(crate) connection: Connection,
    pub(crate) peer: Peer,
    pub(crate) pushes: mpsc::Receiver<Push>,
    late: mpsc::UnboundedSender<Late>,
    pub(crate) lates: mpsc::UnboundedReceiver<Late>,
    streams: Streams,
    /// The storage usage last sent to `Route::Storage`, and whether a task follows the payer's.
    storage: Option<AccountStorage>,
    following_storage: bool,
}

impl Client {
    /// A client from `address`, holding `anonymous`, its place among those not signed in, which
    /// `eviction` tells the loss of, until it signs in. A `one_shot` client answers one call that
    /// needs no sign-in; any other must sign in with its first call.
    pub(crate) async fn open(
        services: &Services,
        address: IpAddr,
        one_shot: bool,
        (anonymous, eviction): (Anonymous, Arc<Eviction>),
        sink: impl Fn(Vec<u8>) + Send + Sync + 'static,
    ) -> Self {
        let (sender, pushes) = mpsc::channel(PUSHES);
        let client = services.hub.register(sender).await;
        let (late, lates) = mpsc::unbounded_channel();
        Self {
            connection: Connection {
                browser_auth: Arc::clone(&services.browser_auth),
                store: Arc::clone(&services.store),
                hub: Arc::clone(&services.hub),
                registry: Arc::clone(&services.registry),
                client,
                account: None,
                token: None,
                checked: None,
                header_token: None,
                identity: None,
                config: services.config.clone(),
                address,
                anonymous_since: tokio::time::Instant::now(),
                anonymous: Some(anonymous),
                eviction,
                one_shot,
                closing: false,
            },
            peer: Peer::new(be_wasm_ops::Side::Host, sink),
            pushes,
            late,
            lates,
            streams: Streams::default(),
            storage: None,
            following_storage: false,
        }
    }

    pub(crate) async fn receive(&mut self, frame: &[u8]) -> Result<(), ServerError> {
        let events = self
            .peer
            .receive(frame)
            .map_err(|error| ServerError::Socket(error.to_string()))?;
        self.handle(events).await;
        Ok(())
    }

    pub(crate) async fn handle(&mut self, events: Vec<Incoming>) {
        for event in events {
            match event {
                Incoming::Started(started) => self.serve(started).await,
                Incoming::Canceled { op } => self.canceled(op).await,
                Incoming::Credited { op } => {
                    self.streams.outlets.credited(&mut self.peer.lock(), op);
                }
                _ => {}
            }
        }
    }

    pub(crate) fn pending_pushes(&mut self) -> Vec<Push> {
        let mut pushes = Vec::new();
        while let Ok(push) = self.pushes.try_recv() {
            pushes.push(push);
        }
        pushes
    }

    pub(crate) fn pending_lates(&mut self) -> Vec<Late> {
        let mut lates = Vec::new();
        while let Ok(late) = self.lates.try_recv() {
            lates.push(late);
        }
        lates
    }

    pub(crate) async fn late(&mut self, late: Late) {
        match late {
            Late::Storage => self.send_storage().await,
        }
    }

    /// Sends the account's storage usage to its storage streams when it differs from the last
    /// sent.
    async fn send_storage(&mut self) {
        let Ok(identity) = self.connection.identity() else {
            return;
        };
        let Ok(usage) = self.connection.store.storage_usage(identity).await else {
            return;
        };
        if self.storage.as_ref() == Some(&usage) {
            return;
        }
        let mut lock = self.peer.lock();
        if self
            .streams
            .send(&mut lock, usage.clone(), |route| *route == Route::Storage)
            .is_ok()
        {
            self.storage = Some(usage);
        }
    }

    /// Follows the payer's usage changes for this connection's storage streams, until the
    /// connection closes.
    fn follow_storage(&mut self, payer: Uuid) {
        if std::mem::replace(&mut self.following_storage, true) {
            return;
        }
        let mut changes = self.connection.store.usage_changes.subscribe();
        let late = self.late.clone();
        tokio::spawn(async move {
            loop {
                let changed = tokio::select! {
                    changed = changes.recv() => changed,
                    () = late.closed() => return,
                };
                let ours = match changed {
                    Ok(changed) => changed == payer,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => true,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                };
                if ours && late.send(Late::Storage).is_err() {
                    return;
                }
            }
        });
    }

    pub(crate) async fn close(self) {
        let Connection {
            hub,
            registry,
            client,
            ..
        } = self.connection;
        self.peer.close();
        for (block, state) in registry.leave_all(client).await {
            let participants = registry.participants(block).await;
            hub.send_all(
                &participants,
                client,
                &Push::SessionChanged { block, state },
            )
            .await;
        }
        hub.remove(client).await;
    }

    fn reply<T, E: be_wasm_ops::wire::Encode<Result<T, Error>>>(
        &self,
        reply: Reply<Result<T, Error>, E>,
        result: Result<T, ServerError>,
    ) {
        reply.send(&mut self.peer.lock(), &result.map_err(wire));
    }

    pub(crate) async fn push(&mut self, push: Push) -> Result<(), ServerError> {
        let mut unwatched = None;
        {
            let mut lock = self.peer.lock();
            let streams = &mut self.streams;
            let sessions = |block: Place| move |route: &Route| matches!(route, Route::Events { block: of, .. } if *of == block);
            match push {
                Push::BlockChanged(block) => {
                    streams.send(&mut lock, BlockChange::Changed(block), |route| {
                        *route == Route::Changes
                    })?;
                }
                Push::BlockRemoved { block, revision } => {
                    streams.send(
                        &mut lock,
                        BlockChange::Removed { block, revision },
                        |route| *route == Route::Changes,
                    )?;
                }
                Push::HeadChanged {
                    block,
                    head,
                    author,
                } => {
                    streams.send(&mut lock, BlockEvent::Moved { head, author }, |route| {
                        *route == Route::Watch(block)
                    })?;
                }
                Push::BlockDeleted { block } | Push::Unwatched { block } => {
                    let deleted = matches!(push, Push::BlockDeleted { .. });
                    if deleted {
                        streams.send(&mut lock, BlockEvent::Deleted, |route| {
                            *route == Route::Watch(block)
                        })?;
                    }
                    let ended: Vec<OpId> = streams
                        .outlets
                        .routes()
                        .filter(|(_, route)| **route == Route::Watch(block))
                        .map(|(op, _)| op)
                        .collect();
                    for op in ended {
                        streams
                            .outlets
                            .end(&mut lock, op, (!deleted).then_some(OpError::Denied));
                    }
                    unwatched = Some(block);
                }
                Push::SessionChanged { block, state } => {
                    streams.send(&mut lock, SessionEvent::Changed(state), sessions(block))?;
                }
                Push::Relayed {
                    block,
                    from,
                    payload,
                } => {
                    streams.send(
                        &mut lock,
                        SessionEvent::Relayed {
                            from,
                            payload: payload.to_vec(),
                        },
                        sessions(block),
                    )?;
                }
                Push::Paired(message) => {
                    streams.send(&mut lock, message, |route| *route == Route::Paired)?;
                }
                Push::ContactsChanged => {
                    streams.send(&mut lock, (), |route| *route == Route::Contacts)?;
                }
                Push::KeyWorkChanged => {
                    streams.send(&mut lock, (), |route| *route == Route::KeyWork)?;
                }
            }
        }
        if let Some(block) = unwatched {
            self.connection
                .hub
                .unwatch(block, self.connection.client)
                .await;
        }
        Ok(())
    }

    async fn canceled(&mut self, op: OpId) {
        let canceled = Some(OpError::Canceled);
        let left = {
            let mut lock = self.peer.lock();
            let streams = &mut self.streams;
            if streams.sessions.remove(&op).is_some() {
                let joined = streams.events(op);
                if let Some((events, _)) = joined {
                    streams.outlets.end(&mut lock, events, canceled);
                }
                embed::sessions::Session(op).end(&mut lock);
                joined
                    .map(|(_, block)| block)
                    .filter(|block| !streams.in_session(*block))
                    .map(|block| (block, true))
            } else {
                match streams.outlets.end(&mut lock, op, canceled) {
                    Some(Route::Watch(block)) => {
                        let watched = streams
                            .outlets
                            .routes()
                            .any(|(_, route)| *route == Route::Watch(block));
                        (!watched).then_some((block, false))
                    }
                    _ => None,
                }
            }
        };
        match left {
            Some((block, false)) => {
                self.connection
                    .hub
                    .unwatch(block, self.connection.client)
                    .await;
            }
            Some((block, true)) => {
                let connection = &self.connection;
                if let Some(state) = connection.registry.leave(block, connection.client).await {
                    connection.announce(block, &state).await;
                }
            }
            None => {}
        }
    }

    async fn prelude(&mut self) -> Result<(), ServerError> {
        let connection = &mut self.connection;
        connection.signed_in().await?;
        if connection.identity.is_some() {
            connection.identity = connection.hub.identity(connection.client).await;
        }
        Ok(())
    }

    async fn serve(&mut self, started: Started) {
        let Some(call) = embed::Call::accept(&mut self.peer.lock(), started) else {
            return;
        };
        let call = match call {
            embed::Call::Connection(embed::connection::Call::Ping { reply }) => {
                return reply.send(&mut self.peer.lock(), &());
            }
            call => call,
        };
        if self.connection.account.is_none()
            && !self.connection.one_shot
            && !signs_in(&call, self.connection.config.local_profiles)
        {
            tracing::debug!("closing a connection whose first call does not sign it in");
            self.connection.closing = true;
            return;
        }
        let prelude = self.prelude().await;
        let rule = rule(&call);
        let prelude = match prelude {
            Ok(()) => self.admit(rule).await,
            refused => refused,
        };
        match call {
            embed::Call::Connection(_) => {}
            embed::Call::Account(call) => self.account(call, prelude).await,
            embed::Call::Organizations(call) => self.organizations(call, prelude).await,
            embed::Call::Members(call) => self.members(call, prelude).await,
            embed::Call::Contacts(call) => self.contacts(call, prelude).await,
            embed::Call::Objects(call) => self.objects(call, prelude).await,
            embed::Call::Blocks(call) => self.blocks(call, prelude).await,
            embed::Call::Sessions(call) => self.sessions(call, prelude).await,
            embed::Call::Keys(call) => self.keys(call, prelude).await,
        }
    }

    /// Applies a call's rule before its handler runs; each handler starts with `prelude?`, the
    /// answer this gives.
    async fn admit(&self, rule: Rule) -> Result<(), ServerError> {
        let connection = &self.connection;
        match rule {
            Rule::Anyone => Ok(()),
            Rule::SignedIn => connection.account().map(drop),
            Rule::Organization => connection.identity().map(drop),
            Rule::Administrator => match connection.identity()?.is_administrator() {
                true => Ok(()),
                false => Err(administrators_only()),
            },
            Rule::AdministratorOf(organization) => {
                let account = connection.account()?;
                match connection.store.membership(account, organization).await? {
                    be_block_protocol::OrganizationRole::Administrator => Ok(()),
                    be_block_protocol::OrganizationRole::Editor => Err(administrators_only()),
                }
            }
            Rule::Blocks(blocks, level) => {
                let identity = connection.identity()?;
                connection
                    .store
                    .require_access(identity, blocks, level)
                    .await
            }
            Rule::Session(session, level) => {
                let identity = connection.identity()?;
                let place = match self.streams.events(session) {
                    Some((_, place)) => place,
                    None => match self.streams.sessions.get(&session) {
                        Some(block) => Place {
                            organization: identity.organization,
                            block: *block,
                        },
                        None => return Err(not_joined()),
                    },
                };
                if place.organization != identity.organization {
                    return Err(ServerError::Refused(
                        ErrorCode::PermissionDenied,
                        "this session is in another organization than the one open".into(),
                    ));
                }
                connection
                    .store
                    .require_access(identity, vec![place.block], level)
                    .await
            }
        }
    }

    async fn account(&mut self, call: embed::account::Call, prelude: Result<(), ServerError>) {
        use embed::account::Call;
        match call {
            Call::BeginBrowserLogin { reply } => {
                let begun = prelude.and_then(|()| {
                    let connection = &self.connection;
                    if !connection.config.allow_login {
                        return Err(login_disabled());
                    }
                    let (code, secret, interval, expires) =
                        connection.browser_auth.begin(connection.address);
                    Ok(BrowserLogin {
                        code,
                        secret,
                        interval: seconds(interval),
                        expires: seconds(expires),
                    })
                });
                self.reply(reply, begun);
            }
            Call::PollBrowserLogin {
                reply,
                code,
                secret,
            } => {
                let polled = async {
                    prelude?;
                    let connection = &self.connection;
                    if !connection.config.allow_login {
                        return Err(login_disabled());
                    }
                    Ok(match connection.browser_auth.poll(&code, &secret)? {
                        browser_auth::Polled::Pending => BrowserLoginState::Pending,
                        browser_auth::Polled::Denied => BrowserLoginState::Denied,
                        browser_auth::Polled::Expired => BrowserLoginState::Expired,
                        browser_auth::Polled::Approved { account, issuer } => {
                            let token = connection
                                .store
                                .claim_browser_session(account, &issuer)
                                .await?;
                            let profile = connection.store.resolve_token(&token).await?;
                            BrowserLoginState::Approved(Authenticated {
                                account: profile.account,
                                email: profile.email,
                                display_name: profile.display_name,
                                token,
                            })
                        }
                    })
                }
                .await;
                self.reply(reply, polled);
            }
            Call::CreateLocalProfile {
                reply,
                display_name,
            } => {
                let created = async {
                    prelude?;
                    if !self.connection.config.local_profiles {
                        return Err(ServerError::Refused(
                            ErrorCode::InvalidRequest,
                            "local profiles are unavailable on this server".into(),
                        ));
                    }
                    let (profile, token) = self
                        .connection
                        .store
                        .create_local_profile(&display_name)
                        .await?;
                    Ok(self.connection.authenticated(profile, token).await)
                }
                .await;
                self.reply(reply, created);
            }
            Call::Authenticate { reply, token } => {
                let authenticated = async {
                    prelude?;
                    let header = token.is_none();
                    let token = token
                        .or_else(|| self.connection.header_token.clone())
                        .ok_or_else(|| {
                            ServerError::Refused(
                                ErrorCode::NotAuthenticated,
                                "no Bearer account token was supplied in Authorization".into(),
                            )
                        })?;
                    let profile = self.connection.store.resolve_token(&token).await?;
                    let mut authenticated = self.connection.authenticated(profile, token).await;
                    if header {
                        authenticated.token.clear();
                    }
                    Ok(authenticated)
                }
                .await;
                self.reply(reply, authenticated);
            }
            Call::Logout { reply } => {
                let connection = &mut self.connection;
                let logged_out = async {
                    prelude?;
                    if let Some(token) = connection.token.take() {
                        connection.store.logout(&token).await?;
                    }
                    connection.account = None;
                    connection.closing = true;
                    connection.identity = None;
                    connection.hub.leave_account(connection.client).await;
                    Ok(())
                }
                .await;
                self.reply(reply, logged_out);
            }
            Call::StorageUsage { reply } => {
                let connection = &self.connection;
                let usage = async {
                    prelude?;
                    connection.store.storage_usage(connection.identity()?).await
                }
                .await;
                self.reply(reply, usage);
            }
            Call::Pair { reply, to, payload } => {
                let connection = &self.connection;
                let paired = async {
                    prelude?;
                    let account = connection.account()?;
                    crate::limits::at_most("a pairing message", payload.len(), MAX_PAIRING_BYTES)?;
                    let message = Push::Paired(PairingMessage {
                        from: connection.client,
                        payload,
                    });
                    let sent = connection
                        .hub
                        .send_account(account, connection.client, to, &message)
                        .await;
                    if to.is_some() && sent == 0 {
                        return Err(ServerError::Refused(
                            ErrorCode::InvalidRequest,
                            "that device is no longer connected".into(),
                        ));
                    }
                    Ok(())
                }
                .await;
                self.reply(reply, paired);
            }
            Call::Paired { stream } => {
                if prelude.is_err() {
                    return stream.fail(&mut self.peer.lock(), OpError::Denied);
                }
                self.streams
                    .open(&mut self.peer.lock(), stream, Route::Paired);
            }
            Call::StorageChanged { stream } => {
                let connection = &self.connection;
                let payer = async {
                    prelude?;
                    connection.store.payer(connection.account()?).await
                }
                .await;
                match payer {
                    Ok(payer) => {
                        {
                            let mut lock = self.peer.lock();
                            let earlier: Vec<OpId> = self
                                .streams
                                .outlets
                                .routes()
                                .filter(|(_, route)| **route == Route::Storage)
                                .map(|(op, _)| op)
                                .collect();
                            for op in earlier {
                                self.streams.outlets.end(&mut lock, op, None);
                            }
                            self.streams.open_with(
                                &mut lock,
                                stream,
                                Route::Storage,
                                Backlog::Latest,
                            );
                        }
                        self.storage = None;
                        self.follow_storage(payer);
                        self.send_storage().await;
                    }
                    Err(error) => {
                        let mut lock = self.peer.lock();
                        tracing::debug!(%error, "refused a storage stream");
                        stream.fail(&mut lock, OpError::Denied);
                    }
                }
            }
        }
    }

    async fn organizations(
        &mut self,
        call: embed::organizations::Call,
        prelude: Result<(), ServerError>,
    ) {
        use embed::organizations::Call;
        match call {
            Call::ListOrganizations { reply } => {
                let connection = &self.connection;
                let listed = async {
                    prelude?;
                    connection
                        .store
                        .list_organizations(connection.account()?)
                        .await
                }
                .await;
                self.reply(reply, listed);
            }
            Call::CreateOrganization { reply, name } => {
                let connection = &self.connection;
                let created = async {
                    prelude?;
                    connection
                        .store
                        .create_organization(connection.account()?, &name)
                        .await
                }
                .await;
                self.reply(reply, created);
            }
            Call::OpenOrganization {
                reply,
                organization,
            } => {
                let opened = async {
                    prelude?;
                    self.connection.open(organization).await
                }
                .await;
                self.reply(reply, opened);
            }
            Call::Invite {
                reply,
                invitation,
                organization,
                email,
                role,
                sealed,
                code,
                escrow,
            } => {
                let connection = &self.connection;
                let invited = async {
                    prelude?;
                    connection
                        .store
                        .invite(
                            connection.account()?,
                            invitation,
                            organization,
                            &email,
                            role,
                            sealed,
                            code,
                            escrow,
                        )
                        .await
                }
                .await;
                self.reply(reply, invited);
            }
            Call::GetInvitation { reply, invitation } => {
                let connection = &self.connection;
                let found = async {
                    prelude?;
                    connection
                        .store
                        .invitation(connection.account()?, invitation)
                        .await
                }
                .await;
                self.reply(reply, found);
            }
            Call::AcceptInvitation {
                reply,
                invitation,
                card,
                contact,
                escrow,
            } => {
                let connection = &self.connection;
                let accepted = async {
                    prelude?;
                    let account = connection.account()?;
                    let inviter = connection
                        .store
                        .accept_invitation(account, invitation, card, contact, escrow)
                        .await?;
                    connection.contacts_changed(&[account, inviter]).await;
                    Ok(())
                }
                .await;
                self.reply(reply, accepted);
            }
        }
    }

    async fn members(&mut self, call: embed::members::Call, prelude: Result<(), ServerError>) {
        use embed::members::Call;
        let connection = &self.connection;
        match call {
            Call::ListMembers { reply } => {
                let listed = async {
                    prelude?;
                    connection.store.list_members(connection.identity()?).await
                }
                .await;
                self.reply(reply, listed);
            }
            Call::SetRole {
                reply,
                account,
                role,
            } => {
                let set = async {
                    prelude?;
                    let identity = connection.identity()?;
                    connection.store.set_role(identity, account, role).await?;
                    if account != identity.account {
                        connection
                            .hub
                            .drop_member(identity.organization, account)
                            .await;
                    }
                    connection
                        .key_work_changed(identity.organization, BTreeSet::new())
                        .await;
                    Ok(())
                }
                .await;
                self.reply(reply, set);
            }
            Call::RemoveMember { reply, account } => {
                let removed = async {
                    prelude?;
                    let identity = connection.identity()?;
                    let revoked = connection.store.remove_member(identity, account).await?;
                    connection
                        .hub
                        .drop_member(identity.organization, account)
                        .await;
                    connection.revoked(&revoked, &[account]).await;
                    connection
                        .key_work_changed(identity.organization, BTreeSet::new())
                        .await;
                    Ok(())
                }
                .await;
                self.reply(reply, removed);
            }
            Call::CreateAgent {
                reply,
                organization,
                name,
                recovery,
                contact,
                escrow,
            } => {
                let created = async {
                    prelude?;
                    let account = connection.account()?;
                    let (agent, token) = connection
                        .store
                        .create_agent(account, organization, &name, recovery, contact, escrow)
                        .await?;
                    connection.contacts_changed(&[account]).await;
                    Ok(CreatedAgent { agent, token })
                }
                .await;
                self.reply(reply, created);
            }
            Call::ListAgents {
                reply,
                organization,
            } => {
                let listed = async {
                    prelude?;
                    connection
                        .store
                        .list_agents(connection.account()?, organization)
                        .await
                }
                .await;
                self.reply(reply, listed);
            }
            Call::RemoveAgent {
                reply,
                organization,
                account,
            } => {
                let removed = async {
                    prelude?;
                    let revoked = connection
                        .store
                        .remove_agent(connection.account()?, organization, account)
                        .await?;
                    connection.hub.drop_member(organization, account).await;
                    connection.revoked(&revoked, &[account]).await;
                    Ok(())
                }
                .await;
                self.reply(reply, removed);
            }
        }
    }

    async fn contacts(&mut self, call: embed::contacts::Call, prelude: Result<(), ServerError>) {
        use embed::contacts::Call;
        let connection = &self.connection;
        match call {
            Call::CreateContactLink {
                reply,
                link,
                card,
                code,
                grant,
            } => {
                let created = async {
                    prelude?;
                    connection
                        .store
                        .create_contact_link(connection.account()?, link, card, code, grant)
                        .await
                }
                .await;
                self.reply(reply, created);
            }
            Call::GetContactLink { reply, link } => {
                let found = async {
                    prelude?;
                    connection
                        .store
                        .contact_link(connection.account()?, link)
                        .await
                }
                .await;
                self.reply(reply, found);
            }
            Call::AcceptContactLink {
                reply,
                link,
                card,
                contact,
                key,
            } => {
                let accepted = async {
                    prelude?;
                    let account = connection.account()?;
                    let accepted = connection
                        .store
                        .accept_contact_link(account, link, card, contact, key)
                        .await?;
                    if let Some(granted) = accepted.granted {
                        connection
                            .reannounce_in(
                                granted.organization,
                                vec![granted.block],
                                true,
                                granted.before,
                            )
                            .await;
                    }
                    connection
                        .contacts_changed(&[account, accepted.owner])
                        .await;
                    Ok(())
                }
                .await;
                self.reply(reply, accepted);
            }
            Call::ListContactLinks { reply } => {
                let listed = async {
                    prelude?;
                    connection.store.contact_links(connection.account()?).await
                }
                .await;
                self.reply(reply, listed);
            }
            Call::DeleteContactLink { reply, link } => {
                let deleted = async {
                    prelude?;
                    connection
                        .store
                        .delete_contact_link(connection.account()?, link)
                        .await
                }
                .await;
                self.reply(reply, deleted);
            }
            Call::PutContact {
                reply,
                contact,
                sealed,
            } => {
                let put = async {
                    prelude?;
                    let account = connection.account()?;
                    connection
                        .store
                        .put_contact(account, contact, sealed)
                        .await?;
                    connection.contacts_changed(&[account]).await;
                    Ok(())
                }
                .await;
                self.reply(reply, put);
            }
            Call::ListContacts { reply } => {
                let listed = async {
                    prelude?;
                    connection.store.list_contacts(connection.account()?).await
                }
                .await;
                self.reply(reply, listed);
            }
            Call::RemoveContact { reply, account } => {
                let removed = async {
                    prelude?;
                    let caller = connection.account()?;
                    let revoked = connection.store.remove_contact(caller, account).await?;
                    connection.revoked(&revoked, &[caller, account]).await;
                    connection.contacts_changed(&[caller, account]).await;
                    Ok(())
                }
                .await;
                self.reply(reply, removed);
            }
            Call::ContactsChanged { stream } => {
                if prelude.is_err() {
                    return stream.fail(&mut self.peer.lock(), OpError::Denied);
                }
                self.streams.open_with(
                    &mut self.peer.lock(),
                    stream,
                    Route::Contacts,
                    Backlog::Latest,
                );
            }
        }
    }

    async fn objects(&mut self, call: embed::objects::Call, prelude: Result<(), ServerError>) {
        use embed::objects::Call;
        let connection = &self.connection;
        match call {
            Call::PutObjects { reply, objects } => {
                let stored = async {
                    prelude?;
                    let identity = connection.identity()?;
                    connection.store.put_objects(identity, objects).await
                }
                .await;
                self.reply(reply, stored);
            }
            Call::GetObject { reply, hash } => {
                let found = async {
                    prelude?;
                    let identity = connection.identity()?;
                    match connection.store.may_read_object(identity, hash).await? {
                        true => connection.store.get_object(hash).await,
                        false => Ok(None),
                    }
                }
                .await;
                self.reply(reply, found);
            }
            Call::MissingObjects { reply, hashes } => {
                let missing = async {
                    prelude?;
                    connection
                        .store
                        .missing_objects(connection.identity()?, hashes)
                        .await
                }
                .await;
                self.reply(reply, missing);
            }
            Call::HoldObjects {
                reply,
                block,
                objects,
            } => {
                let held = async {
                    prelude?;
                    connection
                        .store
                        .hold_objects(connection.identity()?, block, objects)
                        .await
                }
                .await;
                self.reply(reply, held);
            }
        }
    }

    async fn blocks(&mut self, call: embed::blocks::Call, prelude: Result<(), ServerError>) {
        use embed::blocks::Call;
        let connection = &self.connection;
        match call {
            Call::CreateBlock {
                reply,
                block,
                content_type,
                parent,
                metadata,
                references,
                key,
                keys,
            } => {
                let created = async {
                    prelude?;
                    let identity = connection.identity()?;
                    let block = connection
                        .store
                        .create_block(
                            identity,
                            block,
                            content_type,
                            parent,
                            metadata,
                            references,
                            key,
                            keys,
                        )
                        .await?;
                    let mut announced = vec![block.id];
                    announced.extend(block.references.iter().copied());
                    connection
                        .reannounce(identity, announced, false, BTreeSet::new())
                        .await;
                    Ok(block)
                }
                .await;
                self.reply(reply, created);
            }
            Call::SetMetadata {
                reply,
                block,
                metadata,
                references,
            } => {
                let set = async {
                    prelude?;
                    let identity = connection.identity()?;
                    let (block, relinked) = connection
                        .store
                        .set_metadata(identity, block, metadata, references)
                        .await?;
                    let mut announced = vec![block.id];
                    announced.extend(relinked);
                    connection
                        .reannounce(identity, announced, false, BTreeSet::new())
                        .await;
                    Ok(block)
                }
                .await;
                self.reply(reply, set);
            }
            Call::Publish {
                reply,
                block,
                commit,
                expected,
                parents,
                chunks,
                time,
                pinned,
                references,
                metadata,
                objects,
            } => {
                let published = async {
                    prelude?;
                    let identity = connection.identity()?;
                    connection.store.put_objects(identity, objects).await?;
                    let outcome = connection
                        .store
                        .publish(
                            identity, block, commit, expected, parents, chunks, time, pinned,
                            references, metadata,
                        )
                        .await?;
                    if let PublishOutcome::Published(_, Some(unlinked)) = &outcome {
                        let mut blocks = vec![block];
                        blocks.extend(unlinked);
                        connection
                            .reannounce(identity, blocks, false, BTreeSet::new())
                            .await;
                    }
                    Ok(match outcome {
                        PublishOutcome::Published(head, _) => {
                            let place = Place {
                                organization: identity.organization,
                                block,
                            };
                            connection
                                .hub
                                .broadcast(
                                    place,
                                    connection.client,
                                    Push::HeadChanged {
                                        block: place,
                                        head,
                                        author: identity.account,
                                    },
                                )
                                .await;
                            be_block_protocol::PublishOutcome::Published(head)
                        }
                        PublishOutcome::Rejected(head) => {
                            be_block_protocol::PublishOutcome::Rejected(head)
                        }
                    })
                }
                .await;
                self.reply(reply, published);
            }
            Call::ReadBlock { reply, block } => {
                let read = async {
                    prelude?;
                    connection
                        .list(Listing::Block(block))
                        .await?
                        .pop()
                        .ok_or(ServerError::Corrupt)
                }
                .await;
                self.reply(reply, read);
            }
            Call::ReadContents {
                reply,
                blocks,
                held,
            } => {
                let read = async {
                    prelude?;
                    let (blocks, hashes) = connection
                        .store
                        .read_contents(connection.identity()?, blocks, held)
                        .await?;
                    let objects = connection.store.get_objects(hashes, CONTENTS_BYTES).await?;
                    Ok(Contents { blocks, objects })
                }
                .await;
                self.reply(reply, read);
            }
            Call::SetParent {
                reply,
                block,
                parent,
                keys,
            } => {
                let moved = async {
                    prelude?;
                    let identity = connection.identity()?;
                    let (before, lost) = connection
                        .store
                        .set_parent(identity, block, parent, keys)
                        .await?;
                    connection
                        .reannounce(identity, vec![block], true, before.clone())
                        .await;
                    if !lost.is_empty() {
                        connection
                            .key_work_changed(identity.organization, before)
                            .await;
                        for account in lost {
                            connection
                                .evict_sessions(identity.organization, account)
                                .await;
                        }
                    }
                    Ok(())
                }
                .await;
                self.reply(reply, moved);
            }
            Call::ListBlocks { reply, listing } => {
                let listed = async {
                    prelude?;
                    connection.list(listing).await
                }
                .await;
                self.reply(reply, listed);
            }
            Call::Unlist { reply, listing } => {
                let unlisted = async {
                    prelude?;
                    connection.identity()?;
                    connection.hub.unlist(connection.client, listing).await;
                    Ok(())
                }
                .await;
                self.reply(reply, unlisted);
            }
            Call::Changes { stream } => {
                if prelude.is_err() {
                    return stream.fail(&mut self.peer.lock(), OpError::Denied);
                }
                self.streams
                    .open(&mut self.peer.lock(), stream, Route::Changes);
            }
            Call::ListHistory { reply, block } => {
                let listed = async {
                    prelude?;
                    connection
                        .store
                        .list_history(connection.identity()?, block)
                        .await
                }
                .await;
                self.reply(reply, listed);
            }
            Call::PruneHistory { reply, block, drop } => {
                let pruned = async {
                    prelude?;
                    let objects = connection
                        .store
                        .prune_history(connection.identity()?, block, drop)
                        .await?;
                    Ok(objects as u64)
                }
                .await;
                self.reply(reply, pruned);
            }
            Call::DeleteBlock { reply, block } => {
                let deleted = async {
                    prelude?;
                    let identity = connection.identity()?;
                    let before = connection.store.delete_block(identity, block).await?;
                    connection
                        .reannounce(identity, vec![block], true, before)
                        .await;
                    let place = Place {
                        organization: identity.organization,
                        block,
                    };
                    connection
                        .hub
                        .broadcast(
                            place,
                            connection.client,
                            Push::BlockDeleted { block: place },
                        )
                        .await;
                    Ok(())
                }
                .await;
                self.reply(reply, deleted);
            }
            Call::CollectDetached { reply } => {
                let collected = async {
                    prelude?;
                    let identity = connection.identity()?;
                    let collected = connection.store.collect_detached(identity).await?;
                    for block in &collected.blocks {
                        connection
                            .hub
                            .removed(identity.organization, *block, collected.revision)
                            .await;
                    }
                    Ok(Collected {
                        blocks: collected.blocks,
                        objects: collected.objects as u64,
                    })
                }
                .await;
                self.reply(reply, collected);
            }
            Call::SetAccess {
                reply,
                block,
                account,
                access,
            } => {
                let set = async {
                    prelude?;
                    let identity = connection.identity()?;
                    let before = connection
                        .store
                        .set_access(identity, block, account, access)
                        .await?;
                    connection
                        .revoked(
                            &[contacts::Granted {
                                organization: identity.organization,
                                block,
                                before,
                            }],
                            &[account],
                        )
                        .await;
                    Ok(())
                }
                .await;
                self.reply(reply, set);
            }
            Call::ListAccess { reply, block } => {
                let listed = async {
                    prelude?;
                    connection
                        .store
                        .list_access(connection.identity()?, block)
                        .await
                }
                .await;
                self.reply(reply, listed);
            }
            Call::Watch { stream, block } => {
                let allowed = prelude.and_then(|()| connection.identity());
                let Ok(identity) = allowed else {
                    return stream.fail(&mut self.peer.lock(), OpError::Denied);
                };
                let place = Place {
                    organization: identity.organization,
                    block,
                };
                if self.streams.outlets.len() >= MAX_STREAMS_PER_CONNECTION {
                    return stream.fail(&mut self.peer.lock(), OpError::Denied);
                }
                connection.hub.watch(place, connection.client).await;
                self.streams
                    .open(&mut self.peer.lock(), stream, Route::Watch(place));
            }
        }
    }

    async fn sessions(&mut self, call: embed::sessions::Call, prelude: Result<(), ServerError>) {
        use embed::sessions::Call;
        match call {
            Call::SessionNew { resource, block } => {
                if prelude.is_err() {
                    return self
                        .peer
                        .lock()
                        .complete(resource.op(), be_wasm_ops::Outcome::Err(OpError::Denied));
                }
                if self.streams.sessions.len() >= MAX_SESSIONS_PER_CONNECTION {
                    return self
                        .peer
                        .lock()
                        .complete(resource.op(), be_wasm_ops::Outcome::Err(OpError::Denied));
                }
                self.streams.sessions.insert(resource.op(), block);
            }
            Call::SessionJoin { this, stream } => {
                let session = this.op();
                let Some(&block) = self.streams.sessions.get(&session) else {
                    stream.fail(&mut self.peer.lock(), OpError::Canceled);
                    return;
                };
                let connection = &self.connection;
                let joined = async {
                    prelude?;
                    let place = Place {
                        organization: connection.identity()?.organization,
                        block,
                    };
                    let state = connection.registry.join(place, connection.client).await;
                    connection.announce(place, &state).await;
                    Ok::<_, ServerError>((state, place))
                }
                .await;
                let op = stream.op();
                let mut lock = self.peer.lock();
                let streams = &mut self.streams;
                if let Some((before, _)) = streams.events(session) {
                    streams
                        .outlets
                        .end(&mut lock, before, Some(OpError::Canceled));
                }
                let place = joined.as_ref().map_or(
                    Place {
                        organization: Uuid::nil(),
                        block,
                    },
                    |(_, place)| *place,
                );
                streams.open(
                    &mut lock,
                    stream,
                    Route::Events {
                        session,
                        block: place,
                    },
                );
                let only = |route: &Route| {
                    *route
                        == Route::Events {
                            session,
                            block: place,
                        }
                };
                let event = match joined {
                    Ok((state, _)) => SessionEvent::Joined {
                        client: self.connection.client,
                        state,
                    },
                    Err(error) => {
                        let _ = streams.send(&mut lock, SessionEvent::Refused(wire(error)), only);
                        streams.outlets.end(&mut lock, op, None);
                        return;
                    }
                };
                let _ = streams.send(&mut lock, event, only);
            }
            Call::SessionClaimOwnership {
                this,
                reply,
                generation,
            } => {
                let block = self.joined(this.op());
                let connection = &self.connection;
                let claimed = async {
                    prelude?;
                    let block = block?;
                    let (claim, state) = connection
                        .registry
                        .claim(block, connection.client, generation)
                        .await;
                    if matches!(claim, Claim::Granted { .. }) {
                        connection.announce(block, &state).await;
                    }
                    Ok(state)
                }
                .await;
                self.reply(reply, claimed);
            }
            Call::SessionHeartbeat {
                this,
                reply,
                generation,
                clean_at,
            } => {
                let block = self.joined(this.op());
                let connection = &self.connection;
                let beat = async {
                    prelude?;
                    let block = block?;
                    Ok::<SessionState, ServerError>(
                        match connection
                            .registry
                            .heartbeat(block, connection.client, generation, clean_at)
                            .await
                        {
                            Some(state) => state,
                            None => connection.registry.state(block).await,
                        },
                    )
                }
                .await;
                self.reply(reply, beat);
            }
            Call::SessionRelay {
                this,
                reply,
                to,
                payload,
            } => {
                let block = self.joined(this.op());
                let connection = &self.connection;
                let relayed = async {
                    prelude?;
                    let block = block?;
                    let participants = connection.registry.participants(block).await;
                    if !participants.contains(&connection.client) {
                        return Err(not_joined());
                    }
                    crate::limits::at_most("a relayed message", payload.len(), MAX_RELAY_BYTES)?;
                    let message = Push::Relayed {
                        block,
                        from: connection.client,
                        payload: payload.into(),
                    };
                    match to {
                        Some(target) if participants.contains(&target) => {
                            connection.hub.send_to(target, message).await;
                        }
                        Some(_) => {
                            return Err(ServerError::Refused(
                                ErrorCode::InvalidRequest,
                                "that client is not in this session".into(),
                            ));
                        }
                        None => {
                            connection
                                .hub
                                .send_all(&participants, connection.client, &message)
                                .await;
                        }
                    }
                    Ok(())
                }
                .await;
                self.reply(reply, relayed);
            }
        }
    }

    fn joined(&self, session: OpId) -> Result<Place, ServerError> {
        self.streams
            .events(session)
            .map(|(_, block)| block)
            .ok_or_else(not_joined)
    }

    async fn keys(&mut self, call: embed::keys::Call, prelude: Result<(), ServerError>) {
        use embed::keys::Call;
        let connection = &self.connection;
        match call {
            Call::GetKeys { reply, blocks } => {
                let keys = async {
                    prelude?;
                    connection
                        .store
                        .get_keys(connection.identity()?, blocks)
                        .await
                }
                .await;
                self.reply(reply, keys);
            }
            Call::PutKeys { reply, keys } => {
                let put = async {
                    prelude?;
                    connection
                        .store
                        .put_keys(connection.identity()?, keys)
                        .await
                }
                .await;
                self.reply(reply, put);
            }
            Call::GetRecoveryKey { reply } => {
                let found = async {
                    prelude?;
                    let (public, sealed, own) =
                        connection.store.recovery_key(connection.account()?).await?;
                    Ok(RecoveryKey {
                        public,
                        sealed,
                        own,
                    })
                }
                .await;
                self.reply(reply, found);
            }
            Call::SetRecoveryKey {
                reply,
                public,
                sealed,
                own,
                changes,
            } => {
                let set = async {
                    prelude?;
                    let account = connection.account()?;
                    let told: Vec<Uuid> = changes.iter().map(|change| change.contact).collect();
                    connection
                        .store
                        .set_recovery_key(account, public, sealed, own, changes)
                        .await?;
                    connection.contacts_changed(&told).await;
                    Ok(())
                }
                .await;
                self.reply(reply, set);
            }
            Call::ResealKeys { reply, keys } => {
                let resealed = async {
                    prelude?;
                    connection
                        .store
                        .reseal_keys(connection.account()?, keys)
                        .await
                }
                .await;
                self.reply(reply, resealed);
            }
            Call::GetKeyWork { reply } => {
                let work = async {
                    prelude?;
                    connection.store.key_work(connection.identity()?).await
                }
                .await;
                self.reply(reply, work);
            }
            Call::KeyWorkChanged { stream } => {
                if prelude.is_err() {
                    return stream.fail(&mut self.peer.lock(), OpError::Denied);
                }
                self.streams.open_with(
                    &mut self.peer.lock(),
                    stream,
                    Route::KeyWork,
                    Backlog::Latest,
                );
            }
            Call::Rotate { reply, roots, keys } => {
                let rotated = async {
                    prelude?;
                    let identity = connection.identity()?;
                    connection
                        .store
                        .rotate(identity, roots.clone(), keys)
                        .await?;
                    connection
                        .reannounce(identity, roots, true, BTreeSet::new())
                        .await;
                    Ok(())
                }
                .await;
                self.reply(reply, rotated);
            }
            Call::CreateEscrow {
                reply,
                escrow,
                keys,
            } => {
                let created = async {
                    prelude?;
                    connection
                        .store
                        .create_escrow(connection.identity()?, escrow, keys)
                        .await
                }
                .await;
                self.reply(reply, created);
            }
            Call::PinEscrow { reply, sealed } => {
                let pinned = async {
                    prelude?;
                    connection
                        .store
                        .pin_escrow(connection.identity()?, sealed)
                        .await
                }
                .await;
                self.reply(reply, pinned);
            }
        }
    }
}

impl Connection {
    /// When the connection is closed unless it signs in first, or none once it has an account.
    /// One let go of to make room for a newer connection closes at once.
    pub(crate) fn sign_in_deadline(&self) -> Option<tokio::time::Instant> {
        if self.account.is_some() {
            return None;
        }
        if self.eviction.evicted() {
            return Some(self.anonymous_since);
        }
        Some(self.anonymous_since + self.config.sign_in_timeout)
    }

    async fn authenticated(&mut self, profile: store::Profile, token: String) -> Authenticated {
        self.hub.join_account(self.client, profile.account).await;
        self.account = Some(profile.account);
        self.anonymous = None;
        self.token = Some(token.clone());
        self.checked = None;
        Authenticated {
            account: profile.account,
            email: profile.email,
            display_name: profile.display_name,
            token,
        }
    }

    async fn open(&mut self, organization: Uuid) -> Result<OrganizationOpened, ServerError> {
        let account = self.account()?;
        let role = self.store.membership(account, organization).await?;
        let identity = Identity {
            account,
            organization,
            role,
        };
        self.identity = Some(identity);
        self.hub.join_organization(self.client, identity).await;
        let (kind, escrows, pinned) = self.store.opened(account, organization).await?;
        Ok(OrganizationOpened {
            organization,
            role,
            kind,
            escrows,
            pinned,
            server: self.store.server_account(),
        })
    }

    /// Fails once the connection's token no longer signs it in. It resolves the token again
    /// only after a session changed (`ServerStore::sessions_changed`), not on every call.
    pub(crate) async fn signed_in(&mut self) -> Result<(), ServerError> {
        let Some(token) = &self.token else {
            return Ok(());
        };
        let version = *self.store.session_changes.borrow();
        if self.checked == Some(version) {
            return Ok(());
        }
        self.store.resolve_token(token).await?;
        self.checked = Some(version);
        Ok(())
    }

    fn identity(&self) -> Result<Identity, ServerError> {
        self.identity.ok_or_else(|| {
            ServerError::Refused(
                ErrorCode::NotAuthenticated,
                "open an organization before using it".into(),
            )
        })
    }

    fn account(&self) -> Result<Uuid, ServerError> {
        self.account.ok_or_else(|| {
            ServerError::Refused(
                ErrorCode::NotAuthenticated,
                "authenticate before using this connection".into(),
            )
        })
    }

    async fn revoked(&self, revoked: &[contacts::Granted], accounts: &[Uuid]) {
        let mut organizations = BTreeSet::new();
        for granted in revoked {
            self.reannounce_in(
                granted.organization,
                vec![granted.block],
                true,
                granted.before.clone(),
            )
            .await;
            self.key_work_changed(granted.organization, granted.before.clone())
                .await;
            organizations.insert(granted.organization);
        }
        for organization in organizations {
            for account in accounts {
                self.evict_sessions(organization, *account).await;
            }
        }
    }

    async fn key_work_changed(&self, organization: Uuid, accounts: BTreeSet<Uuid>) {
        for (client, _) in self.hub.audience(organization, &accounts).await {
            if client != self.client {
                self.hub.send_to(client, Push::KeyWorkChanged).await;
            }
        }
    }

    async fn evict_sessions(&self, organization: Uuid, account: Uuid) {
        for (client, identity) in self.hub.clients_of(organization, account).await {
            for block in self.registry.blocks_of(client).await {
                let kept = block.organization != organization
                    || self
                        .store
                        .require_access(identity, vec![block.block], Access::View)
                        .await
                        .is_ok();
                if kept {
                    continue;
                }
                self.hub.unwatch(block, client).await;
                self.hub.send_to(client, Push::Unwatched { block }).await;
                if let Some(state) = self.registry.leave(block, client).await {
                    let mut told = state.participants.clone();
                    told.push(client);
                    self.hub
                        .send_all(
                            &told,
                            0,
                            &Push::SessionChanged {
                                block,
                                state: state.clone(),
                            },
                        )
                        .await;
                }
            }
        }
    }

    async fn contacts_changed(&self, accounts: &[Uuid]) {
        for account in accounts {
            self.hub
                .send_account(*account, self.client, None, &Push::ContactsChanged)
                .await;
        }
    }

    async fn reannounce(
        &self,
        identity: Identity,
        blocks: Vec<Uuid>,
        subtree: bool,
        before: BTreeSet<Uuid>,
    ) {
        self.reannounce_in(identity.organization, blocks, subtree, before)
            .await;
    }

    async fn reannounce_in(
        &self,
        organization: Uuid,
        blocks: Vec<Uuid>,
        subtree: bool,
        before: BTreeSet<Uuid>,
    ) {
        let Ok(mut accounts) = self.store.audience(organization, blocks.clone()).await else {
            return;
        };
        accounts.extend(before);
        let members = self.hub.audience(organization, &accounts).await;
        let identities = members.iter().map(|(_, member)| *member).collect();
        let Ok(views) = self
            .store
            .graph_views(organization, identities, blocks, subtree)
            .await
        else {
            return;
        };
        for ((client, _), messages) in members.into_iter().zip(views) {
            for (message, entry) in messages {
                self.hub.tell(client, message, entry).await;
            }
        }
    }

    async fn list(&self, listing: Listing) -> Result<Vec<BlockSummary>, ServerError> {
        let identity = self.identity()?;
        let new = self.hub.list(self.client, listing).await?;
        let blocks = async {
            Ok::<_, ServerError>(match listing {
                Listing::Shared => self.store.list_shared(identity).await?,
                Listing::Children(parent) => self.store.list_children(identity, parent).await?,
                Listing::Deleted => self.store.list_deleted(identity).await?,
                Listing::Backrefs(block) => self.store.list_backrefs(identity, block).await?,
                Listing::References(block) => self.store.list_references(identity, block).await?,
                Listing::Block(block) => vec![self.store.read_block(identity, block).await?],
                Listing::OpenSettings(block) => {
                    self.store.list_open_settings(identity, block).await?
                }
            })
        }
        .await;
        let blocks = match blocks {
            Ok(blocks) => blocks,
            Err(error) => {
                self.hub.refused(self.client, listing, new).await;
                return Err(error);
            }
        };
        self.hub.listed(self.client, listing, &blocks).await;
        Ok(blocks)
    }

    async fn announce(&self, block: Place, state: &SessionState) {
        self.hub
            .send_all(
                &state.participants,
                self.client,
                &Push::SessionChanged {
                    block,
                    state: state.clone(),
                },
            )
            .await;
    }
}
