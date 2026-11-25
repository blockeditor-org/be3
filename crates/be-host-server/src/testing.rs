//! The block server a test signs its devices in against (`Server`): be-server on data of its
//! own, with accounts, a fault relay for each device that is that device's network, and a
//! listener that stops and starts again at its address. be-host-stack's test harness
//! (`be_host_stack::testing::Harness`) signs one account's stack in against it, and be-host's
//! host tests sign each host they start in as a device of one of its accounts.
//!
//! The server half of the fixture lives here, beneath be-host-stack, rather than behind
//! be-host-stack's `testing` feature, so nothing a host ships links be-server
//! (decisions/no_crate_splits_inside_dependency_chains.md, the embedded server cut out of
//! be-host-stack).

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError, Weak},
    time::Duration,
};

use be_block_fault_relay::Relay;
use uuid::Uuid;

use crate::EmbeddedServer;

/// How a test's server is set up.
#[derive(Clone)]
pub struct Options {
    /// Where it may push: tests that serve a push service on loopback allow local endpoints.
    pub push: be_server::push::PushConfig,
    /// The moment its clock starts at, in milliseconds since the epoch, when a test's devices
    /// run on a clock of their own: it stamps when it took each edit, which a device reads
    /// against when the edit was made.
    pub starts_at: Option<i64>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            push: be_server::push::PushConfig::DEFAULT,
            starts_at: None,
        }
    }
}

/// A test's block server. Clones are handles on one server, which stops and deletes its data
/// when the last goes.
#[derive(Clone)]
pub struct Server {
    inner: Arc<Inner>,
}

struct Inner {
    // Stopped before the store and the runtime go, and before the data does.
    running: Mutex<Option<EmbeddedServer>>,
    /// The test's own handle on the server's data, opened before the server so the two never
    /// set the database up at once; it makes accounts and signs devices in.
    store: be_server::ServerStore,
    address: SocketAddr,
    directory: PathBuf,
    push: be_server::push::PushConfig,
    relays: Mutex<Vec<Weak<Relay>>>,
    /// How long each device's relay holds each answer of the server's (`slow`).
    latency: Mutex<Duration>,
    runtime: tokio::runtime::Runtime,
}

