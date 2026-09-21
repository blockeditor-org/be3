use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use be_block::{BlockContent, CounterContent, CounterOp, LiveEdit};
use uuid::Uuid;

use super::*;
use crate::platform;

mod a_counter_lives_in_the_new_stack_and_survives_a_reconnect;
mod an_unmigrated_block_type_has_no_content_in_the_new_stack;
mod flushing_seals_what_the_sessions_hold_and_leaves_them_live;

const PATIENCE: Duration = Duration::from_secs(20);

struct Harness {
    directory: PathBuf,
    server: platform::EmbeddedServer,
    password: String,
    key: [u8; 32],
}

impl Harness {
    fn start() -> Self {
        let directory = std::env::temp_dir().join(format!("block-app-be-test-{}", Uuid::new_v4()));
        let server = platform::start_embedded_be_server(directory.join("server"))
            .expect("the embedded be server starts");
        Self {
            directory,
            server,
            password: "correct horse battery".into(),
            key: [23; 32],
        }
    }

    fn connect(&self, be_workspace: Option<Uuid>) -> Uuid {
        let workspace = Uuid::new_v4();
        start(Config {
            url: self.server.url.clone(),
            directory: self.directory.join("objects"),
            account: Uuid::new_v4(),
            workspace,
            email: "counter@example.com".into(),
            display_name: "Counter".into(),
            password: self.password.clone(),
            key: self.key,
            be_workspace,
            context: eframe::egui::Context::default(),
        });
        workspace
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

fn wait_for_count(block: Uuid, expected: i64) {
    let deadline = Instant::now() + PATIENCE;
    while Instant::now() < deadline {
        if count_of(block) == Some(expected) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "the new stack never reported {expected} for {block}: it holds {:?}, and the stack says {:?}",
        count_of(block),
        status().error
    );
}

fn add(block: Uuid, by: i64) {
    operate(
        block,
        CounterContent::encode_operation(&CounterOp::Add { by }),
    );
}
