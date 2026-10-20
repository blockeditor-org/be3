use std::sync::{Arc, Condvar, Mutex, OnceLock, PoisonError};

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use uuid::Uuid;

mod graph;
pub(crate) mod revisions;
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

pub(crate) use platform::sleep;

pub(crate) use graph::{Graph, Node, Query, Scope};
pub(crate) use worker::{History, PairingRequest, Presence, Shared};

use worker::Command;

pub(crate) struct Config {
    pub(crate) server_url: String,
    pub(crate) token: String,
    pub(crate) account: Uuid,
    pub(crate) organization: Uuid,
    pub(crate) identity: [u8; 32],
    pub(crate) display_name: String,
    pub(crate) email: String,
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) data_dir: PathBuf,
}

impl Config {
    fn socket_url(&self) -> String {
        crate::accounts::socket_url(&self.server_url)
    }
}

const LOG_LIMIT: usize = 512;

#[derive(Clone)]
pub(crate) struct Content {
    pub(crate) content_type: Uuid,
    pub(crate) bytes: Vec<u8>,
    pub(crate) session: Vec<u8>,
    pub(crate) derived: Option<be_block::Entries>,
    pub(crate) revision: u64,
    log: std::collections::VecDeque<(Vec<u8>, Option<u64>)>,
    taken: std::collections::HashMap<u64, u64>,
}

