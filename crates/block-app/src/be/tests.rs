use std::time::Duration;

use be_block::{BlockContent, Checklist, ChecklistContent, Counter, CounterContent, LiveEdit};
use block_client::ManagementClient;
use uuid::Uuid;

use super::*;
use crate::platform;

mod a_checklist_and_a_counter_are_held_by_one_peer;
mod a_child_moved_into_a_migrated_block_is_added_to_its_content;
mod a_counter_lives_in_the_new_stack_and_survives_a_reconnect;
mod a_duplicated_block_carries_what_its_source_held;
mod a_held_block_stays_open_when_its_editors_close;
mod a_peer_rejoins_what_was_open_when_the_server_comes_back;
mod an_edit_made_across_a_takeover_is_kept;
mod an_idle_peer_never_wakes_its_worker;
mod an_unmigrated_block_type_has_no_content_in_the_new_stack;
mod flushing_seals_what_the_sessions_hold_and_leaves_them_live;
mod replacing_content_reaches_a_block_open_or_not;
mod two_peers_of_one_workspace_share_a_counter;
mod undo_steps_back_through_what_this_peer_did;

const TEST_ORIGIN: u64 = 0;

fn operate(block: Uuid, operation: Vec<u8>) {
    operate_from(block, TEST_ORIGIN, operation);
}

const PATIENCE: Duration = Duration::from_secs(20);
const QUIET: Duration = Duration::from_secs(2);

pub(crate) struct Harness {
    directory: PathBuf,
    server: Option<platform::EmbeddedServer>,
    url: String,
    account: Uuid,
    token: String,
    workspace: Uuid,
    _one_stack: std::sync::MutexGuard<'static, ()>,
}

impl Harness {
    pub(crate) fn start() -> Self {
        static ONE_STACK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let one_stack = ONE_STACK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
            _one_stack: one_stack,
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
        start(self.config(self.directory.join("objects")));
    }

    fn config(&self, data_dir: PathBuf) -> Config {
        Config {
            server_url: self.url.clone(),
            token: self.token.clone(),
            account: self.account,
            workspace: self.workspace,
            data_dir,
        }
    }

    async fn outside_peer(&self) -> std::sync::Arc<be_client::Peer<be_store::MemoryStore>> {
        let config = self.config(self.directory.join("outside"));
        let peer = be_client::Peer::connect(
            be_client::PeerConfig::new(
                config.socket_url(),
                be_store::ContentKey::from_bytes(config.content_key()),
                be_client::Credentials::Adopted,
            )
            .workspace(Some(self.workspace)),
            be_store::MemoryStore::new(),
        )
        .await
        .expect("a second peer joins the workspace");
        std::sync::Arc::new(peer)
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
        .map(|counter| counter.root().value())
}

fn counted(shared: &Shared, block: Uuid) -> Option<i64> {
    let held = shared.blocks.get(&block)?;
    (held.content_type == CounterContent::CONTENT_TYPE)
        .then(|| CounterContent::decode(&held.bytes).ok())
        .flatten()
        .map(|counter| counter.root().value())
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
    operate(block, CounterContent::encode_operation(&Counter::add(by)));
}
