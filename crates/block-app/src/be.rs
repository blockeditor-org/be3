use std::sync::{Arc, Mutex, OnceLock};

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use block::Block;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use uuid::Uuid;

mod worker;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
use native as platform;
#[cfg(target_arch = "wasm32")]
use web as platform;

use worker::{Command, Shared};

pub(crate) struct Config {
    pub(crate) server_url: String,
    pub(crate) token: String,
    pub(crate) account: Uuid,
    pub(crate) workspace: Uuid,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) data_dir: PathBuf,
    pub(crate) context: eframe::egui::Context,
}

impl Config {
    fn socket_url(&self) -> String {
        let base = match self.server_url.split_once("://") {
            Some(("http", host)) => format!("ws://{host}"),
            Some(("https", host)) => format!("wss://{host}"),
            _ => self.server_url.clone(),
        };
        format!(
            "{base}/api/be?token={}&workspace={}",
            self.token, self.workspace
        )
    }

    fn content_key(&self) -> [u8; 32] {
        *be_store::Hash::of_parts(&[CONTENT_KEY_LABEL, self.workspace.as_bytes()]).as_bytes()
    }
}

const CONTENT_KEY_LABEL: &[u8] = b"be3.workspace.content-key.v1";

#[derive(Clone)]
pub(crate) struct Content {
    pub(crate) content_type: Uuid,
    pub(crate) bytes: Vec<u8>,
    pub(crate) revision: u64,
}

#[derive(Default)]
pub(crate) struct Status {
    pub(crate) running: bool,
    pub(crate) connected: bool,
    pub(crate) blocks: usize,
    pub(crate) wakes: u64,
    pub(crate) unsealed: usize,
    pub(crate) error: Option<String>,
}

pub(crate) fn content_type_for(block_type: Uuid) -> Option<Uuid> {
    (block_type == block_client::blocks::counter::Counter::TYPE_ID)
        .then_some(<be_block::CounterContent as be_block::BlockContent>::CONTENT_TYPE)
}

struct Stack {
    account: Uuid,
    workspace: Uuid,
    commands: Option<UnboundedSender<Command>>,
    shared: Arc<Mutex<Shared>>,
    running: platform::Running,
}

impl Drop for Stack {
    fn drop(&mut self) {
        self.commands = None;
        self.running.finish();
    }
}

fn stack() -> &'static Mutex<Option<Stack>> {
    static STACK: OnceLock<Mutex<Option<Stack>>> = OnceLock::new();
    STACK.get_or_init(|| Mutex::new(None))
}

pub(crate) fn start(config: Config) {
    stop();
    let (commands, receiver) = unbounded_channel();
    let shared = Arc::new(Mutex::new(Shared::default()));
    let account = config.account;
    let workspace = config.workspace;
    let running = platform::spawn(config, receiver, Arc::clone(&shared));
    *stack().lock().unwrap() = Some(Stack {
        account,
        workspace,
        commands: Some(commands),
        shared,
        running,
    });
}

pub(crate) fn installed_for(account: Uuid, workspace: Uuid) -> bool {
    stack()
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|stack| stack.account == account && stack.workspace == workspace)
}

pub(crate) fn stop() {
    let stack = stack().lock().unwrap().take();
    drop(stack);
}

pub(crate) fn flush() {
    let (done, sealed) = std::sync::mpsc::channel();
    send(Command::Flush(done));
    platform::await_flush(sealed);
}

pub(crate) fn open(block: Uuid, content_type: Uuid) {
    send(Command::Open(block, content_type));
}

pub(crate) fn close(block: Uuid) {
    send(Command::Close(block));
}

pub(crate) fn operate(block: Uuid, operation: Vec<u8>) {
    send(Command::Operate(block, operation));
}

pub(crate) fn content(block: Uuid) -> Option<Content> {
    with_shared(|shared| shared.blocks.get(&block).cloned())?
}

pub(crate) fn status() -> Status {
    with_shared(|shared| Status {
        running: true,
        connected: shared.connected,
        blocks: shared.blocks.len(),
        wakes: shared.wakes,
        unsealed: shared.unsealed,
        error: shared.error.clone(),
    })
    .unwrap_or_default()
}

fn send(command: Command) {
    if let Some(commands) = stack()
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|stack| stack.commands.as_ref())
    {
        let _ = commands.send(command);
    }
}

fn with_shared<T>(read: impl FnOnce(&Shared) -> T) -> Option<T> {
    let held = stack().lock().unwrap();
    let stack = held.as_ref()?;
    let shared = stack.shared.lock().unwrap();
    Some(read(&shared))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
