#![cfg(not(target_arch = "wasm32"))]
//! The stack's harness (`be_host_stack::testing::Harness`) for tests outside be-host-stack: on
//! a server of the test's own (`be_host_server::testing::Server`), with every block type the
//! host links served in process.

use std::path::PathBuf;

use be_host_server::testing::Server;
use uuid::Uuid;

pub struct Harness {
    // Dropped first: the stack stops before its server goes.
    harness: be_host_stack::testing::Harness,
    server: Server,
}

impl std::ops::Deref for Harness {
    type Target = be_host_stack::testing::Harness;

    fn deref(&self) -> &Self::Target {
        &self.harness
    }
}

impl std::ops::DerefMut for Harness {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.harness
    }
}

impl Harness {
    pub fn start() -> Self {
        be_host_stack::testing::serve_in_process(be_host_stack::testing::InProcess {
            sources: be_type_bundle::sources,
            knows: be_type_bundle::knows,
        });
        let server = Server::start();
        Self {
            // A failed connect's wait moves the clock on to the next reconnect, so no test
            // strands on a clock nobody moves (guides/blocks/host.md).
            harness: be_host_stack::testing::Harness::start(Box::new(Served(server.clone())))
                .on_clock(be_block_client::Clock::driven()),
            server,
        }
    }

    /// The server the stack signs in against.
    pub fn server(&self) -> &Server {
        &self.server
    }
}

/// The test server, as the stack's harness asks it.
struct Served(Server);

impl be_host_stack::testing::TestServer for Served {
    fn url(&self) -> String {
        self.0.url()
    }

    fn directory(&self) -> PathBuf {
        self.0.directory().to_owned()
    }

    fn account(&self, name: &str) -> (Uuid, String) {
        let account = self.0.account(name);
        (account.id, account.email)
    }

    fn token(&self, account: Uuid) -> String {
        self.0.token(account)
    }

    fn stop(&self) {
        self.0.stop();
    }

    fn start_again(&self) {
        self.0.start_again();
    }
}
