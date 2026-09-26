use std::sync::{Arc, Condvar, Mutex, OnceLock};

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use uuid::Uuid;

mod graph;
mod version;
mod worker;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
use native as platform;
#[cfg(target_arch = "wasm32")]
use web as platform;

pub(crate) use graph::{Graph, Node, Query, Scope};
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
        crate::accounts::socket_url(&self.server_url)
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

type MergeContent = fn(Option<&[u8]>, Option<&[u8]>, Option<&[u8]>) -> Option<(Vec<u8>, bool)>;

type ContentReferences = fn(&[u8], Uuid) -> Vec<Uuid>;

struct Kind {
    content_type: Uuid,
    join: worker::Join,
    copy: worker::Copy,
    seed: worker::Seed,
    replace: worker::Seed,
    describe: fn(&[u8]) -> Option<Described>,
    child: ChildOperations,
    merge: MergeContent,
    references: ContentReferences,
}

const fn kind<C>() -> Kind
where
    C: be_block::LiveEdit + be_block::Merge + Clone + Default,
{
    Kind {
        content_type: C::CONTENT_TYPE,
        join: worker::join::<C>,
        copy: worker::copy::<C>,
        seed: worker::seed::<C>,
        replace: worker::replace::<C>,
        describe: describe::<C>,
        child: child_operations::<C>,
        merge: merge_content::<C>,
        references: content_references::<C>,
    }
}

const fn kind_with_history<C>() -> Kind
where
    C: be_block::Undo + be_block::Merge + Clone + Default,
{
    Kind {
        content_type: C::CONTENT_TYPE,
        join: worker::join_with_history::<C>,
        copy: worker::copy::<C>,
        seed: worker::seed::<C>,
        replace: worker::replace::<C>,
        describe: describe::<C>,
        child: child_operations::<C>,
        merge: merge_content::<C>,
        references: content_references::<C>,
    }
}

#[derive(Default)]
pub(crate) struct Described {
    pub(crate) name: Option<String>,
    pub(crate) derived: be_block::DerivedMetadata,
}

fn describe<C: be_block::BlockContent>(bytes: &[u8]) -> Option<Described> {
    let content = C::decode(bytes).ok()?;
    Some(Described {
        name: content.name(),
        derived: content.derived_metadata(),
    })
}

fn merge_content<C: be_block::Merge + Default>(
    base: Option<&[u8]>,
    ours: Option<&[u8]>,
    theirs: Option<&[u8]>,
) -> Option<(Vec<u8>, bool)> {
    let decode = |bytes: Option<&[u8]>| match bytes {
        Some(bytes) => C::decode(bytes).ok(),
        None => Some(C::default()),
    };
    match C::merge3(&decode(base)?, &decode(ours)?, &decode(theirs)?) {
        be_commit::MergeResult::Clean(value) => Some((value.encode(), false)),
        be_commit::MergeResult::Conflicted { value, .. } => Some((value.encode(), true)),
    }
}

fn content_references<C: be_block::BlockContent>(bytes: &[u8], workspace: Uuid) -> Vec<Uuid> {
    C::decode(bytes)
        .map(|content| content.references_in(workspace))
        .unwrap_or_default()
}

fn child_operations<C: be_block::LiveEdit>(
    bytes: &[u8],
    change: be_block::ChildChange,
) -> Option<Vec<Vec<u8>>> {
    let operations = C::decode(bytes).ok()?.child_operations(change)?;
    Some(operations.iter().map(C::encode_operation).collect())
}

