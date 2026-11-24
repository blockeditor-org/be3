use std::time::Duration;

use be_wasm_root_api::{CoreSecret, SecretCommand};
use beui::native::Context;
use beui_host::native::FreetypeFonts;

use super::*;
use crate::{HostStateStore, ServerLocation};

mod an_agent_asked_for_while_another_is_made_waits_and_each_gets_its_own_credentials;
mod an_agent_the_root_stopped_waiting_for_is_removed_once_made;

/// The secrets the root was answered, by its request's id, each an agent's name or none.
type Answers = Vec<(u64, Option<String>)>;

/// A core signed in to the running stack's server (`be_host_testing::Harness`) with an
/// organization of a company open, which it administers, as the root's host runs one: it makes
/// and removes agents through the stack, which knows the organization.
fn signed_in() -> HostCore {
    let state = HostStateStore::open(":memory:").expect("the host state opens");
    let mut core = HostCore::with_state(state).expect("the app starts");
    core.signed_in = true;
    core.account.server = ServerLocation::Remote("http://the-harness".to_owned());
    core.organization = Some(be_block_protocol::Organization {
        id: Uuid::new_v4(),
        name: "Counters".to_owned(),
        kind: be_block_protocol::OrganizationKind::Company,
        role: be_block_protocol::OrganizationRole::Administrator,
        escrows: Vec::new(),
    });
    core
}

/// The block stack as the root's host keeps it, with a core to serve and the context its
/// commands are done in.
struct Served {
    stack: BlockStack,
    core: HostCore,
    context: Context,
    answers: Answers,
    /// Whether the root still waits for what it asked when it is answered.
    waits: bool,
}

impl Served {
    fn new(waits: bool) -> Self {
        Self {
            stack: BlockStack::default(),
            core: signed_in(),
            context: Context::new(FreetypeFonts::default()),
            answers: Vec::new(),
            waits,
        }
    }

    /// One frame of the root's host: what the organization's requests answered is delivered,
    /// then the block stack is served the root's `asked`.
    fn step(&mut self, asked: Vec<(u64, SecretCommand)>) {
        crate::desk().deliver(&mut self.core);
        let (answers, waits) = (&mut self.answers, self.waits);
        self.stack.serve(
            &mut self.core,
            &self.context,
            Vec::new(),
            asked,
            |id, secret| {
                let name = secret.map(|secret| match secret {
                    CoreSecret::Agent(agent) => agent.name,
                    _ => panic!("only agents were asked for"),
                });
                answers.push((id, name));
                waits
            },
        );
    }

    /// Steps frames, each once a request answered, until `done`.
    #[track_caller]
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) {
        for _ in 0..64 {
            // Read before the frame, so a letter posted while it runs is not missed.
            let seen = be_host_base::host::letters_posted();
            self.step(Vec::new());
            if done(self) {
                return;
            }
            assert!(
                be_host_base::host::wait_for_letter(seen, Duration::from_secs(60)),
                "the server answers in time while waiting until {what}"
            );
        }
        panic!("{what} did not happen");
    }

    /// Whether the organization lists an agent named `name`, as the core last heard.
    fn lists(&self, name: &str) -> bool {
        self.core.agents.iter().any(|agent| agent.name == name)
    }
}
