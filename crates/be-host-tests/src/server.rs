use std::sync::{Arc, Mutex, PoisonError, Weak};

use be_block_fault_relay::Relay;
use uuid::Uuid;

/// A be-server a host test owns, which hosts join as devices of its accounts
/// ([`crate::HostTest::device`]), where `HostTest::start` has each host embed a private one.
///
/// Each device reaches it through a relay of its own (be-block-fault-relay), which is that
/// device's network: [`crate::HostTest::offline`] takes it away from one device,
/// [`Server::offline`] from all of them. The server runs in the test's process, on data in a
/// directory of its own that it deletes when the last handle to it goes.
#[derive(Clone)]
pub struct Server {
    inner: Arc<Inner>,
}

struct Inner {
    // Dropped before the runtime it runs on.
    running: be_server::Running,
    /// The test's own handle on the server's data, opened before the server so the two never
    /// set the database up at once; it makes accounts and signs devices in.
    store: be_server::ServerStore,
    relays: Mutex<Vec<Weak<Relay>>>,
    runtime: tokio::runtime::Runtime,
}

/// An account on a [`Server`], whose devices [`crate::HostTest::device`] starts.
#[derive(Clone)]
pub struct Account {
    pub(crate) server: Server,
    pub(crate) id: Uuid,
}

impl Server {
    pub fn start() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("test-server")
            .enable_all()
            .build()
            .expect("the test server's runtime starts");
        let directory = std::env::temp_dir().join(format!("be3-host-test-server-{}", Uuid::new_v4()));
        let config = be_server::ServerConfig {
            account_quota: None,
            ..be_server::ServerConfig::OPEN
        };
        let store = be_server::ServerStore::open(&directory)
            .unwrap_or_else(|error| panic!("the test server's data could not be set up: {error}"));
        let running = runtime.block_on(be_server::Running::at(directory, config));
        Self {
            inner: Arc::new(Inner {
                running,
                store,
                relays: Mutex::new(Vec::new()),
                runtime,
            }),
        }
    }

    /// Makes an account named `name`, with its own My files in the server's personal
    /// organization. A test's devices of different accounts reach each other only as
    /// accounts on one server do.
    pub fn account(&self, name: &str) -> Account {
        let email = format!("{}@example.com", name.to_lowercase().replace(' ', "."));
        let (profile, _) = self
            .inner
            .runtime
            .block_on(
                self.inner
                    .store
                    .register(&email, name, be_server::Running::PASSWORD),
            )
            .unwrap_or_else(|error| panic!("the test server could not make the account {name}: {error}"));
        Account {
            server: self.clone(),
            id: profile.account,
        }
    }

    /// Takes the server away from every device, or brings it back; each device's own
    /// [`crate::HostTest::online`] then tells it so.
    pub fn offline(&self, offline: bool) {
        for relay in self.relays() {
            relay.offline(offline);
        }
    }

    /// A network of its own for one device, carrying its connections to the server.
    pub(crate) fn relay(&self) -> Arc<Relay> {
        let relay = Arc::new(Relay::start(self.inner.running.address));
        self.inner
            .relays
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Arc::downgrade(&relay));
        relay
    }

    /// A new sign-in for one device of `account`.
    pub(crate) fn token(&self, account: Uuid) -> String {
        self.inner
            .runtime
            .block_on(self.inner.store.issue_session(account))
            .unwrap_or_else(|error| panic!("the test server could not sign a device in: {error}"))
    }

    fn relays(&self) -> Vec<Arc<Relay>> {
        let mut relays = self.inner.relays.lock().unwrap_or_else(PoisonError::into_inner);
        relays.retain(|relay| relay.strong_count() > 0);
        relays.iter().filter_map(Weak::upgrade).collect()
    }
}