const KINDS: &[Kind] = &[
    kind::<be_block::AudioContent>(),
    kind_with_history::<be_block::CalendarContent>(),
    kind_with_history::<be_block::ChecklistContent>(),
    kind::<be_block::CompiledLogicContent>(),
    kind_with_history::<be_block::CounterContent>(),
    kind_with_history::<be_block::DatabaseContent>(),
    kind_with_history::<be_block::DatabaseSchemaContent>(),
    kind_with_history::<be_block::DatabaseViewContent>(),
    kind_with_history::<be_block::DeterministicGameContent>(),
    kind::<be_block::GameModuleContent>(),
    kind_with_history::<be_block::HotbarContent>(),
    kind::<be_block::ImageContent>(),
    kind_with_history::<be_block::CanvasContent>(),
    kind_with_history::<be_block::LogicGameContent>(),
    kind_with_history::<be_block::LogicGridContent>(),
    kind_with_history::<be_block::MapContent>(),
    kind_with_history::<be_block::PaintReviewContent>(),
    kind::<be_block::PaintSnapshotContent>(),
    kind::<be_block::PdfContent>(),
    kind_with_history::<be_block::PixelArtContent>(),
    kind_with_history::<be_block::PresentationContent>(),
    kind::<be_block::TextContent>(),
    kind_with_history::<be_block::VideoContent>(),
    kind::<be_block::UiSettingsContent>(),
    kind::<be_block::BrowserTabContent>(),
    kind_with_history::<be_block::SettingsContent>(),
    kind::<be_block::FolderContent>(),
    kind_with_history::<be_block::PixelRayTracerContent>(),
    kind::<be_block::FileTreeContent>(),
    kind::<be_block::PanZoomContent>(),
    kind::<be_block::Scene3dContent>(),
    kind::<be_block::TriangleContent>(),
    kind::<be_block::WorkspaceUiContent>(),
    kind::<be_block::RepositoryContent>(),
    kind::<be_block::CheckoutContent>(),
];

fn kind_of(content_type: Uuid) -> Option<&'static Kind> {
    KINDS.iter().find(|kind| kind.content_type == content_type)
}

pub(crate) fn describe_of(content: &Content) -> Option<Described> {
    (kind_of(content.content_type)?.describe)(&content.bytes)
}

pub(crate) fn is_known(content_type: Uuid) -> bool {
    kind_of(content_type).is_some()
}

fn merge_for(content_type: Uuid) -> Option<MergeContent> {
    kind_of(content_type).map(|kind| kind.merge)
}

fn references_for(content_type: Uuid) -> Option<ContentReferences> {
    kind_of(content_type).map(|kind| kind.references)
}

fn copy_for(content_type: Uuid) -> Option<worker::Copy> {
    kind_of(content_type).map(|kind| kind.copy)
}

fn replace_for(content_type: Uuid) -> Option<worker::Seed> {
    kind_of(content_type).map(|kind| kind.replace)
}

fn seed_for(content_type: Uuid) -> Option<worker::Seed> {
    kind_of(content_type).map(|kind| kind.seed)
}

fn join_for(content_type: Uuid) -> Option<worker::Join> {
    kind_of(content_type).map(|kind| kind.join)
}

