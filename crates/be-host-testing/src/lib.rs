#![cfg(not(target_arch = "wasm32"))]
//! The stack's harness (`be_host_stack::testing::Harness`) for tests outside be-host-stack: on
//! a server of the test's own (`be_host_server::testing::Server`), with every block type the
//! host links served in process.

use be_host_server::testing::Server;

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
        let counter = server.account("Counter");
        let harness = be_host_stack::testing::Harness::start(be_host_stack::testing::SignIn {
            url: server.url(),
            account: counter.id,
            token: server.token(counter.id),
            email: counter.email,
        });
        Self {
            // A failed connect's wait moves the clock on to the next reconnect, so no test
            // strands on a clock nobody moves (guides/blocks/host.md).
            harness: harness.on_clock(be_block_client::Clock::driven()),
            server,
        }
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

    /// The server the stack signs in against.
    pub fn server(&self) -> &Server {
        &self.server
    }
}
