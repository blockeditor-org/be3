use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use be_ops::{Backlog, Incoming, OpError, OpId, Outlets, Reply, Started, net::Peer};
use be_protocol::{
    Authenticated, BlockChange, BlockEvent, BlockSummary, BrowserLogin, Collected, Contents,
    CreatedAgent, Error, ErrorCode, Listing, OrganizationOpened, PairingMessage, RecoveryKey,
    SessionEvent, SessionState, embed,
};
use be_session::Claim;
use be_store::Hash;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    CONTENTS_BYTES, Identity, PublishOutcome, ServerConfig, ServerError, ServerStore, Services,
    SessionRegistry, WatchHub, browser_auth, contacts, store, watch::Push,
};

const STREAM_BUFFER: usize = 4096;
const PUSHES: usize = 4096;

pub(crate) enum Late {
    SignedIn {
        reply: Reply<Result<Authenticated, Error>>,
        profile: store::Profile,
        token: String,
    },
}

fn wire(error: ServerError) -> Error {
    Error {
        code: error.code(),
        message: error.to_string(),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Route {
    Watch(Uuid),
    Changes,
    Paired,
    Contacts,
    KeyWork,
    Events { session: OpId, block: Uuid },
}

#[derive(Default)]
struct Streams {
    outlets: Outlets<Route>,
    sessions: HashMap<OpId, Uuid>,
}

impl Streams {
    fn send<T: be_ops::Serialize>(
        &mut self,
        connection: &mut be_ops::Connection,
        item: T,
        to: impl FnMut(&Route) -> bool,
    ) -> Result<(), ServerError> {
        self.outlets
            .send(connection, &item, to)
            .map_err(|_| ServerError::Socket("the client is not reading what it is sent".into()))
    }

    fn open(&mut self, stream: be_ops::Producer<impl Sized>, route: Route) {
        self.outlets
            .insert(stream, route, Backlog::Queue(STREAM_BUFFER));
    }

    fn events(&self, session: OpId) -> Option<OpId> {
        self.outlets.routes().find_map(|(op, route)| {
            matches!(route, Route::Events { session: of, .. } if *of == session).then_some(op)
        })
    }

    fn in_session(&self, block: Uuid) -> bool {
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
    pub(crate) header_token: Option<String>,
    pub(crate) identity: Option<Identity>,
    pub(crate) config: ServerConfig,
}

pub(crate) struct Client {
    pub(crate) connection: Connection,
    pub(crate) peer: Peer,
    pub(crate) pushes: mpsc::Receiver<Push>,
    late: mpsc::UnboundedSender<Late>,
    pub(crate) lates: mpsc::UnboundedReceiver<Late>,
    streams: Streams,
}

impl Client {
    pub(crate) async fn open(
        services: &Services,
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
                header_token: None,
                identity: None,
                config: services.config.clone(),
            },
            peer: Peer::new(be_ops::Side::Host, sink),
            pushes,
            late,
            lates,
            streams: Streams::default(),
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
            Late::SignedIn {
                reply,
                profile,
                token,
            } => {
                let authenticated = self.connection.authenticated(profile, token).await;
                self.reply(reply, Ok(authenticated));
            }
        }
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

    fn reply<T, E: be_ops::wire::Encode<Result<T, Error>>>(
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
            let sessions = |block: Uuid| move |route: &Route| matches!(route, Route::Events { block: of, .. } if *of == block);
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
                        SessionEvent::Relayed { from, payload },
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
            if let Some(block) = streams.sessions.remove(&op) {
                if let Some(events) = streams.events(op) {
                    streams.outlets.end(&mut lock, events, canceled);
                }
                embed::sessions::Session(op).end(&mut lock);
                (!streams.in_session(block)).then_some((block, true))
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
        if let Some(token) = &connection.token {
            connection.store.resolve_token(token).await?;
        }
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
        let prelude = self.prelude().await;
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

    async fn account(&mut self, call: embed::account::Call, prelude: Result<(), ServerError>) {
        use embed::account::Call;
        match call {
            Call::BeginBrowserLogin { reply } => {
                let begun = prelude.and_then(|()| {
                    let (code, secret) =
                        self.connection.browser_auth.begin(self.connection.client)?;
                    Ok(BrowserLogin { code, secret })
                });
                self.reply(reply, begun);
            }
            Call::FinishBrowserLogin {
                reply,
                code,
                secret,
            } => {
                let finished = prelude.and_then(|()| {
                    if !self.connection.config.allow_login {
                        return Err(ServerError::Refused(
                            ErrorCode::LoginDisabled,
                            "This server is not accepting sign-ins.".into(),
                        ));
                    }
                    self.connection
                        .browser_auth
                        .finish(self.connection.client, &code, &secret)
                });
                let decided = match finished {
                    Err(error) => return self.reply(reply, Err(error)),
                    Ok(browser_auth::Finished::Approved { account, issuer }) => {
                        let signed_in = async {
                            let connection = &self.connection;
                            let token = connection
                                .store
                                .claim_browser_session(account, &issuer)
                                .await?;
                            let profile = connection.store.resolve_token(&token).await?;
                            Ok::<_, ServerError>(
                                self.connection.authenticated(profile, token).await,
                            )
                        };
                        let signed_in = signed_in.await;
                        return self.reply(reply, signed_in);
                    }
                    Ok(browser_auth::Finished::Waiting(decided)) => decided,
                };
                self.wait_for_browser(reply, code, secret, decided);
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
                    connection.identity = None;
                    connection.hub.leave_account(connection.client).await;
                    Ok(())
                }
                .await;
                self.reply(reply, logged_out);
            }
            Call::Pair { reply, to, payload } => {
                let connection = &self.connection;
                let paired = async {
                    prelude?;
                    let account = connection.account()?;
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
                self.streams.open(stream, Route::Paired);
            }
        }
    }

    fn wait_for_browser(
        &self,
        reply: Reply<Result<Authenticated, Error>>,
        code: String,
        secret: String,
        mut decided: tokio::sync::watch::Receiver<()>,
    ) {
        let peer = self.peer.clone();
        let late = self.late.clone();
        let sessions = Arc::clone(&self.connection.browser_auth);
        let store = Arc::clone(&self.connection.store);
        let client = self.connection.client;
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    () = peer.canceled(reply.op()) => {
                        reply.fail(&mut peer.lock(), OpError::Canceled);
                        return;
                    }
                    _ = decided.changed() => {}
                }
                let approved = match sessions.finish(client, &code, &secret) {
                    Err(error) => {
                        reply.send(&mut peer.lock(), &Err(wire(error)));
                        return;
                    }
                    Ok(browser_auth::Finished::Waiting(next)) => {
                        decided = next;
                        continue;
                    }
                    Ok(browser_auth::Finished::Approved { account, issuer }) => {
                        async {
                            let token = store.claim_browser_session(account, &issuer).await?;
                            let profile = store.resolve_token(&token).await?;
                            Ok::<_, ServerError>((profile, token))
                        }
                        .await
                    }
                };
                match approved {
                    Ok((profile, token)) => {
                        let _ = late.send(Late::SignedIn {
                            reply,
                            profile,
                            token,
                        });
                    }
                    Err(error) => reply.send(&mut peer.lock(), &Err(wire(error))),
                }
                return;
            }
        });
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
                self.streams
                    .outlets
                    .insert(stream, Route::Contacts, Backlog::Latest);
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
                    connection
                        .store
                        .may_store(
                            identity,
                            objects.iter().map(|bytes| Hash::of(bytes)).collect(),
                        )
                        .await?;
                    connection.store.put_objects(&objects)
                }
                .await;
                self.reply(reply, stored);
            }
            Call::GetObject { reply, hash } => {
                let found = async {
                    prelude?;
                    let identity = connection.identity()?;
                    match connection.store.may_read_object(identity, hash).await? {
                        true => connection.store.get_object(hash),
                        false => Ok(None),
                    }
                }
                .await;
                self.reply(reply, found);
            }
            Call::MissingObjects { reply, hashes } => {
                let missing = async {
                    prelude?;
                    connection.identity()?;
                    connection.store.missing_objects(&hashes)
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
                    connection.store.put_objects(&objects)?;
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
                            connection
                                .hub
                                .broadcast(
                                    block,
                                    connection.client,
                                    Push::HeadChanged {
                                        block,
                                        head,
                                        author: identity.account,
                                    },
                                )
                                .await;
                            be_protocol::PublishOutcome::Published(head)
                        }
                        PublishOutcome::Rejected(head) => {
                            be_protocol::PublishOutcome::Rejected(head)
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
                    let mut objects = Vec::new();
                    let mut room = CONTENTS_BYTES;
                    for hash in hashes {
                        let Some(bytes) = connection.store.get_object(hash)? else {
                            continue;
                        };
                        if bytes.len() > room {
                            break;
                        }
                        room -= bytes.len();
                        objects.push(bytes);
                    }
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
                self.streams.open(stream, Route::Changes);
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
                    connection
                        .hub
                        .broadcast(block, connection.client, Push::BlockDeleted { block })
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
                let allowed = async {
                    prelude?;
                    let identity = connection.identity()?;
                    connection.store.read_block(identity, block).await?;
                    Ok::<_, ServerError>(())
                }
                .await;
                if allowed.is_err() {
                    stream.fail(&mut self.peer.lock(), OpError::Denied);
                    return;
                }
                connection.hub.watch(block, connection.client).await;
                self.streams.open(stream, Route::Watch(block));
            }
        }
    }

    async fn sessions(&mut self, call: embed::sessions::Call, prelude: Result<(), ServerError>) {
        use embed::sessions::Call;
        match call {
            Call::SessionNew { resource, block } => {
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
                    let identity = connection.identity()?;
                    connection.store.read_block(identity, block).await?;
                    let state = connection.registry.join(block, connection.client).await;
                    connection.announce(block, &state).await;
                    Ok::<_, ServerError>(state)
                }
                .await;
                let op = stream.op();
                let mut lock = self.peer.lock();
                let streams = &mut self.streams;
                if let Some(before) = streams.events(session) {
                    streams
                        .outlets
                        .end(&mut lock, before, Some(OpError::Canceled));
                }
                streams.open(stream, Route::Events { session, block });
                let only = |route: &Route| *route == Route::Events { session, block };
                let event = match joined {
                    Ok(state) => SessionEvent::Joined {
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
                    connection.identity()?;
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
                    connection.identity()?;
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
                    connection.identity()?;
                    let block = block?;
                    let participants = connection.registry.participants(block).await;
                    if !participants.contains(&connection.client) {
                        return Err(ServerError::Refused(
                            ErrorCode::PermissionDenied,
                            "join the session before speaking in it".into(),
                        ));
                    }
                    let message = Push::Relayed {
                        block,
                        from: connection.client,
                        payload,
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

    fn joined(&self, session: OpId) -> Result<Uuid, ServerError> {
        self.streams
            .sessions
            .get(&session)
            .copied()
            .filter(|_| self.streams.events(session).is_some())
            .ok_or_else(|| {
                ServerError::Refused(
                    ErrorCode::PermissionDenied,
                    "join the session before speaking in it".into(),
                )
            })
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
                self.streams
                    .outlets
                    .insert(stream, Route::KeyWork, Backlog::Latest);
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
    async fn authenticated(&mut self, profile: store::Profile, token: String) -> Authenticated {
        self.hub.join_account(self.client, profile.account).await;
        self.account = Some(profile.account);
        self.token = Some(token.clone());
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
                if self.store.read_block(identity, block).await.is_ok() {
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
        self.hub.list(self.client, listing).await;
        let blocks = match listing {
            Listing::Shared => self.store.list_shared(identity).await?,
            Listing::Children(parent) => self.store.list_children(identity, parent).await?,
            Listing::Deleted => self.store.list_deleted(identity).await?,
            Listing::Backrefs(block) => self.store.list_backrefs(identity, block).await?,
            Listing::References(block) => self.store.list_references(identity, block).await?,
            Listing::Block(block) => vec![self.store.read_block(identity, block).await?],
            Listing::OpenSettings(block) => self.store.list_open_settings(identity, block).await?,
        };
        self.hub.listed(self.client, listing, &blocks).await;
        Ok(blocks)
    }

    async fn announce(&self, block: Uuid, state: &SessionState) {
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