impl Drop for Inner {
    fn drop(&mut self) {
        // The server stops before its files go, or it fails opening them as it winds down.
        *lock(&self.running) = None;
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// An account on a [`Server`].
#[derive(Clone)]
pub struct Account {
    pub server: Server,
    pub id: Uuid,
    pub email: String,
    pub name: String,
}

impl Server {
    pub fn start() -> Self {
        Self::with(Options::default())
    }

    pub fn with(options: Options) -> Self {
        if let Some(starts_at) = options.starts_at {
            be_block_commit::start_now_at(starts_at);
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("test-server")
            .enable_all()
            .build()
            .expect("the test server's runtime starts");
        let directory = std::env::temp_dir().join(format!("be3-test-server-{}", Uuid::new_v4()));
        let store = be_server::ServerStore::open(&directory)
            .unwrap_or_else(|error| panic!("the test server's data could not be set up: {error}"));
        let running = crate::start_embedded_server_at(
            "127.0.0.1:0",
            directory.clone(),
            options.push.clone(),
        )
        .expect("the test server starts");
        let address = running
            .url
            .trim_start_matches("http://")
            .parse()
            .expect("the test server listens on an address");
        Self {
            inner: Arc::new(Inner {
                running: Mutex::new(Some(running)),
                store,
                address,
                directory,
                push: options.push,
                relays: Mutex::new(Vec::new()),
                latency: Mutex::new(Duration::ZERO),
                runtime,
            }),
        }
    }

    /// Where a device that reaches it directly, past any relay, signs in.
    pub fn url(&self) -> String {
        format!("http://{}", self.inner.address)
    }

    pub fn address(&self) -> SocketAddr {
        self.inner.address
    }

    /// The server's data, which a test may read or change behind its back.
    pub fn directory(&self) -> &Path {
        &self.inner.directory
    }

    /// Makes an account named `name`, with its own My files in the server's personal
    /// organization. A test's devices of different accounts reach each other only as accounts
    /// on one server do.
    pub fn account(&self, name: &str) -> Account {
        let email = format!("{}@example.com", name.to_lowercase().replace(' ', "."));
        let (profile, _) = self
            .inner
            .runtime
            .block_on(self.inner.store.register(&email, name))
            .unwrap_or_else(|error| {
                panic!("the test server could not make the account {name}: {error}")
            });
        Account {
            server: self.clone(),
            id: profile.account,
            email: profile.email,
            name: profile.display_name,
        }
    }

    /// A new sign-in for one device of `account`.
    pub fn token(&self, account: Uuid) -> String {
        self.inner
            .runtime
            .block_on(self.inner.store.issue_session(account))
            .unwrap_or_else(|error| panic!("the test server could not sign a device in: {error}"))
    }

    /// Stops the server, as a device losing its connection finds it: its listener is closed
    /// before this returns, so `start_again` binds the address again at once (std's listeners
    /// set SO_REUSEADDR on Unix, so connections left in TIME_WAIT hold nothing). A relay going
    /// offline is not this: the server keeps running behind it.
    pub fn stop(&self) {
        *lock(&self.inner.running) = None;
    }

    /// Starts the server again on the same data and address after `stop`.
    pub fn start_again(&self) {
        let running = crate::start_embedded_server_at(
            &self.inner.address.to_string(),
            self.inner.directory.clone(),
            self.inner.push.clone(),
        )
        .expect("the test server starts again");
        *lock(&self.inner.running) = Some(running);
    }

    /// A network of its own for one device, carrying its connections to the server.
    pub fn relay(&self) -> Arc<Relay> {
        let relay = Arc::new(Relay::start(self.inner.address));
        // `stress --latency` (crates/be-tool-local) slows every answer too.
        let stressed = std::env::var("HOST_TEST_LATENCY_MS")
            .ok()
            .and_then(|milliseconds| milliseconds.parse().ok())
            .map_or(Duration::ZERO, Duration::from_millis);
        relay.delay_answers((*lock(&self.inner.latency)).max(stressed));
        lock(&self.inner.relays).push(Arc::downgrade(&relay));
        relay
    }

    /// Has the server answer the devices started from now on `by` late, as a distant or busy
    /// one does, each answer in order.
    pub fn slow(&self, by: Duration) {
        *lock(&self.inner.latency) = by;
    }

    /// Takes the server away from every device's relay, or brings it back.
    pub fn offline(&self, offline: bool) {
        for relay in self.relays() {
            relay.offline(offline);
        }
    }

    /// Has the server serve `of` its account's block, which holds its contacts' records, with
    /// keys that do not open, as a server tampering with them would: a device that saw the
    /// records keeps the ones it pinned, and says it cannot read the server's copy
    /// (guides/blocks/keys.md, "Contacts"). Every contact's record is spoiled alike.
    pub fn spoil_contacts(&self, of: &Account) {
        let of = of.id.to_string();
        be_block_sqlite_schema::open(self.inner.directory.join("metadata.sqlite"))
            .unwrap_or_else(|error| panic!("the test server's database did not open: {error}"))
            .execute(
                "UPDATE wrapped_keys SET sealed = x'07070707070707070707070707070707'
                 WHERE wrapped_by = 'account:' || ?1 AND key_id IN (
                    SELECT keys.id FROM keys JOIN blocks
                        ON blocks.organization_id = keys.organization_id
                        AND blocks.id = keys.block_id
                    WHERE blocks.author = ?1 AND blocks.owner_only = 1
                        AND blocks.parent_kind = 1)",
                [of.as_str()],
            )
            .unwrap_or_else(|error| panic!("the test server could not spoil the keys: {error}"));
    }

    /// Adds `joiner` to the organization named `name` that `of` belongs to, as an editor,
    /// straight into the server's data, as a server adding a member on its own would: no
    /// invitation signed them into the organization's log.
    pub fn add_member(&self, of: &Account, name: &str, joiner: &Account) {
        let added = be_block_sqlite_schema::open(self.inner.directory.join("metadata.sqlite"))
            .unwrap_or_else(|error| panic!("the test server's database did not open: {error}"))
            .execute(
                "INSERT INTO memberships (organization_id, account_id, role)
                 SELECT organizations.id, ?3, 'editor' FROM organizations
                 JOIN memberships ON memberships.organization_id = organizations.id
                 WHERE organizations.name = ?1 AND memberships.account_id = ?2",
                [
                    name,
                    of.id.to_string().as_str(),
                    joiner.id.to_string().as_str(),
                ],
            )
            .unwrap_or_else(|error| panic!("the test server could not add the member: {error}"));
        assert_eq!(
            added, 1,
            "the account belongs to one organization of that name"
        );
    }

    fn relays(&self) -> Vec<Arc<Relay>> {
        let mut relays = lock(&self.inner.relays);
        relays.retain(|relay| relay.strong_count() > 0);
        relays.iter().filter_map(Weak::upgrade).collect()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
