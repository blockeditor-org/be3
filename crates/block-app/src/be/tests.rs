use std::time::Duration;

use be_block::{BlockContent, ChecklistContent, ChecklistOp, CounterContent, CounterOp, LiveEdit};
use block_client::ManagementClient;
use uuid::Uuid;

use super::*;
use crate::platform;

mod a_checklist_and_a_counter_are_held_by_one_peer;
mod a_counter_lives_in_the_new_stack_and_survives_a_reconnect;
mod a_peer_rejoins_what_was_open_when_the_server_comes_back;
mod an_idle_peer_never_wakes_its_worker;
mod an_unmigrated_block_type_has_no_content_in_the_new_stack;
mod flushing_seals_what_the_sessions_hold_and_leaves_them_live;
mod two_peers_of_one_workspace_share_a_counter;

const PATIENCE: Duration = Duration::from_secs(20);
const QUIET: Duration = Duration::from_secs(2);

pub(crate) struct Harness {
    directory: PathBuf,
    server: Option<platform::EmbeddedServer>,
    url: String,
    account: Uuid,
    token: String,
    workspace: Uuid,
}

impl Harness {
    pub(crate) fn start() -> Self {
        let directory = std::env::temp_dir().join(format!("block-app-be-test-{}", Uuid::new_v4()));
        let server = platform::start_embedded_server(directory.join("server"))
            .expect("the embedded block server starts");
        let url = server.url.clone();
        let managed = url.clone();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a test runtime starts");
        let (account, token, workspace) = runtime.block_on(async {
            let client = ManagementClient::new(managed).expect("the management url is valid");
            let session = client
                .register("counter@example.com", "Counter", "correct horse battery")
                .await
                .expect("the account registers");
            let workspace = client
                .create_workspace(&session.token, "Counters")
                .await
                .expect("the workspace is created");
            (session.account.id, session.token, workspace.id)
        });
        Self {
            directory,
            server: Some(server),
            url,
            account,
            token,
            workspace,
        }
    }

    fn address(&self) -> String {
        self.url
            .split_once("://")
            .map(|(_, host)| host.to_owned())
            .expect("the embedded server url has a scheme")
    }

    fn stop_server(&mut self) {
        self.server = None;
    }

    fn start_server(&mut self, address: &str) {
        self.server = Some(
            platform::start_embedded_server_at(address, self.directory.join("server"))
                .expect("the embedded block server starts again"),
        );
    }

    pub(crate) fn connect(&self) {
        start(Config {
            server_url: self.url.clone(),
            token: self.token.clone(),
            account: self.account,
            workspace: self.workspace,
            data_dir: self.directory.join("objects"),
            context: eframe::egui::Context::default(),
        });
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        stop();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn count_of(block: Uuid) -> Option<i64> {
    let held = content(block)?;
    (held.content_type == CounterContent::CONTENT_TYPE)
        .then(|| CounterContent::decode(&held.bytes).ok())
        .flatten()
        .map(|counter| counter.count())
}

fn counted(shared: &Shared, block: Uuid) -> Option<i64> {
    let held = shared.blocks.get(&block)?;
    (held.content_type == CounterContent::CONTENT_TYPE)
        .then(|| CounterContent::decode(&held.bytes).ok())
        .flatten()
        .map(|counter| counter.count())
}

fn wait_for_count(block: Uuid, expected: i64) {
    let reached = wait_for(PATIENCE, |shared| {
        (counted(shared, block) == Some(expected)).then_some(())
    });
    assert!(
        reached.is_some(),
        "the new stack never reported {expected} for {block}: it holds {:?}, and the stack says {:?}",
        count_of(block),
        status().error
    );
}

fn wait_until(what: &str, ready: impl Fn(&Shared) -> bool) {
    let reached = wait_for(PATIENCE, |shared| ready(shared).then_some(()));
    assert!(
        reached.is_some(),
        "the new stack never {what}: it says {:?}",
        status().error
    );
}

fn add(block: Uuid, by: i64) {
    operate(
        block,
        CounterContent::encode_operation(&CounterOp::Add { by }),
    );
}