impl Content {
    pub(crate) fn new(
        content_type: Uuid,
        bytes: Vec<u8>,
        session: Vec<u8>,
        derived: Option<be_block::Entries>,
    ) -> Self {
        Self {
            content_type,
            bytes,
            session,
            derived,
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
            worker::Logged::Replaced { acknowledged } => {
                for (origin, count) in acknowledged {
                    *self.taken.entry(origin).or_default() += count;
                }
                self.log.clear();
            }
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
                session: self.session.clone(),
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
        session: Vec<u8>,
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
    pub(crate) conflicts: usize,
    pub(crate) error: Option<String>,
}

type ChildOperations = fn(&[u8], be_block::ChildChange) -> Option<Vec<Vec<u8>>>;

type MergeContent = fn(Option<&[u8]>, Option<&[u8]>, Option<&[u8]>) -> Option<(Vec<u8>, bool)>;

type ContentReferences = fn(&[u8], Uuid) -> Vec<Uuid>;

struct Kind {
    content_type: Uuid,
    frozen: bool,
    schema: fn() -> String,
    blank: fn() -> Vec<u8>,
    join: worker::Join,
    copy: worker::Copy,
    seed: worker::Seed,
    replace: worker::Seed,
    describe: fn(&[u8]) -> Option<be_block::Entries>,
    preview: fn(&[u8]) -> Option<Content>,
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
        frozen: C::FROZEN,
        schema: be_block::schema_fingerprint::<C>,
        blank: blank::<C>,
        join: worker::join::<C>,
        copy: worker::copy::<C>,
        seed: worker::seed::<C>,
        replace: worker::replace::<C>,
        describe: describe::<C>,
        preview: preview::<C>,
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
        frozen: C::FROZEN,
        schema: be_block::schema_fingerprint::<C>,
        blank: blank::<C>,
        join: worker::join_with_history::<C>,
        copy: worker::copy::<C>,
        seed: worker::seed::<C>,
        replace: worker::replace::<C>,
        describe: describe::<C>,
        preview: preview::<C>,
        child: child_operations::<C>,
        merge: merge_content::<C>,
        references: content_references::<C>,
    }
}

fn blank<C: be_block::BlockContent + Default>() -> Vec<u8> {
    C::default().encode()
}

fn describe<C: be_block::BlockContent>(bytes: &[u8]) -> Option<be_block::Entries> {
    Some(be_block::derived_metadata(&C::decode(bytes).ok()?))
}

fn preview<C: be_block::LiveEdit>(bytes: &[u8]) -> Option<Content> {
    let content = C::decode(bytes).ok()?;
    Some(Content::new(
        C::CONTENT_TYPE,
        bytes.to_vec(),
        content.session_state(),
        Some(be_block::derived_metadata(&content)),
    ))
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

fn content_references<C: be_block::BlockContent>(bytes: &[u8], organization: Uuid) -> Vec<Uuid> {
    C::decode(bytes)
        .map(|content| content.references_in(organization))
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
    kind::<be_block::BeuiDemoContent>(),
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
    kind_with_history::<be_block::TextContent>(),
    kind_with_history::<be_block::VideoContent>(),
    kind::<be_block::EditorSettingsContent>(),
    kind::<be_block::InputSettingsContent>(),
    kind::<be_block::InputRemapperContent>(),
    kind::<be_block::DisplaySettingsContent>(),
    kind::<be_block::BrowserTabContent>(),
    kind_with_history::<be_block::SettingsContent>(),
    kind_with_history::<be_block::SettingsProfileContent>(),
    kind::<be_block::FolderContent>(),
    kind::<be_block::WorkspaceContent>(),
    kind_with_history::<be_block::PixelRayTracerContent>(),
    kind::<be_block::EditorViewContent>(),
    kind::<be_block::PanZoomContent>(),
    kind::<be_block::Scene3dContent>(),
    kind::<be_block::TriangleContent>(),
    kind::<be_block::RepositoryContent>(),
    kind::<be_block::CheckoutContent>(),
];

fn kind_of(content_type: Uuid) -> Option<&'static Kind> {
    KINDS.iter().find(|kind| kind.content_type == content_type)
}

pub(crate) fn describe_of(content: &Content) -> Option<be_block::Entries> {
    match &content.derived {
        Some(derived) => Some(derived.clone()),
        None => (kind_of(content.content_type)?.describe)(&content.bytes),
    }
}

pub(crate) fn derived_for(content_type: Uuid, bytes: &[u8]) -> Option<be_block::Entries> {
    (kind_of(content_type)?.describe)(bytes)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Stale {
    Unreadable(String),
    Reshaped,
}

pub(crate) fn locked(block: Uuid) -> bool {
    with_shared(|shared| {
        shared.locked.contains(&block) || shared.graph.get(block).is_some_and(|node| node.locked)
    })
    .unwrap_or_default()
}

pub(crate) fn staleness(block: Uuid) -> Option<Stale> {
    let guard = stack().lock().unwrap_or_else(PoisonError::into_inner);
    let stack = guard.as_ref()?;
    if stack.tried.contains(&block) {
        return None;
    }
    let shared = stack.shared.lock().unwrap_or_else(PoisonError::into_inner);
    let node = shared.graph.get(block)?;
    let kind = kind_of(node.content_type)?;
    if kind.frozen {
        return None;
    }
    if let Some(error) = shared.unreadable.get(&block) {
        return Some(Stale::Unreadable(error.clone()));
    }
    let stamped = node.metadata.get(&be_block::SCHEMA)?;
    (stamped != (kind.schema)()).then_some(Stale::Reshaped)
}

pub(crate) fn reset(block: Uuid) {
    let Some(node) = node(block) else {
        return;
    };
    let Some(kind) = kind_of(node.content_type) else {
        return;
    };
    if let Some(stack) = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        stack.tried.remove(&block);
    }
    replace(block, node.content_type, (kind.blank)());
}

pub(crate) fn try_anyway(block: Uuid) {
    if !matches!(staleness(block), Some(Stale::Reshaped)) {
        return;
    }
    if let Some(stack) = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        stack.tried.insert(block);
    }
    crate::compositor::changed();
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
    organization: Uuid,
    commands: Option<UnboundedSender<Command>>,
    held: std::collections::HashSet<Uuid>,
    versioned: std::collections::HashSet<Uuid>,
    tried: std::collections::HashSet<Uuid>,
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
    let organization = config.organization;
    let running = platform::spawn(config, receiver, Arc::clone(&shared), Arc::clone(&changed));
    *stack().lock().unwrap_or_else(PoisonError::into_inner) = Some(Stack {
        account,
        organization,
        commands: Some(commands),
        held: std::collections::HashSet::new(),
        versioned: std::collections::HashSet::new(),
        tried: std::collections::HashSet::new(),
        shared,
        changed,
        running,
    });
}

