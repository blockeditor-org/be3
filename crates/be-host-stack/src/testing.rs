//! What tests of the stack and of what runs on it sign in with: the harness
//! (`Harness`), one account's stack against a test's block server, and the block types a
//! test serves in process (`serve_in_process`).
//!
//! The harness asks the server for no more than `TestServer` says, so the server itself
//! (`be_host_server::testing::Server`) lives beneath this crate and nothing a host ships links
//! be-server (decisions/no_crate_splits_inside_dependency_chains.md).

#[cfg(feature = "testing")]
use std::sync::OnceLock;
#[cfg(not(target_arch = "wasm32"))]
use std::{path::PathBuf, sync::MutexGuard};

use uuid::Uuid;

#[cfg(feature = "testing")]
use crate::types::Source;

#[cfg(feature = "testing")]
pub struct InProcess {
    pub sources: fn() -> Vec<Source>,
    pub knows: fn(Uuid) -> bool,
}

#[cfg(feature = "testing")]
static IN_PROCESS: OnceLock<InProcess> = OnceLock::new();

#[cfg(feature = "testing")]
pub fn serve_in_process(types: InProcess) {
    let _ = IN_PROCESS.set(types);
}

#[cfg(feature = "testing")]
pub(crate) fn in_process() -> Option<&'static InProcess> {
    IN_PROCESS.get()
}

/// What the harness asks of a test's block server.
#[cfg(not(target_arch = "wasm32"))]
pub trait TestServer {
    /// Where a device that reaches it directly signs in.
    fn url(&self) -> String;
    /// The server's data.
    fn directory(&self) -> PathBuf;
    /// Makes the account named `name`: its id and email.
    fn account(&self, name: &str) -> (Uuid, String);
    /// A new sign-in for one device of `account`.
    fn token(&self, account: Uuid) -> String;
    /// Closes its listener, as a device losing its connection finds it.
    fn stop(&self);
    /// Listens again, at the same address and on the same data, after `stop`.
    fn start_again(&self);
}

/// One account's stack signed in against a test's server, in the account's organization
/// "Counters": one at a time in a process, since the stack is one per process.
#[cfg(not(target_arch = "wasm32"))]
pub struct Harness {
    pub(crate) server: Box<dyn TestServer>,
    pub(crate) directory: PathBuf,
    /// Where the server listens, which `config` signs in at unless the stack reaches it
    /// through something else (`reached_at`).
    pub(crate) url: String,
    pub(crate) reached_at: Option<String>,
    pub(crate) account: Uuid,
    pub(crate) token: String,
    pub(crate) organization: Uuid,
    pub(crate) phrase: be_block_keys::RecoveryPhrase,
    pub(crate) clock: be_block_client::Clock,
    _one_stack: MutexGuard<'static, ()>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Harness {
    /// Makes the account "Counter" on `server` and its organization "Counters"; `connect`
    /// starts the stack in it.
    pub fn start(server: Box<dyn TestServer>) -> Self {
        static ONE_STACK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let one_stack = ONE_STACK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        be_lib_build_id::install("test");
        // The previous test's stack has stopped (`stop_and_wait`, unless that test failed), so
        // nothing counts into these from here on but this test's.
        #[cfg(test)]
        crate::version::forget_counts();
        let directory = std::env::temp_dir().join(format!("be3-test-stack-{}", Uuid::new_v4()));
        let (account, email) = server.account("Counter");
        let token = server.token(account);
        let url = server.url();
        let phrase = be_block_keys::RecoveryPhrase::generate();
        let organization = runtime()
            .block_on(crate::accounts::create_organization(
                url.clone(),
                token.clone(),
                crate::accounts::Me {
                    account,
                    identity: phrase.secret().to_bytes(),
                    display_name: "Counter".to_owned(),
                    email,
                },
                "Counters".to_owned(),
            ))
            .expect("the organization is created")
            .id;
        Self {
            server,
            directory,
            url,
            reached_at: None,
            account,
            token,
            organization,
            phrase,
            clock: be_block_client::Clock::real(),
            _one_stack: one_stack,
        }
    }

    /// Has the stack reach the server at `url`: through a relay, say.
    pub fn reached_at(mut self, url: String) -> Self {
        self.reached_at = Some(url);
        self
    }

    /// Runs the stack on `clock` (driven or manual) rather than the real one.
    pub fn on_clock(mut self, clock: be_block_client::Clock) -> Self {
        self.clock = clock;
        self
    }

    pub fn account(&self) -> Uuid {
        self.account
    }

    pub fn organization(&self) -> Uuid {
        self.organization
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn identity(&self) -> [u8; 32] {
        self.phrase.secret().to_bytes()
    }

    pub fn connect(&self) {
        crate::start(self.config(self.directory.join("objects")));
    }

    pub fn config(&self, data_dir: PathBuf) -> crate::Config {
        crate::Config {
            local: false,
            server_url: self.reached_at.clone().unwrap_or_else(|| self.url.clone()),
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

    /// A peer of this account in its organization, as another device of it signs in.
    pub fn outside_config(&self) -> be_block_client::PeerConfig {
        be_block_client::PeerConfig::new(
            crate::accounts::socket_url(&self.url),
            be_block_keys::RecoverySecret::from_bytes(self.identity()),
            be_block_client::Credentials::Token(self.token.clone()),
        )
        .organization(Some(self.organization))
    }

    /// Stops the server, as a device losing its connection finds it: the stack keeps what it
    /// does until `start_server` brings it back at the same address.
    pub fn stop_server(&mut self) {
        self.server.stop();
    }

    /// Starts the server again on the same data and address after `stop_server`.
    pub fn start_server(&mut self) {
        self.server.start_again();
    }

    /// A block this account made on the server from another device, in the organization, under
    /// a parent this device has not opened: nothing here lists it, so a create here naming its
    /// id is one the server refuses, though this device knows of no such block.
    pub fn block_out_of_sight(&self, content_type: Uuid) -> Uuid {
        let (parent, block) = (Uuid::now_v7(), Uuid::now_v7());
        runtime().block_on(async {
            let peer = be_block_client::Peer::connect(
                self.outside_config(),
                be_block_store::MemoryStore::new(),
            )
            .await
            .expect("another device signs in");
            for (id, under) in [
                (parent, be_block_graph::BlockParent::Root),
                (block, be_block_graph::BlockParent::Block(parent)),
            ] {
                peer.create_block(
                    id,
                    content_type,
                    under,
                    &be_block_content::BlockMetadata::default(),
                    false,
                )
                .await
                .expect("another device makes its block");
            }
        });
        block
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Harness {
    fn drop(&mut self) {
        be_block_commit::pin_now(None);
        // A clock a test held would freeze the module timers of every test after it.
        *crate::HELD_CLOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        if std::thread::panicking() {
            crate::abandon();
        } else {
            crate::stop_and_wait();
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts")
}
