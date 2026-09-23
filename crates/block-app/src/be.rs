use std::sync::{Arc, Condvar, Mutex, OnceLock};

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

pub(crate) use worker::{History, Shared};

use worker::Command;

pub(crate) struct Config {
    pub(crate) server_url: String,
    pub(crate) token: String,
    pub(crate) account: Uuid,
    pub(crate) workspace: Uuid,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) data_dir: PathBuf,
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

struct Migrated {
    block_type: Uuid,
    content_type: Uuid,
    join: worker::Join,
    copy: worker::Copy,
    name: fn(&[u8]) -> Option<String>,
}

const fn migrated<B, C>() -> Migrated
where
    B: Block,
    C: be_block::LiveEdit + be_block::Merge + Clone + Default,
{
    Migrated {
        block_type: B::TYPE_ID,
        content_type: C::CONTENT_TYPE,
        join: worker::join::<C>,
        copy: worker::copy::<C>,
        name: content_name::<C>,
    }
}

const fn migrated_with_history<B, C>() -> Migrated
where
    B: Block,
    C: be_block::Undo + be_block::Merge + Clone + Default,
{
    Migrated {
        block_type: B::TYPE_ID,
        content_type: C::CONTENT_TYPE,
        join: worker::join_with_history::<C>,
        copy: worker::copy::<C>,
        name: content_name::<C>,
    }
}

fn content_name<C: be_block::BlockContent>(bytes: &[u8]) -> Option<String> {
    C::decode(bytes).ok()?.name()
}

const MIGRATED: &[Migrated] = &[
    migrated_with_history::<block_client::blocks::calendar::Calendar, be_block::CalendarContent>(),
    migrated::<block_client::blocks::checklist::Checklist, be_block::ChecklistContent>(),
    migrated::<block_client::blocks::counter::Counter, be_block::CounterContent>(),
    migrated::<block_client::blocks::ui_settings::UiSettings, be_block::UiSettingsContent>(),
    migrated::<block_client::blocks::web_browser_tab::WebBrowserTab, be_block::BrowserTabContent>(),
];

pub(crate) fn content_type_for(block_type: Uuid) -> Option<Uuid> {
    MIGRATED
        .iter()
        .find(|migrated| migrated.block_type == block_type)
        .map(|migrated| migrated.content_type)
}

pub(crate) fn name_of(content: &Content) -> Option<String> {
    let migrated = MIGRATED
        .iter()
        .find(|migrated| migrated.content_type == content.content_type)?;
    (migrated.name)(&content.bytes)
}

fn copy_for(content_type: Uuid) -> Option<worker::Copy> {
    MIGRATED
        .iter()
        .find(|migrated| migrated.content_type == content_type)
        .map(|migrated| migrated.copy)
}

fn join_for(content_type: Uuid) -> Option<worker::Join> {
    MIGRATED
        .iter()
        .find(|migrated| migrated.content_type == content_type)
        .map(|migrated| migrated.join)
}

struct Stack {
    account: Uuid,
    workspace: Uuid,
    commands: Option<UnboundedSender<Command>>,
    held: std::collections::HashSet<Uuid>,
    shared: Arc<Mutex<Shared>>,
    #[cfg_attr(not(test), allow(dead_code))]
    changed: Arc<Condvar>,
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
    let changed = Arc::new(Condvar::new());
    let account = config.account;
    let workspace = config.workspace;
    let running = platform::spawn(config, receiver, Arc::clone(&shared), Arc::clone(&changed));
    *stack().lock().unwrap() = Some(Stack {
        account,
        workspace,
        commands: Some(commands),
        held: std::collections::HashSet::new(),
        shared,
        changed,
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
    let held = stack()
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|stack| stack.held.contains(&block));
    if !held {
        send(Command::Close(block));
    }
}

pub(crate) fn hold(block: Uuid, block_type: Uuid) {
    let Some(content_type) = content_type_for(block_type) else {
        return;
    };
    let fresh = stack()
        .lock()
        .unwrap()
        .as_mut()
        .is_some_and(|stack| stack.held.insert(block));
    if fresh {
        send(Command::Open(block, content_type));
    }
}

pub(crate) fn operate(block: Uuid, operation: Vec<u8>) {
    send(Command::Operate(block, operation));
}

pub(crate) fn history(block: Uuid) -> History {
    with_shared(|shared| shared.histories.get(&block).copied())
        .flatten()
        .unwrap_or_default()
}

pub(crate) fn undo(block: Uuid) {
    send(Command::History { block, redo: false });
}

pub(crate) fn redo(block: Uuid) {
    send(Command::History { block, redo: true });
}

pub(crate) fn duplicate(from: Uuid, to: Uuid, block_type: Uuid) {
    if let Some(content_type) = content_type_for(block_type) {
        send(Command::Duplicate {
            from,
            to,
            content_type,
        });
    }
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

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) fn wait_for<T>(
    timeout: std::time::Duration,
    read: impl Fn(&Shared) -> Option<T>,
) -> Option<T> {
    let (shared, changed) = stack()
        .lock()
        .unwrap()
        .as_ref()
        .map(|stack| (Arc::clone(&stack.shared), Arc::clone(&stack.changed)))?;
    let deadline = std::time::Instant::now() + timeout;
    let mut held = shared.lock().unwrap();
    loop {
        if let Some(value) = read(&held) {
            return Some(value);
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        held = changed.wait_timeout(held, remaining).unwrap().0;
    }
}

fn with_shared<T>(read: impl FnOnce(&Shared) -> T) -> Option<T> {
    let held = stack().lock().unwrap();
    let stack = held.as_ref()?;
    let shared = stack.shared.lock().unwrap();
    Some(read(&shared))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) use tests::Harness;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
