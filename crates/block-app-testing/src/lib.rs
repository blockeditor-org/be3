#![cfg(not(target_arch = "wasm32"))]

use std::{path::PathBuf, sync::MutexGuard};

use block_app_stack::{Config, embedded_server::EmbeddedServer};
use uuid::Uuid;

pub struct Harness {
    directory: PathBuf,
    _server: EmbeddedServer,
    url: String,
    account: Uuid,
    token: String,
    organization: Uuid,
    phrase: be_keys::RecoveryPhrase,
    _one_stack: MutexGuard<'static, ()>,
}

impl Harness {
    pub fn start() -> Self {
        static ONE_STACK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let one_stack = ONE_STACK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        block_app_stack::testing::serve_in_process(block_app_stack::testing::InProcess {
            sources: first_party_types::sources,
            knows: first_party_types::knows,
        });
        build_id::install("test");
        let directory = std::env::temp_dir().join(format!("block-app-be-test-{}", Uuid::new_v4()));
        let runtime = runtime();
        let (profile, token) = runtime.block_on(async {
            be_server::ServerStore::open(directory.join("server"))
                .unwrap()
                .register("counter@example.com", "Counter", "correct horse battery")
                .await
                .unwrap()
        });
        let server = block_app_stack::embedded_server::start_embedded_server_at(
            "127.0.0.1:0",
            directory.join("server"),
            be_server::ServerConfig::OPEN.lease,
        )
        .expect("the embedded block server starts");
        let url = server.url.clone();
        let organization = runtime
            .block_on(block_app_stack::accounts::create_organization(
                url.clone(),
                token.clone(),
                "Counters".to_owned(),
            ))
            .expect("the organization is created");
        Self {
            directory,
            _server: server,
            url,
            account: profile.account,
            token,
            organization: organization.id,
            phrase: be_keys::RecoveryPhrase::generate(),
            _one_stack: one_stack,
        }
    }

    pub fn connect(&self) {
        block_app_stack::start(Config {
            server_url: self.url.clone(),
            token: self.token.clone(),
            account: self.account,
            organization: self.organization,
            identity: self.phrase.secret().to_bytes(),
            display_name: "Counter".to_owned(),
            email: "counter@example.com".to_owned(),
            data_dir: self.directory.join("objects"),
            clock: be_client::Clock::real(),
        });
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        if std::thread::panicking() {
            block_app_stack::abandon();
        } else {
            block_app_stack::stop_and_wait();
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
