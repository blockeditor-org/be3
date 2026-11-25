//! What tests of the stack and of what runs on it sign in with: the harness
//! (`Harness`), one account's stack against a test's block server, and the block types a
//! test serves in process (`serve_in_process`).
//!
//! It is compiled into every native build, inert unless a test calls it
//! (decisions/test_hooks_are_ordinary_code.md). The harness is handed what it signs in with
//! (`SignIn`) rather than the server, so the server itself (`be_host_server::testing::Server`)
//! lives beneath this crate and nothing a host ships links be-server; whoever holds the server
//! stops and starts it.

use std::sync::OnceLock;
use std::{path::PathBuf, sync::MutexGuard};

use uuid::Uuid;

use crate::types::Source;

pub struct InProcess {
    pub sources: fn() -> Vec<Source>,
    pub knows: fn(Uuid) -> bool,
}

static IN_PROCESS: OnceLock<InProcess> = OnceLock::new();

pub fn serve_in_process(types: InProcess) {
    let _ = IN_PROCESS.set(types);
}

pub(crate) fn in_process() -> Option<&'static InProcess> {
    IN_PROCESS.get()
}

/// What the harness signs in with: an account a test's block server made, and where the
/// server listens.
pub struct SignIn {
    pub url: String,
    pub account: Uuid,
    pub email: String,
    pub token: String,
}

/// One account's stack signed in against a test's server, in the account's organization
/// "Counters": one at a time in a process, since the stack is one per process.
pub struct Harness {
    pub(crate) directory: PathBuf,
    /// Where the server listens, which `config` signs in at unless the stack reaches it
    /// through something else (`reached_at`).
    pub(crate) url: String,
    pub(crate) reached_at: Option<String>,
    pub(crate) account: Uuid,
    pub(crate) email: String,
    pub(crate) token: String,
    pub(crate) organization: Uuid,
    pub(crate) phrase: be_block_keys::RecoveryPhrase,
    pub(crate) clock: be_block_client::Clock,
    _one_stack: MutexGuard<'static, ()>,
}

impl Harness {
    /// Makes the organization "Counters" of the account `sign_in` names, the test server's
    /// "Counter"; `connect` starts the stack in it.
    pub fn start(sign_in: SignIn) -> Self {
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
        let SignIn {
            url,
            account,
            email,
            token,
        } = sign_in;
        let phrase = be_block_keys::RecoveryPhrase::generate();
        let organization = runtime()
            .block_on(crate::accounts::create_organization(
                url.clone(),
                token.clone(),
                crate::accounts::Me {
                    account,
                    identity: phrase.secret().to_bytes(),
                    display_name: "Counter".to_owned(),
                    email: email.clone(),
                },
                "Counters".to_owned(),
            ))
            .expect("the organization is created")
            .id;
        Self {
            directory,
            url,
            reached_at: None,
            account,
            email,
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
            email: self.email.clone(),
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

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts")
}
