use std::sync::Arc;
use std::time::Duration;

use be_block_client::{Credentials, Event, Peer, PeerConfig};
use be_block_fault_relay::Relay;
use be_block_keys::{InviteCode, RecoveryPhrase, noise::Channel};
use be_block_protocol::SessionHost;
use be_block_store::MemoryStore;
use uuid::Uuid;

use crate::Server;

/// How long a [`Player`] waits for the server or a host before it fails the test.
const PATIENCE: Duration = Duration::from_secs(60);

/// A player that is no device: an account of a test [`Server`] reaching it as a raw
/// be-block-client peer, with no be-host, as a game's own client would. It opens a Join link
/// and speaks to whoever hosts the block on its host channel (guides/blocks/hosted.md,
/// "Hosting"), so a test can watch members carry on across hosts.
pub struct Player {
    name: String,
    // Dropped first: the peer's tasks run on the runtime.
    peer: Arc<Peer<MemoryStore>>,
    events: tokio::sync::broadcast::Receiver<Event>,
    organization: Option<Uuid>,
    url: String,
    token: String,
    phrase: RecoveryPhrase,
    _relay: Arc<Relay>,
    runtime: tokio::runtime::Runtime,
}

impl Server {
    /// Makes an account named `name` and signs a [`Player`] of it in.
    pub fn player(&self, name: &str) -> Player {
        let account = self.account(name);
        let token = self.token(account.id);
        let relay = self.relay();
        let url = be_block_client::socket_url(&format!("http://{}", relay.address()));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("test-player")
            .enable_all()
            .build()
            .expect("the player's runtime starts");
        let phrase = RecoveryPhrase::generate();
        let peer = runtime.block_on(async {
            let (connection, _) = be_block_client::Connection::connect(
                &url,
                &be_block_client::Clock::real(),
                &Credentials::Token(token.clone()),
            )
            .await
            .unwrap_or_else(|error| panic!("{name} could not connect: {error:?}"));
            let personal = be_block_protocol::read::<Vec<be_block_protocol::Organization>, _>(
                &connection,
                None,
                be_block_protocol::Query::Organizations,
            )
            .await
            .ok()
            .and_then(Result::ok)
            .and_then(|organizations| {
                organizations.into_iter().find(|organization| {
                    organization.kind == be_block_protocol::OrganizationKind::Personal
                })
            })
            .unwrap_or_else(|| panic!("{name} has no personal organization"))
            .id;
            connect(name, &url, &token, &phrase, personal).await
        });
        let events = peer.connection().subscribe();
        Player {
            name: name.to_owned(),
            peer,
            events,
            organization: None,
            url,
            token,
            phrase,
            _relay: relay,
            runtime,
        }
    }
}

impl Player {
    /// Opens the share link `link` (as a device's share dialog shows it), answering the block
    /// it shares, and opens the organization it is in, as a guest of it if need be.
    pub fn open_share_link(&mut self, link: &str) -> Uuid {
        let code = link
            .split_once("#share/")
            .map_or(link, |(_, code)| code)
            .trim();
        let code = InviteCode::parse(code)
            .unwrap_or_else(|error| panic!("{link} is no share link: {error:?}"));
        let peer = Arc::clone(&self.peer);
        let (link, _) = self
            .runtime
            .block_on(be_block_client::share_links::open_share_link(
                &peer.connection(),
                peer.keys().identity(),
                peer.account(),
                &code,
                None,
            ))
            .unwrap_or_else(|error| panic!("{} could not open the link: {error:?}", self.name));
        self.organization = Some(link.organization);
        self.peer = self.runtime.block_on(connect(
            &self.name,
            &self.url,
            &self.token,
            &self.phrase,
            link.organization,
        ));
        self.events = self.peer.connection().subscribe();
        link.block
    }

    /// Joins `block`'s live session as a Join member, answering who hosts it as of the join.
    pub fn join(&mut self, block: Uuid) -> Option<SessionHost> {
        let organization = self
            .organization
            .unwrap_or_else(|| panic!("{} opened no link", self.name));
        let connection = self.peer.connection();
        let joined = self
            .runtime
            .block_on(async {
                tokio::time::timeout(
                    PATIENCE,
                    connection.join(
                        organization,
                        block,
                        be_block_store::Hash::from_bytes([0; 32]),
                    ),
                )
                .await
            })
            .unwrap_or_else(|_| panic!("{} waited too long to join", self.name))
            .unwrap_or_else(|error| panic!("{} could not join: {error:?}", self.name));
        joined.host
    }

    /// Waits until the server says a host other than `than` hosts `block`, answering it.
    pub fn host_other_than(&mut self, block: Uuid, than: Option<SessionHost>) -> SessionHost {
        let than = than.map(|host| host.client);
        loop {
            if let Event::Host {
                block: of,
                host: Some(host),
                ..
            } = self.next(&format!("a host other than {than:?}"))
                && of == block
                && Some(host.client) != than
            {
                return host;
            }
        }
    }

    /// Handshakes with `host` on `block`'s host channel and says `said` on the channel it
    /// opens, which has the host admit this player (guides/blocks/hosted.md, "Hosting").
    pub fn handshake(&mut self, block: Uuid, host: SessionHost, said: &[u8]) -> Channel {
        let peer = Arc::clone(&self.peer);
        let (joining, first) = self
            .runtime
            .block_on(peer.join_host(block, host.account))
            .unwrap_or_else(|error| panic!("{} could not check the host: {error:?}", self.name));
        self.runtime
            .block_on(peer.connection().send_host(block, None, first))
            .unwrap_or_else(|error| panic!("{} could not reach the host: {error:?}", self.name));
        let answer = loop {
            if let Event::Hosted {
                block: of,
                from,
                payload,
                ..
            } = self.next("the host's answer")
                && of == block
                && from == host.client
            {
                break payload;
            }
        };
        let mut channel = joining
            .finish(&answer)
            .unwrap_or_else(|error| panic!("{}'s handshake failed: {error:?}", self.name));
        self.runtime
            .block_on(peer.connection().send_host(block, None, channel.seal(said)))
            .unwrap_or_else(|error| panic!("{} could not speak: {error:?}", self.name));
        channel
    }

    fn next(&mut self, what: &str) -> Event {
        let events = &mut self.events;
        match self
            .runtime
            .block_on(async { tokio::time::timeout(PATIENCE, events.recv()).await })
        {
            Ok(Ok(event)) => event,
            // Events dropped unseen may have held what it waits for: no test should pass or
            // fail on a guess about them.
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped))) => {
                panic!("{} missed {skipped} events waiting for {what}", self.name)
            }
            Ok(Err(error)) => panic!("{}'s events ended waiting for {what}: {error}", self.name),
            Err(_) => panic!("{} waited too long for {what}", self.name),
        }
    }
}

/// A peer of the account `token` signs in, in `organization`.
async fn connect(
    name: &str,
    url: &str,
    token: &str,
    phrase: &RecoveryPhrase,
    organization: Uuid,
) -> Arc<Peer<MemoryStore>> {
    let peer = Peer::connect(
        PeerConfig::new(url, phrase.secret(), Credentials::Token(token.to_owned()))
            .organization(Some(organization)),
        MemoryStore::new(),
    )
    .await
    .unwrap_or_else(|error| panic!("{name} could not open the organization: {error:?}"));
    Arc::new(peer)
}