pub(crate) fn installed_for(account: Uuid, organization: Uuid) -> bool {
    stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .is_some_and(|stack| stack.account == account && stack.organization == organization)
}

pub(crate) fn stop() {
    let stack = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    drop(stack);
}

pub(crate) fn flush() {
    let (done, sealed) = std::sync::mpsc::channel();
    send(Command::Flush(done));
    platform::await_flush(sealed);
}

pub(crate) fn open(block: Uuid, content_type: Uuid) {
    if let Some(source) = revisions::source(block) {
        revisions::load(block, source, content_type);
        return;
    }
    show_staged(block);
    send(Command::Open {
        block,
        content_type,
        owned: false,
    });
}

pub(crate) fn close(block: Uuid) {
    if revisions::source(block).is_some() {
        return;
    }
    let held = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
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
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
        .is_some_and(|stack| stack.held.insert(block));
    if fresh {
        show_staged(block);
        send(Command::Open {
            block,
            content_type,
            owned: true,
        });
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
        .unwrap_or_else(PoisonError::into_inner)
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

pub(crate) fn operate(block: Uuid, operation: Vec<u8>) {
    send(Command::Operate(block, None, operation));
}

pub(crate) fn operate_from(block: Uuid, origin: u64, operation: Vec<u8>) {
    send(Command::Operate(block, Some(origin), operation));
}

pub(crate) fn update_since(block: Uuid, origin: u64, sent: Option<u64>) -> Option<(u64, Update)> {
    with_shared(|shared| {
        let content = shared_content(shared, block)?;
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
    let to = Uuid::now_v7();
    let mut metadata = source.metadata.clone();
    metadata.remove(&be_block::ARTIFACT);
    let author = account().unwrap_or_default();
    with_shared_mut(|shared| {
        shared.graph.change(Node {
            id: to,
            content_type: source.content_type,
            author: Some(author),
            parent: Some(be_graph::BlockParent::Detached),
            access: be_graph::Access::Edit,
            references: source.references.clone(),
            metadata: metadata.clone(),
            head: None,
            locked: false,
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

fn show_staged(block: Uuid) {
    with_shared_mut(|shared| {
        if shared.blocks.contains_key(&block) {
            return;
        }
        let Some(content) = shared.staged.get(&block).and_then(|(content_type, bytes)| {
            kind_of(*content_type).and_then(|kind| (kind.preview)(bytes))
        }) else {
            return;
        };
        shared.blocks.insert(block, content);
        shared.previewed.insert(block);
        shared.touch(block);
    });
}

fn shared_content(shared: &Shared, block: Uuid) -> Option<&Content> {
    match shared.revision_previews.get(&block) {
        Some(preview)
            if !preview.released
                && shared
                    .graph
                    .get(preview.source)
                    .is_some_and(|node| node.access.can_view()) =>
        {
            preview.content.as_ref()
        }
        Some(_) => None,
        None => shared.blocks.get(&block),
    }
}

pub(crate) fn content(block: Uuid) -> Option<Content> {
    with_shared(|shared| shared_content(shared, block).cloned())?
}

pub(crate) fn content_revision(block: Uuid) -> Option<u64> {
    with_shared(|shared| shared_content(shared, block).map(|content| content.revision))?
}

pub(crate) fn describe_block(block: Uuid) -> Option<be_block::Entries> {
    with_shared(|shared| shared.blocks.get(&block).and_then(describe_of))?
}

pub(crate) fn pairing_requests() -> Vec<PairingRequest> {
    with_shared(|shared| shared.pairing.clone()).unwrap_or_default()
}

pub(crate) fn approve_pairing(from: u64, code: String) {
    send(Command::ApprovePairing { from, code });
}

pub(crate) fn dismiss_pairing(from: u64) {
    send(Command::DismissPairing(from));
}

pub(crate) fn contacts() -> Vec<be_client::Contact> {
    with_shared(|shared| shared.contacts.clone()).unwrap_or_default()
}

pub(crate) fn contacts_revision() -> u64 {
    with_shared(|shared| shared.contacts_revision).unwrap_or_default()
}

pub(crate) fn make_contact_link(
    offer: Option<(Uuid, be_graph::Access)>,
    reply: impl FnOnce(Result<String, String>) + Send + 'static,
) {
    if stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_none()
    {
        reply(Err("Open an organization first.".to_owned()));
        return;
    }
    send(Command::Contact(worker::ContactCommand::Link {
        offer,
        reply: Box::new(reply),
    }));
}

pub(crate) fn open_contact_link(
    code: be_keys::InviteCode,
    reply: impl FnOnce(Result<be_keys::ContactCard, String>) + Send + 'static,
) {
    send(Command::Contact(worker::ContactCommand::Open {
        code,
        reply: Box::new(reply),
    }));
}

pub(crate) fn accept_contact_link(
    code: be_keys::InviteCode,
    reply: impl FnOnce(Result<be_keys::ContactCard, String>) + Send + 'static,
) {
    send(Command::Contact(worker::ContactCommand::Accept {
        code,
        reply: Box::new(reply),
    }));
}

pub(crate) fn remove_contact(account: Uuid) {
    send(Command::Contact(worker::ContactCommand::Remove(account)));
}

pub(crate) fn escrow_warning() -> Option<&'static str> {
    with_shared(|shared| shared.escrow_warning).flatten()
}

pub(crate) fn status() -> Status {
    with_shared(|shared| Status {
        running: true,
        connected: shared.connected,
        blocks: shared.blocks.len(),
        wakes: shared.wakes,
        unsealed: shared.unsealed,
        conflicts: shared.conflicts.values().map(Vec::len).sum(),
        error: shared.error.clone(),
    })
    .unwrap_or_default()
}

fn send(command: Command) {
    let held = stack().lock().unwrap_or_else(PoisonError::into_inner);
    let Some(stack) = held.as_ref() else {
        return;
    };
    let Some(commands) = &stack.commands else {
        return;
    };
    stack
        .shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .pending += 1;
    if commands.send(command).is_err() {
        stack
            .shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pending -= 1;
    }
}

pub(crate) fn busy() -> bool {
    with_shared(|shared| {
        shared.pending > 0 || shared.uploading > 0 || shared.owing > 0 || !shared.graph.loaded
    })
    .unwrap_or_default()
}

pub(crate) fn seal_owed() {
    if with_shared(|shared| shared.owing > 0 && shared.pending == 0).unwrap_or_default() {
        let (done, _) = std::sync::mpsc::channel();
        send(Command::Flush(done));
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) fn wait_for<T>(
    timeout: std::time::Duration,
    read: impl Fn(&Shared) -> Option<T>,
) -> Option<T> {
    let (shared, changed) = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|stack| (Arc::clone(&stack.shared), Arc::clone(&stack.changed)))?;
    let deadline = std::time::Instant::now() + timeout;
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    loop {
        if let Some(value) = read(&held) {
            return Some(value);
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        held = changed
            .wait_timeout(held, remaining)
            .unwrap_or_else(PoisonError::into_inner)
            .0;
    }
}

fn with_shared_mut<T>(change: impl FnOnce(&mut Shared) -> T) -> Option<T> {
    let held = stack().lock().unwrap_or_else(PoisonError::into_inner);
    let stack = held.as_ref()?;
    let mut shared = stack.shared.lock().unwrap_or_else(PoisonError::into_inner);
    Some(change(&mut shared))
}

pub(crate) fn account() -> Option<Uuid> {
    stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|stack| stack.account)
}

pub(crate) fn identity() -> Option<(Uuid, Uuid)> {
    stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|stack| (stack.account, stack.organization))
}

pub(crate) fn stale_revision() -> u64 {
    with_shared(|shared| shared.stale_revision).unwrap_or_default()
}

pub(crate) fn graph_revision() -> u64 {
    with_shared(|shared| shared.graph.revision).unwrap_or_default()
}

pub(crate) fn take_touched() -> std::collections::HashSet<Uuid> {
    with_shared_mut(|shared| std::mem::take(&mut shared.touched)).unwrap_or_default()
}

pub(crate) fn graph_loaded() -> bool {
    with_shared(|shared| shared.graph.loaded).unwrap_or_default()
}

pub(crate) fn with_graph<T>(read: impl FnOnce(&Graph) -> T) -> Option<T> {
    with_shared(|shared| read(&shared.graph))
}

pub(crate) fn query(query: Query) -> Vec<Node> {
    if let Query::Block(id) = query
        && revisions::source(id).is_some()
    {
        return node(id).into_iter().collect();
    }
    let query = match query {
        Query::Children(id) => Query::Children(revisions::source(id).unwrap_or(id)),
        Query::References(id) => Query::References(revisions::source(id).unwrap_or(id)),
        Query::Backrefs(id) => Query::Backrefs(revisions::source(id).unwrap_or(id)),
        Query::Parents(id) => Query::Parents(revisions::source(id).unwrap_or(id)),
        other => other,
    };
    with_shared(|shared| shared.graph.query(query)).unwrap_or_default()
}

pub(crate) fn has_head(block: Uuid) -> bool {
    with_shared(|shared| {
        shared
            .graph
            .get(block)
            .is_some_and(|node| node.head.is_some())
    })
    .unwrap_or(false)
}

pub(crate) fn node(block: Uuid) -> Option<Node> {
    with_shared(|shared| {
        if let Some(preview) = shared.revision_previews.get(&block) {
            let mut node = shared.graph.get(preview.source)?.clone();
            node.id = block;
            node.access = if preview.released {
                be_graph::Access::None
            } else {
                node.access.min(be_graph::Access::View)
            };
            return Some(node);
        }
        shared.graph.get(block).cloned()
    })?
}

pub(crate) fn may_load(block: Uuid) -> bool {
    node(block).is_none_or(|node| node.access.can_view())
        && with_shared(|shared| !shared.unreadable.contains_key(&block)).unwrap_or_default()
}

pub(crate) fn access(block: Uuid) -> be_graph::Access {
    node(block).map_or(be_graph::Access::None, |node| node.access)
}

thread_local! {
    static PENDING_NAMES: std::cell::RefCell<std::collections::HashMap<Uuid, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub(crate) fn name_when_created(block: Uuid, name: String) {
    if node(block).is_some() {
        set_name(block, Some(name));
        return;
    }
    PENDING_NAMES.with(|pending| pending.borrow_mut().insert(block, name));
}

pub(crate) fn create(
    block: Uuid,
    content_type: Uuid,
    parent: be_graph::BlockParent,
    mut metadata: be_block::BlockMetadata,
    content: Option<Vec<u8>>,
) {
    if let Some(name) = PENDING_NAMES.with(|pending| pending.borrow_mut().remove(&block)) {
        metadata.insert(&be_block::NAME, &name);
    }
    if metadata.derived.is_empty()
        && let Some(kind) = kind_of(content_type)
    {
        let bytes = content.clone().unwrap_or_else(kind.blank);
        metadata.derived = (kind.describe)(&bytes).unwrap_or_default();
    }
    let author = account().unwrap_or_default();
    with_shared_mut(|shared| {
        shared.graph.change(Node {
            id: block,
            content_type,
            author: Some(author),
            parent: Some(parent),
            access: be_graph::Access::Edit,
            references: Vec::new(),
            metadata: metadata.clone(),
            head: None,
            locked: false,
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

pub(crate) fn placing(block: Uuid, placing: bool) {
    with_shared_mut(|shared| shared.graph.placing(block, placing));
}

pub(crate) fn set_parent(block: Uuid, parent: be_graph::BlockParent) {
    let node = node(block);
    if node
        .as_ref()
        .is_some_and(|node| node.parent == Some(parent))
    {
        return;
    }
    if let Some(node) = node
        && node.content_type
            != <be_block::EditorViewContent as be_block::BlockContent>::CONTENT_TYPE
        && node.access.can_edit()
    {
        let from = match (node.parent, parent) {
            (Some(be_graph::BlockParent::Root), be_graph::BlockParent::Detached) => {
                Some(be_block::DeletedFrom::Root)
            }
            (Some(be_graph::BlockParent::Block(old)), be_graph::BlockParent::Detached) => {
                Some(be_block::DeletedFrom::Block(old))
            }
            _ => None,
        };
        let restored = parent != be_graph::BlockParent::Detached;
        if from.is_some() || (restored && node.metadata.contains(&be_block::DELETED_FROM)) {
            define(
                block,
                vec![be_block::DELETED_FROM.definition(from.as_ref())],
            );
        }
    }
    with_shared_mut(|shared| {
        shared
            .graph
            .update(block, |node| node.parent = Some(parent))
    });
    send(Command::SetParent { block, parent });
}

pub(crate) fn define(block: Uuid, changes: Vec<be_block::Definition>) {
    let Some(metadata) = node(block).map(|node| node.metadata) else {
        return;
    };
    let mut next = metadata.clone();
    next.define(&changes);
    if next == metadata {
        return;
    }
    let derived = describe_block(block);
    with_shared_mut(|shared| {
        shared.graph.update(block, |node| node.metadata = next);
        if let Some(derived) = derived {
            shared.graph.derive(block, derived);
        }
    });
    send(Command::Define { block, changes });
}

pub(crate) fn set_name(block: Uuid, name: Option<String>) {
    define(block, vec![be_block::NAME.definition(name.as_ref())]);
}

pub(crate) fn describe_implicitly(block: Uuid, derived: be_block::Entries) {
    with_shared_mut(|shared| shared.graph.derive(block, derived));
}

pub(crate) fn set_access(block: Uuid, account: Uuid, access: be_graph::Access) {
    send(Command::SetAccess {
        block,
        account,
        access,
    });
}

pub(crate) fn leave(block: Uuid) {
    send(Command::Leave(block));
}

type MembersReply = Box<dyn FnOnce(Result<Vec<be_protocol::Member>, String>) + Send>;

pub(crate) fn list_members(reply: MembersReply) {
    send(Command::Members(worker::MembersCommand::List(reply)));
}

pub(crate) fn set_role(account: Uuid, role: be_protocol::OrganizationRole, reply: MembersReply) {
    send(Command::Members(worker::MembersCommand::SetRole {
        account,
        role,
        reply,
    }));
}

pub(crate) fn remove_member(account: Uuid, reply: MembersReply) {
    send(Command::Members(worker::MembersCommand::Remove {
        account,
        reply,
    }));
}

pub(crate) fn list_access(
    block: Uuid,
    reply: impl FnOnce(Result<Vec<be_protocol::AccessEntry>, String>) + Send + 'static,
) {
    send(Command::ListAccess {
        block,
        reply: Box::new(reply),
    });
}

type Exported = Result<(String, Vec<u8>), String>;

pub(crate) fn export(block: Uuid, reply: impl FnOnce(Exported) + Send + 'static) {
    let Some((name, blocks)) = with_shared(|shared| exported(&shared.graph, block)).flatten()
    else {
        reply(Err("This block is not loaded yet.".to_owned()));
        return;
    };
    send(Command::Export {
        blocks,
        reply: Box::new(move |encoded| reply(encoded.map(|bytes| (name, bytes)))),
    });
}

fn exported(graph: &Graph, block: Uuid) -> Option<(String, Vec<be_client::transfer::Saved>)> {
    use be_block::BlockContent;
    let picked = graph.get(block)?;
    let top = match picked
        .parent
        .and_then(be_graph::BlockParent::block)
        .and_then(|parent| graph.get(parent))
    {
        Some(view)
            if picked.content_type == be_block::DatabaseContent::CONTENT_TYPE
                && view.content_type == be_block::DatabaseViewContent::CONTENT_TYPE =>
        {
            view
        }
        _ => picked,
    };
    let saved = |node: &Node, parent| be_client::transfer::Saved {
        id: node.id,
        block_type: node.content_type,
        parent,
        metadata: node.metadata.clone(),
        references: node.references.clone(),
        content: None,
    };
    let mut blocks = vec![saved(top, None)];
    let mut next = 0;
    while next < blocks.len() {
        let parent = blocks[next].id;
        for child in graph.query(Query::Children(parent)) {
            if child.access.can_view() && blocks.iter().all(|held| held.id != child.id) {
                blocks.push(saved(&child, Some(next)));
            }
        }
        next += 1;
    }
    let name = top.metadata.name().unwrap_or_default();
    Some((be_client::transfer::file_name(&name), blocks))
}

pub(crate) fn import(bytes: &[u8], parent: be_graph::BlockParent) -> Result<(Uuid, Uuid), String> {
    let blocks = be_client::transfer::renumber(be_client::transfer::decode(bytes)?);
    let taken: std::collections::HashSet<String> = with_shared(|shared| {
        shared
            .graph
            .nodes()
            .filter_map(|node| node.metadata.get(&be_block::ROW_PREFIX))
            .filter(|prefix| !prefix.trim().is_empty())
            .collect()
    })
    .unwrap_or_default();
    let author = account().ok_or("Sign in to import blocks.")?;
    let top = (blocks[0].id, blocks[0].block_type);
    let mut imported = Vec::with_capacity(blocks.len());
    for mut block in blocks.iter().cloned() {
        if let Some(prefix) = block.metadata.get(&be_block::ROW_PREFIX)
            && taken.contains(&prefix)
            && let Some(bytes) = &block.content
            && let Ok(mut database) =
                <be_block::DatabaseContent as be_block::BlockContent>::decode(bytes)
        {
            database.apply(&be_block::database::Database::set_prefix(String::new()));
            let encoded = be_block::BlockContent::encode(&database);
            block.metadata.derived = derived_for(block.block_type, &encoded).unwrap_or_default();
            block.content = Some(encoded);
        }
        let placed = match block.parent {
            Some(position) => be_graph::BlockParent::Block(blocks[position].id),
            None => parent,
        };
        with_shared_mut(|shared| {
            shared.graph.change(Node {
                id: block.id,
                content_type: block.block_type,
                author: Some(author),
                parent: Some(placed),
                access: be_graph::Access::Edit,
                references: block.references.clone(),
                metadata: block.metadata.clone(),
                head: None,
                locked: false,
            });
        });
        if let Some(bytes) = &block.content {
            with_shared_mut(|shared| {
                shared
                    .staged
                    .insert(block.id, (block.block_type, bytes.clone()))
            });
        }
        imported.push((block, placed));
    }
    send(Command::Import(imported));
    Ok(top)
}

fn with_shared<T>(read: impl FnOnce(&Shared) -> T) -> Option<T> {
    let held = stack().lock().unwrap_or_else(PoisonError::into_inner);
    let stack = held.as_ref()?;
    let shared = stack.shared.lock().unwrap_or_else(PoisonError::into_inner);
    Some(read(&shared))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) use tests::Harness;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
