#![cfg(not(target_arch = "wasm32"))]

use std::{path::PathBuf, sync::MutexGuard};

use be_host_server::EmbeddedServer;
use be_host_stack::Config;
use uuid::Uuid;

pub struct Harness {
    directory: PathBuf,
    server: Option<EmbeddedServer>,
    url: String,
    account: Uuid,
    token: String,
    organization: Uuid,
    phrase: be_block_keys::RecoveryPhrase,
    _one_stack: MutexGuard<'static, ()>,
}

impl Harness {
    pub fn start() -> Self {
        static ONE_STACK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let one_stack = ONE_STACK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        be_host_stack::testing::serve_in_process(be_host_stack::testing::InProcess {
            sources: be_type_bundle::sources,
            knows: be_type_bundle::knows,
        });
        be_lib_build_id::install("test");
        let directory = std::env::temp_dir().join(format!("be-host-be-test-{}", Uuid::new_v4()));
        let runtime = runtime();
        let (profile, token) = runtime.block_on(async {
            be_server::ServerStore::open(directory.join("server"))
                .unwrap()
                .register("counter@example.com", "Counter")
                .await
                .unwrap()
        });
        let server = be_host_server::start_embedded_server_at(
            "127.0.0.1:0",
            directory.join("server"),
            be_server::push::PushConfig::DEFAULT,
        )
        .expect("the embedded block server starts");
        let url = server.url.clone();
        let phrase = be_block_keys::RecoveryPhrase::generate();
        let organization = runtime
            .block_on(be_host_stack::accounts::create_organization(
                url.clone(),
                token.clone(),
                be_host_stack::accounts::Me {
                    account: profile.account,
                    identity: phrase.secret().to_bytes(),
                    display_name: "Counter".to_owned(),
                    email: "counter@example.com".to_owned(),
                },
                "Counters".to_owned(),
            ))
            .expect("the organization is created");
        Self {
            directory,
            server: Some(server),
            url,
            account: profile.account,
            token,
            organization: organization.id,
            phrase,
            _one_stack: one_stack,
        }
    }

    /// Stops the server, as a device losing its connection finds it: the stack keeps what it
    /// does until `start_server` brings it back at the same address.
    pub fn stop_server(&mut self) {
        // Dropped, the server is told to stop and its thread joined, so its listener is closed
        // before this returns; std's listeners set SO_REUSEADDR on Unix, so connections it left
        // in TIME_WAIT hold nothing, and `start_server` retries a bind the kernel still refuses.
        self.server = None;
    }

    /// Starts the server again on the same data and address after `stop_server`.
    pub fn start_server(&mut self) {
        let address = self
            .url
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_owned();
        // The address can still read as taken just after the stop: the bind is tried again
        // until it is free, as it is once the kernel lets go of the old listener.
        let mut tries = 0;
        let server = loop {
            match be_host_server::start_embedded_server_at(
                &address,
                self.directory.join("server"),
                be_server::push::PushConfig::DEFAULT,
            ) {
                Ok(server) => break server,
                Err(error)
                    if tries < 100_000
                        && error.downcast_ref::<std::io::Error>().is_some_and(|error| {
                            error.kind() == std::io::ErrorKind::AddrInUse
                        }) =>
                {
                    tries += 1;
                    std::thread::yield_now();
                }
                Err(error) => panic!("the embedded block server starts again: {error}"),
            }
        };
        self.server = Some(server);
    }

    /// A block this account made on the server from another device, in the organization, under
    /// a parent this device has not opened: nothing here lists it, so a create here naming its
    /// id is one the server refuses, though this device knows of no such block.
    pub fn block_out_of_sight(&self, content_type: Uuid) -> Uuid {
        let (parent, block) = (Uuid::now_v7(), Uuid::now_v7());
        runtime().block_on(async {
            let peer = be_block_client::Peer::connect(
                be_block_client::PeerConfig::new(
                    be_block_client::socket_url(&self.url),
                    self.phrase.secret(),
                    be_block_client::Credentials::Token(self.token.clone()),
                )
                .organization(Some(self.organization)),
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

    pub fn connect(&self) {
        be_host_stack::start(Config {
            local: false,
            server_url: self.url.clone(),
            token: self.token.clone(),
            account: self.account,
            organization: self.organization,
            identity: self.phrase.secret().to_bytes(),
            display_name: "Counter".to_owned(),
            email: "counter@example.com".to_owned(),
            data_dir: self.directory.join("objects"),
            clock: be_block_client::Clock::driven(),
        });
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        if std::thread::panicking() {
            be_host_stack::abandon();
        } else {
            be_host_stack::stop_and_wait();
        }
        // The server stops before its files go, or it fails opening them as it winds down.
        self.server = None;
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a test runtime starts")
}
