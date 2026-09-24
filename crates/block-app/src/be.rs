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

pub(crate) use worker::{History, Presence, Shared};

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

const LOG_LIMIT: usize = 512;

#[derive(Clone)]
pub(crate) struct Content {
    pub(crate) content_type: Uuid,
    pub(crate) bytes: Vec<u8>,
    pub(crate) revision: u64,
    log: std::collections::VecDeque<(Vec<u8>, Option<u64>)>,
    taken: std::collections::HashMap<u64, u64>,
}

impl Content {
    pub(crate) fn new(content_type: Uuid, bytes: Vec<u8>) -> Self {
        Self {
            content_type,
            bytes,
            revision: 1,
            log: std::collections::VecDeque::new(),
            taken: std::collections::HashMap::new(),
        }
    }

    pub(crate) fn record(&mut self, entry: worker::Logged) {
        self.revision += 1;
        match entry {
            worker::Logged::Operation { bytes, origin } => {
                if let Some(origin) = origin {
                    *self.taken.entry(origin).or_default() += 1;
                }
                self.log.push_back((bytes, origin));
                if self.log.len() > LOG_LIMIT {
                    self.log.pop_front();
                }
            }
            worker::Logged::Replaced => self.log.clear(),
        }
    }

    pub(crate) fn since(&self, origin: u64, sent: Option<u64>) -> Update {
        let behind = sent.map(|sent| self.revision.saturating_sub(sent));
        match behind {
            Some(behind) if behind as usize <= self.log.len() => Update::Operations(
                self.log
                    .iter()
                    .skip(self.log.len() - behind as usize)
                    .map(|(bytes, from)| (bytes.clone(), *from == Some(origin)))
                    .collect(),
            ),
            _ => Update::Snapshot {
                content_type: self.content_type,
                bytes: self.bytes.clone(),
                applied: self.taken.get(&origin).copied().unwrap_or_default(),
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Update {
    Snapshot {
        content_type: Uuid,
        bytes: Vec<u8>,
        applied: u64,
    },
    Operations(Vec<(Vec<u8>, bool)>),
}

pub(crate) fn next_origin() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
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

type ChildOperations = fn(&[u8], be_block::ChildChange) -> Option<Vec<Vec<u8>>>;

struct Migrated {
    block_type: Uuid,
    content_type: Uuid,
    join: worker::Join,
    copy: worker::Copy,
    seed: worker::Seed,
    replace: worker::Seed,
    name: fn(&[u8]) -> Option<String>,
    references: fn(&[u8], Uuid) -> Vec<Uuid>,
    child: ChildOperations,
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
        seed: worker::seed::<C>,
        replace: worker::replace::<C>,
        name: content_name::<C>,
        references: content_references::<C>,
        child: child_operations::<C>,
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
        seed: worker::seed::<C>,
        replace: worker::replace::<C>,
        name: content_name::<C>,
        references: content_references::<C>,
        child: child_operations::<C>,
    }
}

fn content_name<C: be_block::BlockContent>(bytes: &[u8]) -> Option<String> {
    C::decode(bytes).ok()?.name()
}

fn child_operations<C: be_block::LiveEdit>(
    bytes: &[u8],
    change: be_block::ChildChange,
) -> Option<Vec<Vec<u8>>> {
    let operations = C::decode(bytes).ok()?.child_operations(change)?;
    Some(operations.iter().map(C::encode_operation).collect())
}

fn content_references<C: be_block::BlockContent>(bytes: &[u8], workspace: Uuid) -> Vec<Uuid> {
    C::decode(bytes)
        .map(|content| content.references_in(workspace))
        .unwrap_or_default()
}

const MIGRATED: &[Migrated] = &[
    migrated::<block_client::blocks::audio::Audio, be_block::AudioContent>(),
    migrated_with_history::<block_client::blocks::calendar::Calendar, be_block::CalendarContent>(),
    migrated_with_history::<block_client::blocks::checklist::Checklist, be_block::ChecklistContent>(
    ),
    migrated::<block_client::blocks::compiled_logic::CompiledLogic, be_block::CompiledLogicContent>(
    ),
    migrated_with_history::<block_client::blocks::counter::Counter, be_block::CounterContent>(),
    migrated_with_history::<block_client::blocks::database::Database, be_block::DatabaseContent>(),
    migrated_with_history::<
        block_client::blocks::database_schema::DatabaseSchema,
        be_block::DatabaseSchemaContent,
    >(),
    migrated_with_history::<
        block_client::blocks::database_view::DatabaseView,
        be_block::DatabaseViewContent,
    >(),
    migrated_with_history::<
        block_client::blocks::deterministic_game::DeterministicGame,
        be_block::DeterministicGameContent,
    >(),
    migrated::<block_client::blocks::game_module::GameModule, be_block::GameModuleContent>(),
    migrated_with_history::<block_client::blocks::hotbar::Hotbar, be_block::HotbarContent>(),
    migrated::<block_client::blocks::image::Image, be_block::ImageContent>(),
    migrated_with_history::<
        block_client::blocks::infinite_canvas::InfiniteCanvas,
        be_block::CanvasContent,
    >(),
    migrated_with_history::<block_client::blocks::logic_game::LogicGame, be_block::LogicGameContent>(
    ),
    migrated_with_history::<block_client::blocks::logic_grid::LogicGrid, be_block::LogicGridContent>(
    ),
    migrated_with_history::<block_client::blocks::map::Map, be_block::MapContent>(),
    migrated_with_history::<
        block_client::blocks::paint_review::PaintReview,
        be_block::PaintReviewContent,
    >(),
    migrated::<block_client::blocks::paint_snapshot::PaintSnapshot, be_block::PaintSnapshotContent>(
    ),
    migrated::<block_client::blocks::pdf::Pdf, be_block::PdfContent>(),
    migrated_with_history::<block_client::blocks::pixel_art::PixelArt, be_block::PixelArtContent>(),
    migrated_with_history::<
        block_client::blocks::presentation::Presentation,
        be_block::PresentationContent,
    >(),
    migrated::<block_client::blocks::text::TextDocument, be_block::TextContent>(),
    migrated_with_history::<block_client::blocks::video::Video, be_block::VideoContent>(),
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

pub(crate) fn references_of(content: &Content) -> Option<Vec<Uuid>> {
    let migrated = MIGRATED
        .iter()
        .find(|migrated| migrated.content_type == content.content_type)?;
    let workspace = stack().lock().unwrap().as_ref()?.workspace;
    Some((migrated.references)(&content.bytes, workspace))
}

pub(crate) fn block_type_of(content_type: Uuid) -> Option<Uuid> {
    MIGRATED
        .iter()
        .find(|migrated| migrated.content_type == content_type)
        .map(|migrated| migrated.block_type)
}

pub(crate) fn is_migrated(content_type: Uuid) -> bool {
    MIGRATED
        .iter()
        .any(|migrated| migrated.content_type == content_type)
}

fn copy_for(content_type: Uuid) -> Option<worker::Copy> {
    MIGRATED
        .iter()
        .find(|migrated| migrated.content_type == content_type)
        .map(|migrated| migrated.copy)
}

fn replace_for(content_type: Uuid) -> Option<worker::Seed> {
    MIGRATED
        .iter()
        .find(|migrated| migrated.content_type == content_type)
        .map(|migrated| migrated.replace)
}

fn seed_for(content_type: Uuid) -> Option<worker::Seed> {
    MIGRATED
        .iter()
        .find(|migrated| migrated.content_type == content_type)
        .map(|migrated| migrated.seed)
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

pub(crate) fn change_child(
    block: Uuid,
    block_type: Uuid,
    change: be_block::ChildChange,
) -> Option<bool> {
    let migrated = MIGRATED
        .iter()
        .find(|migrated| migrated.block_type == block_type)?;
    let Some(held) = content(block) else {
        hold(block, block_type);
        return None;
    };
    for operation in (migrated.child)(&held.bytes, change)? {
        send(Command::Operate(block, None, operation));
    }
    Some(true)
}

pub(crate) fn operate_from(block: Uuid, origin: u64, operation: Vec<u8>) {
    send(Command::Operate(block, Some(origin), operation));
}

pub(crate) fn update_since(block: Uuid, origin: u64, sent: Option<u64>) -> Option<(u64, Update)> {
    with_shared(|shared| {
        let content = shared.blocks.get(&block)?;
        (sent != Some(content.revision)).then(|| (content.revision, content.since(origin, sent)))
    })?
}

pub(crate) fn show(block: Uuid, kind: Uuid, value: Option<Vec<u8>>) {
    send(Command::Presence { block, kind, value });
}

pub(crate) fn presence_since(block: Uuid, sent: Option<u64>) -> Option<(u64, Vec<Presence>)> {
    with_shared(|shared| {
        let (revision, presence) = shared.presence.get(&block)?;
        (sent != Some(*revision)).then(|| (*revision, presence.clone()))
    })?
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

pub(crate) fn replace(block: Uuid, content_type: Uuid, bytes: Vec<u8>) {
    send(Command::Replace {
        block,
        content_type,
        bytes,
    });
}

pub(crate) fn seed(block: Uuid, content_type: Uuid, bytes: Vec<u8>) {
    send(Command::Seed {
        block,
        content_type,
        bytes,
    });
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