struct Stack {
    account: Uuid,
    workspace: Uuid,
    commands: Option<UnboundedSender<Command>>,
    held: std::collections::HashSet<Uuid>,
    versioned: std::collections::HashSet<Uuid>,
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
        versioned: std::collections::HashSet::new(),
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

pub(crate) fn hold(block: Uuid, content_type: Uuid) {
    if !is_known(content_type) {
        return;
    }
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
    content_type: Uuid,
    change: be_block::ChildChange,
) -> Option<bool> {
    let kind = kind_of(content_type)?;
    let Some(held) = content(block) else {
        hold(block, content_type);
        return None;
    };
    for operation in (kind.child)(&held.bytes, change)? {
        send(Command::Operate(block, None, operation));
    }
    Some(true)
}

pub(crate) fn version(block: Uuid, command: block_plugin_api::VersionCommand) {
    send(Command::Version { block, command });
}

pub(crate) fn version_since(
    block: Uuid,
    content_type: Uuid,
    sent: Option<u64>,
) -> Option<(u64, block_plugin_api::VersionStatus)> {
    let fresh = stack()
        .lock()
        .unwrap()
        .as_mut()
        .is_some_and(|stack| stack.versioned.insert(block));
    if fresh {
        hold(block, content_type);
        send(Command::WatchVersion(block));
    }
    with_shared(|shared| {
        let state = shared.versions.get(&block)?;
        (sent != Some(state.revision)).then(|| (state.revision, state.status.clone()))
    })?
}

pub(crate) fn is_versioned(content_type: Uuid) -> bool {
    content_type == <be_block::Checkout as be_block::Root>::CONTENT_TYPE
        || content_type == <be_block::Repository as be_block::Root>::CONTENT_TYPE
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

pub(crate) fn duplicate(from: Uuid) -> Option<(Uuid, Uuid)> {
    let source = node(from)?;
    kind_of(source.content_type)?;
    let to = Uuid::new_v4();
    let mut metadata = source.metadata.clone();
    metadata.artifact = None;
    let author = account().unwrap_or_default();
    with_shared_mut(|shared| {
        shared.graph.change(Node {
            id: to,
            content_type: source.content_type,
            author,
            parent: be_graph::BlockParent::Detached,
            access: be_graph::Access::Edit,
            references: source.references.clone(),
            metadata: metadata.clone(),
            head: None,
            version: 0,
        });
    });
    send(Command::Duplicate {
        from,
        to,
        content_type: source.content_type,
        metadata,
    });
    Some((to, source.content_type))
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

fn with_shared_mut<T>(change: impl FnOnce(&mut Shared) -> T) -> Option<T> {
    let held = stack().lock().unwrap();
    let stack = held.as_ref()?;
    let mut shared = stack.shared.lock().unwrap();
    Some(change(&mut shared))
}

pub(crate) fn account() -> Option<Uuid> {
    stack().lock().unwrap().as_ref().map(|stack| stack.account)
}

pub(crate) fn identity() -> Option<(Uuid, Uuid)> {
    stack()
        .lock()
        .unwrap()
        .as_ref()
        .map(|stack| (stack.account, stack.workspace))
}

pub(crate) fn graph_revision() -> u64 {
    with_shared(|shared| shared.graph.revision).unwrap_or_default()
}

pub(crate) fn graph_loaded() -> bool {
    with_shared(|shared| shared.graph.loaded).unwrap_or_default()
}

pub(crate) fn with_graph<T>(read: impl FnOnce(&Graph) -> T) -> Option<T> {
    with_shared(|shared| read(&shared.graph))
}

pub(crate) fn query(query: Query) -> Vec<Node> {
    with_shared(|shared| shared.graph.query(query)).unwrap_or_default()
}

pub(crate) fn nodes() -> Vec<Node> {
    with_shared(|shared| shared.graph.nodes()).unwrap_or_default()
}

pub(crate) fn node(block: Uuid) -> Option<Node> {
    with_shared(|shared| shared.graph.get(block).cloned())?
}

pub(crate) fn access(block: Uuid) -> be_graph::Access {
    node(block).map_or(be_graph::Access::None, |node| node.access)
}

pub(crate) fn create(
    block: Uuid,
    content_type: Uuid,
    parent: be_graph::BlockParent,
    metadata: be_block::BlockMetadata,
    content: Option<Vec<u8>>,
) {
    let author = account().unwrap_or_default();
    with_shared_mut(|shared| {
        shared.graph.change(Node {
            id: block,
            content_type,
            author,
            parent,
            access: be_graph::Access::Edit,
            references: Vec::new(),
            metadata: metadata.clone(),
            head: None,
            version: 0,
        });
    });
    send(Command::Create {
        block,
        content_type,
        parent,
        metadata,
        bytes: content,
    });
}

pub(crate) fn set_parent(block: Uuid, parent: be_graph::BlockParent) {
    with_shared_mut(|shared| shared.graph.update(block, |node| node.parent = parent));
    send(Command::SetParent { block, parent });
}

pub(crate) fn set_metadata(block: Uuid, metadata: be_block::BlockMetadata) {
    with_shared_mut(|shared| {
        shared
            .graph
            .update(block, |node| node.metadata = metadata.clone());
    });
    send(Command::SetMetadata { block, metadata });
}

pub(crate) fn set_name(block: Uuid, name: Option<String>) {
    let Some(mut metadata) = node(block).map(|node| node.metadata) else {
        return;
    };
    metadata.named_by_hand = name.is_some();
    if name.is_some() {
        metadata.name = name;
    }
    set_metadata(block, metadata);
}

pub(crate) fn describe_implicitly(block: Uuid, described: Described) {
    let Some(original) = node(block).map(|node| node.metadata) else {
        return;
    };
    let mut metadata = original.clone();
    if !metadata.named_by_hand {
        metadata.name = described.name;
    }
    metadata.derived = described.derived;
    if metadata != original {
        set_metadata(block, metadata);
    }
}

pub(crate) fn set_access(block: Uuid, account: Uuid, access: be_graph::Access) {
    send(Command::SetAccess {
        block,
        account,
        access,
    });
}

pub(crate) fn list_access(
    block: Uuid,
) -> std::sync::mpsc::Receiver<Result<Vec<be_protocol::AccessEntry>, String>> {
    let (reply, received) = crate::host::waking_channel();
    send(Command::ListAccess { block, reply });
    received
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
