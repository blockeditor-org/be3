use std::sync::{
    Arc, Condvar, OnceLock, PoisonError,
    atomic::{AtomicUsize, Ordering},
};

use be_wasm_threads::Mutex;

use std::path::PathBuf;

use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use uuid::Uuid;

pub mod accounts;
pub mod app_graph;
pub mod authorship;
mod changes;
pub mod conflicts;
pub mod device_push;
pub mod discovery;
#[cfg(not(target_arch = "wasm32"))]
pub mod embedded_server;
mod graph;
pub mod invite_link;
pub mod keys;
pub mod link;
mod nudge;
pub mod offline_choice;
mod replicas;
pub mod revisions;
pub mod search;
mod seen;
pub mod terminal_host;
#[cfg(feature = "testing")]
pub mod testing;
pub mod types;
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
#[cfg(target_arch = "wasm32")]
pub use web::run_in_worker;

pub use changes::{ChangeWatch, Changes};
pub use graph::{Graph, Node, Owner, Query, Scope};
pub use search::{SearchAsk, SearchHit, Searched};
pub use seen::{Frame, Part, Seen};
pub use worker::{
    History, OFFLINE_LEAVING, OFFLINE_PAIRING, OFFLINE_SHARING, PairingRequest, Shared,
};

use seen::Edit;
use worker::Command;

fn refs(config: &Config) -> Result<be_block_client::RefsDb, String> {
    refs_in(&config.data_dir)
}

pub fn refs_in(root: &std::path::Path) -> Result<be_block_client::RefsDb, String> {
    static OPEN: Mutex<Vec<(std::path::PathBuf, be_block_client::RefsDb)>> = Mutex::new(Vec::new());
    let path = root.join("be-refs.sqlite3");
    let mut open = OPEN.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((_, refs)) = open.iter().find(|(held, _)| *held == path) {
        return Ok(refs.clone());
    }
    let refs = be_block_client::RefsDb::open(&path)
        .map_err(|error| format!("this device's record of what it holds cannot open: {error}"))?;
    open.push((path, refs.clone()));
    Ok(refs)
}

pub struct Config {
    pub server_url: String,
    pub token: String,
    pub account: Uuid,
    pub organization: Uuid,
    pub identity: [u8; 32],
    pub display_name: String,
    pub email: String,
    pub data_dir: PathBuf,
    pub clock: be_block_client::Clock,
}

impl Config {
    fn socket_url(&self) -> String {
        crate::accounts::socket_url(&self.server_url)
    }
}

#[derive(Clone)]
pub struct Content {
    pub content_type: Uuid,
    /// Behind an `Arc`, so copying a `Seen` to hand it over copies no open block's content.
    bytes: Arc<Vec<u8>>,
    pub revision: u64,
}

impl Content {
    pub fn new(content_type: Uuid, bytes: Vec<u8>) -> Self {
        Self {
            content_type,
            bytes: Arc::new(bytes),
            revision: 1,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Default)]
pub struct Status {
    pub running: bool,
    pub connected: bool,
    pub connecting: bool,
    pub blocks: usize,
    pub wakes: u64,
    pub unsaved: usize,
    pub kept: usize,
    pub saves: u64,
    pub conflicts: usize,
    pub conflicted: Vec<Uuid>,
    pub error: Option<String>,
    pub waiting: usize,
    pub syncing: bool,
    pub unsent: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsentEdit {
    pub block: Uuid,
    pub name: Option<String>,
    pub since: i64,
    pub offerable: bool,
    /// Why the server refused it, in words (`refusal`), when it did rather than access being
    /// lost (`UnsentBlock::refused`).
    pub refused: Option<&'static str>,
}

/// Why the server refused to make a block, in words for the person; its code goes to the log.
pub fn refusal(code: be_block_protocol::ErrorCode) -> &'static str {
    use be_block_protocol::ErrorCode;
    match code {
        ErrorCode::OverQuota => "it would go over a limit",
        ErrorCode::InvalidRequest => "it could not take it as it is",
        ErrorCode::PolicyRefused => "your organization does not allow it",
        ErrorCode::PermissionDenied | ErrorCode::BlockNotFound => "you may not make it there",
        _ => "it would not say why",
    }
}

pub use types::{host_block_types, is_known, kind_of, provided};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stale {
    Unreadable(String),
    Reshaped,
}

pub fn locked(block: Uuid) -> bool {
    with_seen(|seen| {
        seen.locked.contains(&block) || seen.graph.get(block).is_some_and(|node| node.locked)
    })
    .unwrap_or_default()
}

pub fn staleness(block: Uuid) -> Option<Stale> {
    let shared = {
        let mut guard = stack().lock().unwrap_or_else(PoisonError::into_inner);
        let stack = guard.as_mut()?;
        if stack.tried.contains(&block) {
            return None;
        }
        stack.view()
    };
    let node = shared.graph.get(block)?;
    let kind = kind_of(node.content_type)?;
    if kind.frozen {
        return None;
    }
    if let Some(error) = shared.unreadable.get(&block) {
        return Some(Stale::Unreadable(error.clone()));
    }
    let stamped = node.metadata.get(&be_block_content::SCHEMA)?;
    (stamped != kind.schema).then_some(Stale::Reshaped)
}

pub fn reset(block: Uuid) {
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
    replace(block, node.content_type, kind.blank);
}

pub fn try_anyway(block: Uuid) {
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
    be_host_base::host::changed();
}

/// The stack as the page holds it: only the page reads and writes it. What the worker does
/// reaches it as the `Seen` the worker hands over (`seen::Handover`), never through a lock the
/// worker takes.
struct Stack {
    account: Uuid,
    organization: Uuid,
    commands: Option<UnboundedSender<Command>>,
    nudges: Option<nudge::Nudges>,
    /// Dropped as the stack stops, which has the worker close its connection
    /// (`Inbox::stopped`).
    _stopping: tokio::sync::oneshot::Sender<()>,
    held: std::collections::HashSet<Uuid>,
    /// The blocks whose version status each owner shows (`want_versions`).
    versioned: std::collections::HashMap<Owner, Vec<Uuid>>,
    tried: std::collections::HashSet<Uuid>,
    /// Where the worker hands over each `Seen` it publishes.
    handover: Arc<seen::Handover>,
    /// What the page reads: the last `Seen` taken from `handover`, with the page's edits the
    /// worker had not applied by then laid over it again.
    view: Arc<Seen>,
    /// The page's edits the worker had not applied by `view`'s `Seen`, numbered.
    edits: Vec<(u64, Vec<Edit>)>,
    /// The number of the last edit.
    edited: u64,
    /// How many commands the page sent: those the worker has not finished are pending.
    sent: Arc<AtomicUsize>,
    /// Frames the host could not deliver yet (`keep_editor_frames`), by editor.
    kept_frames: std::collections::HashMap<u64, Vec<(Uuid, Arc<Vec<u8>>)>>,
    /// Whether an editor watches the account's storage (`watch_storage`).
    storage_watched: bool,
    /// The full-text searches each owner shows (`want_searches`).
    searches: std::collections::HashMap<Owner, Vec<SearchAsk>>,
    /// The worker's own state, for a test to wait on and reach into: the page never locks it.
    #[cfg_attr(not(any(test, feature = "testing")), allow(dead_code))]
    shared: Arc<Mutex<Shared>>,
    #[cfg_attr(not(any(test, feature = "testing")), allow(dead_code))]
    changed: Arc<Condvar>,
    running: platform::Running,
}

impl Stack {
    /// Takes the `Seen` the worker last handed over, if it has not been taken.
    fn catch_up(&mut self) {
        let Some(seen) = self.handover.take() else {
            return;
        };
        self.edits
            .retain(|(number, _)| *number > seen.edits_applied);
        self.view = seen;
        if !self.edits.is_empty() {
            // The page heard of these edits as it made them: laying them over a newer `Seen`
            // tells it nothing again.
            let view = Arc::make_mut(&mut self.view);
            changes::quietly(|| {
                for edit in self.edits.iter().flat_map(|(_, edits)| edits) {
                    edit.apply(view);
                }
            });
        }
    }

    fn view(&mut self) -> Arc<Seen> {
        self.catch_up();
        Arc::clone(&self.view)
    }

    /// Commands sent that the worker had not finished by the `Seen` read.
    fn pending(&self) -> usize {
        self.sent
            .load(Ordering::Acquire)
            .saturating_sub(self.view.done)
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        self.commands = None;
        self.nudges = None;
        self.running.stop();
    }
}

fn stack() -> &'static Mutex<Option<Stack>> {
    static STACK: OnceLock<Mutex<Option<Stack>>> = OnceLock::new();
    STACK.get_or_init(|| Mutex::new(None))
}

pub fn start(config: Config) {
    stop();
    let (commands, receiver) = unbounded_channel();
    let (nudges, nudged) = nudge::channel();
    let (stopping, stopped) = tokio::sync::oneshot::channel();
    let sent = Arc::new(AtomicUsize::new(0));
    let handover = Arc::new(seen::Handover::default());
    let shared = Shared {
        wall: worker::Wall::new(config.clock.clone()),
        sent: Arc::clone(&sent),
        ..Shared::default()
    };
    let seen = Arc::new(shared.seen.clone());
    let shared = Arc::new(Mutex::new(shared));
    let changed = Arc::new(Condvar::new());
    let account = config.account;
    let organization = config.organization;
    let running = platform::spawn(
        config,
        worker::Inbox {
            commands: receiver,
            nudged,
            handover: Arc::clone(&handover),
            held_clock: held_clock(),
            stopped,
        },
        Arc::clone(&shared),
        Arc::clone(&changed),
    );
    *stack().lock().unwrap_or_else(PoisonError::into_inner) = Some(Stack {
        account,
        organization,
        commands: Some(commands),
        nudges: Some(nudges),
        _stopping: stopping,
        held: std::collections::HashSet::new(),
        versioned: std::collections::HashMap::new(),
        tried: std::collections::HashSet::new(),
        handover,
        view: seen,
        edits: Vec::new(),
        edited: 0,
        sent,
        kept_frames: std::collections::HashMap::new(),
        storage_watched: false,
        searches: std::collections::HashMap::new(),
        shared,
        changed,
        running,
    });
    changes::everything();
}

/// The organization the stack runs in in this process, while it runs.
pub fn running_organization() -> Option<Uuid> {
    stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|stack| stack.organization)
}

pub fn installed_for(account: Uuid, organization: Uuid) -> bool {
    stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .is_some_and(|stack| stack.account == account && stack.organization == organization)
}

/// Does not wait on the server: closes the connection so whatever the worker waits on fails at
/// once, and the worker keeps what open replicas hold as local commits. Returns without waiting
/// for the worker; the next stack's worker waits for it (use `stop_and_wait` where the process
/// ends or its files go).
pub fn stop() {
    let stack = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    let stopped = stack.is_some();
    drop(stack);
    if stopped {
        changes::everything();
    }
}

/// Stops as a crash would: like `stop`, but the worker ends without keeping what open replicas
/// hold, so only what the device had already written survives.
#[cfg(any(test, feature = "testing"))]
pub fn crash() {
    if let Some(stack) = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
    {
        stack
            .shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .crashed = true;
    }
    stop();
}

pub fn stop_and_wait() {
    stop();
    platform::wait_for_stopped();
}

#[cfg(all(any(test, feature = "testing"), not(target_arch = "wasm32")))]
pub fn abandon() {
    let stack = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    if let Some(mut stack) = stack {
        stack.running.abandon();
    }
}

/// Cuts the reconnect wait short. The host calls it when its window gains focus; it is the hook
/// for network-change and foreground events.
pub fn retry_now() {
    if let Some(stack) = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        && let Some(nudges) = &stack.nudges
    {
        nudges.nudge();
    }
}

/// Seals what the open blocks owe and waits for the worker to answer, then returns what closing
/// now would lose, as the worker counted it when it answered: open blocks whose edits the
/// device's journal does not hold yet, commands sent before the flush that still wait their turn
/// (behind an upload or the server lane), and unfinished imports. Commands sent after the flush
/// do not count. When the worker does not answer in time, and on the web, which cannot wait, it
/// is `status().unsaved` as read before the flush. The close and switch guards ask while it is
/// above zero.
pub fn flush() -> usize {
    let unsaved = status().unsaved;
    let (done, sealed) = std::sync::mpsc::channel();
    send(Command::Flush(worker::FlushReply(done)));
    platform::await_flush(sealed).unwrap_or(unsaved)
}

#[cfg(any(test, feature = "testing"))]
pub fn open(block: Uuid, content_type: Uuid) {
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

#[cfg(any(test, feature = "testing"))]
pub fn close(block: Uuid) {
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

/// Keeps a block open for the host itself when its last editor closes (`close` leaves it
/// alone). Nothing releases it before the stack stops. A held block that is stale is reset
/// without asking, since the host needs it.
pub fn hold(block: Uuid, content_type: Uuid) {
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

pub fn change_child(
    block: Uuid,
    content_type: Uuid,
    change: be_block_content::ChildChange,
) -> Option<bool> {
    if !is_known(content_type) {
        return None;
    }
    if out_of_reach(block) {
        return Some(false);
    }
    send(Command::ChangeChild {
        block,
        content_type,
        change: api_child(change),
    });
    Some(true)
}

fn api_child(change: be_block_content::ChildChange) -> be_sdk_type_api::ChildChange {
    match change {
        be_block_content::ChildChange::Add(child) => be_sdk_type_api::ChildChange::Add(child),
        be_block_content::ChildChange::Delete(child) => be_sdk_type_api::ChildChange::Delete(child),
        be_block_content::ChildChange::Replace { old, new } => {
            be_sdk_type_api::ChildChange::Replace(be_sdk_type_api::ReplaceChild { old, new })
        }
        be_block_content::ChildChange::Reorder { block, after } => {
            be_sdk_type_api::ChildChange::Reorder(be_sdk_type_api::ReorderChild { block, after })
        }
    }
}

pub fn version(block: Uuid, command: be_sdk_api::VersionCommand) {
    send(Command::Version { block, command });
}

/// Names the versioned blocks (checkouts and repositories, with their content types) whose
/// status an owner shows: the stack computes each one's status while some owner wants it, and
/// lets go of what a status read once none does.
pub fn want_versions(owner: Owner, blocks: Vec<(Uuid, Uuid)>) {
    let (added, removed) = {
        let mut held = stack().lock().unwrap_or_else(PoisonError::into_inner);
        let Some(stack) = held.as_mut() else {
            return;
        };
        let union = |versioned: &std::collections::HashMap<Owner, Vec<Uuid>>| {
            versioned
                .values()
                .flatten()
                .copied()
                .collect::<std::collections::HashSet<Uuid>>()
        };
        let ids: Vec<Uuid> = blocks.iter().map(|(block, _)| *block).collect();
        if stack
            .versioned
            .get(&owner)
            .map_or(ids.is_empty(), |held| *held == ids)
        {
            return;
        }
        let before = union(&stack.versioned);
        match ids.is_empty() {
            true => stack.versioned.remove(&owner),
            false => stack.versioned.insert(owner, ids),
        };
        let after = union(&stack.versioned);
        (
            after.difference(&before).copied().collect::<Vec<_>>(),
            before.difference(&after).copied().collect::<Vec<_>>(),
        )
    };
    for (block, content_type) in blocks {
        if added.contains(&block) {
            hold(block, content_type);
            send(Command::WatchVersion(block));
        }
    }
    for block in removed {
        send(Command::UnwatchVersion(block));
    }
}

/// A block's version status, unless it is the one `sent` names; computed while an owner wants
/// it (`want_versions`).
pub fn version_since(block: Uuid, sent: Option<u64>) -> Option<(u64, be_sdk_api::VersionStatus)> {
    with_seen(|shared| {
        let state = shared.versions.get(&block)?;
        (sent != Some(state.revision)).then(|| (state.revision, state.status.clone()))
    })?
}

pub fn is_versioned(content_type: Uuid) -> bool {
    content_type == <be_block_repository::Checkout as be_block_content::Root>::CONTENT_TYPE
        || content_type == <be_block_repository::Repository as be_block_content::Root>::CONTENT_TYPE
}

pub fn operate(block: Uuid, operation: Vec<u8>) {
    send(Command::Operate(
        block,
        operation,
        be_host_base::host::clock(),
    ));
}

pub fn attach(block: Uuid, content_type: Uuid, editor: u64, editable: bool) {
    show_staged(block);
    send(Command::Attach {
        block,
        content_type,
        editor,
        editable,
    });
}

pub fn detach(block: Uuid, editor: u64) {
    send(Command::Detach { block, editor });
}

pub fn from_editor(block: Uuid, editor: u64, frame: &[u8]) {
    let Some(bytes) = replicas::unframe(frame) else {
        return;
    };
    send(Command::FromEditor {
        block,
        editor,
        bytes,
        at: held_clock().unwrap_or_else(be_host_base::host::clock),
    });
}

/// The modules' clock as the host holds it (`hold_clock`): only the page reads and writes it,
/// and a stack is handed it as it starts (`Inbox::held_clock`), then each change as a command.
static HELD_CLOCK: Mutex<Option<std::time::Instant>> = Mutex::new(None);

fn held_clock() -> Option<std::time::Instant> {
    *HELD_CLOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn hold_clock(at: Option<std::time::Instant>) {
    let mut held = HELD_CLOCK.lock().unwrap_or_else(PoisonError::into_inner);
    if *held == at {
        return;
    }
    *held = at;
    drop(held);
    send(Command::HoldClock(at));
}

/// The host's wall clock while a test pins it, `None` otherwise: commit times and the embedded
/// server's retention follow it (`be_block_commit::pin_now`), so a test moves history on with
/// its clock.
pub fn pin_wall_clock(at: Option<std::time::Duration>) {
    be_block_commit::pin_now(at.map(|at| i64::try_from(at.as_millis()).unwrap_or(i64::MAX)));
}

pub use replicas::unframe;

/// What the worker has for `editor` since the last take, after what the host kept back
/// (`keep_editor_frames`). The worker forgets them once it hears they were taken
/// (`Edit::TookFrames`).
pub fn take_editor_frames(editor: u64) -> Vec<(Uuid, Arc<Vec<u8>>)> {
    let (mut frames, upto) = {
        let mut held = stack().lock().unwrap_or_else(PoisonError::into_inner);
        let Some(stack) = held.as_mut() else {
            return Vec::new();
        };
        let kept = stack.kept_frames.remove(&editor).unwrap_or_default();
        let view = stack.view();
        let queued = view.editor_frames.get(&editor);
        (
            kept.into_iter()
                .chain(
                    queued
                        .into_iter()
                        .flatten()
                        .map(|frame| (frame.block, Arc::clone(&frame.bytes))),
                )
                .collect::<Vec<_>>(),
            queued.and_then(|queued| queued.last()).map(|frame| frame.number),
        )
    };
    if let Some(upto) = upto {
        edit(vec![Edit::TookFrames { editor, upto }]);
    }
    frames.shrink_to_fit();
    frames
}

/// Keeps frames the host took and could not deliver yet, for the next take.
pub fn keep_editor_frames(editor: u64, mut kept: Vec<(Uuid, Arc<Vec<u8>>)>) {
    if kept.is_empty() {
        return;
    }
    let mut held = stack().lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(stack) = held.as_mut() {
        let earlier = stack.kept_frames.entry(editor).or_default();
        kept.append(earlier);
        *earlier = kept;
    }
}

/// Waits until the worker has frames for `editor` the page has not taken, naming `what` for a
/// wait that fails (`wait_until`).
#[cfg(all(any(test, feature = "testing"), not(target_arch = "wasm32")))]
#[track_caller]
pub fn wait_for_frames(what: &str, editor: u64) {
    let taken = {
        let mut held = stack().lock().unwrap_or_else(PoisonError::into_inner);
        let stack = held.as_mut().expect("the stack is running");
        if stack.kept_frames.contains_key(&editor) {
            return;
        }
        let view = stack.view();
        if view.editor_frames.contains_key(&editor) {
            return;
        }
        stack
            .edits
            .iter()
            .flat_map(|(_, edits)| edits)
            .filter_map(|edit| match edit {
                Edit::TookFrames { editor: took, upto } if *took == editor => Some(*upto),
                _ => None,
            })
            .max()
    };
    wait_until(what, |shared| {
        shared
            .editor_frames
            .get(&editor)
            .and_then(|frames| frames.last())
            .is_some_and(|frame| taken.is_none_or(|taken| frame.number > taken))
            .then_some(())
    });
}

pub fn snapshot_frame(block: Uuid) -> Option<Vec<u8>> {
    with_seen(|shared| {
        let content = match shared.revision_previews.get(&block) {
            Some(preview) => preview.content.as_ref()?,
            None => shared
                .blocks
                .get(&block)
                .filter(|_| shared.previewed.contains(&block))?,
        };
        Some(replicas::snapshot_frame(
            content.content_type,
            content.bytes().to_vec(),
        ))
    })?
}

#[cfg(test)]
pub fn show(block: Uuid, kind: Uuid, value: Option<Vec<u8>>) {
    send(Command::Presence { block, kind, value });
}

#[cfg(test)]
fn hear_head(block: Uuid, head: be_block_commit::CommitId) {
    send(Command::HearHead { block, head });
}

#[cfg(test)]
pub fn send_to_peers(block: Uuid, to: Option<u64>, channel: Uuid, payload: Vec<u8>) {
    send(Command::SendToPeers {
        block,
        to,
        channel,
        payload,
    });
}

pub fn history(block: Uuid) -> History {
    with_seen(|shared| shared.histories.get(&block).copied())
        .flatten()
        .unwrap_or_default()
}

pub fn undo(block: Uuid) {
    send(Command::History { block, redo: false });
}

pub fn redo(block: Uuid) {
    send(Command::History { block, redo: true });
}

pub fn duplicate(from: Uuid) -> Option<(Uuid, Uuid)> {
    let source = node(from)?;
    if !is_known(source.content_type) {
        return None;
    }
    let to = Uuid::now_v7();
    let mut metadata = source.metadata.clone();
    metadata.remove(&be_block_content::ARTIFACT);
    let references = source.contents();
    let author = account().unwrap_or_default();
    edit(vec![Edit::Node(Node {
        id: to,
        content_type: source.content_type,
        author: Some(author),
        parent: Some(be_block_graph::BlockParent::Detached),
        access: be_block_graph::Access::Manage,
        references,
        metadata: metadata.clone(),
        head: None,
        locked: false,
        held: true,
        sets_access: false,
        size: 0,
        tree_size: 0,
    })]);
    send(Command::Duplicate {
        from,
        to,
        content_type: source.content_type,
        metadata,
    });
    Some((to, source.content_type))
}

pub fn replace(block: Uuid, content_type: Uuid, bytes: Vec<u8>) {
    send(Command::Replace {
        block,
        content_type,
        bytes,
    });
}

pub fn seed(block: Uuid, content_type: Uuid, bytes: Vec<u8>) {
    send(Command::Seed {
        block,
        content_type,
        bytes,
    });
}

fn show_staged(block: Uuid) {
    if with_seen(|seen| !seen.blocks.contains_key(&block) && seen.staged.contains_key(&block))
        .unwrap_or_default()
    {
        edit(vec![Edit::ShowStaged(block)]);
    }
}

pub fn content(block: Uuid) -> Option<Content> {
    with_seen(|seen| seen.content(block).cloned())?
}

pub fn describe_block(block: Uuid) -> Option<be_block_content::Entries> {
    with_seen(|shared| {
        shared
            .described
            .get(&block)
            .map(|description| description.derived.clone())
    })?
}

pub fn pairing_requests() -> Vec<PairingRequest> {
    with_seen(|shared| shared.pairing.clone()).unwrap_or_default()
}

/// Approves a device asking to pair; `reply` hears whether it was, or the refusal to show where
/// the person approved (offline, it needs the server).
pub fn approve_pairing(
    from: u64,
    code: String,
    reply: impl FnOnce(Result<(), String>) + Send + 'static,
) {
    send(Command::ApprovePairing {
        from,
        code,
        reply: seen::after_publish(reply),
    });
}

pub fn dismiss_pairing(from: u64) {
    send(Command::DismissPairing(from));
}

pub fn contacts() -> Vec<be_block_client::Contact> {
    with_seen(|shared| shared.contacts.to_vec()).unwrap_or_default()
}

/// The account's storage as the server last said, while `watch_storage` is on.
pub fn storage_usage() -> Option<be_block_protocol::AccountStorage> {
    with_seen(|shared| shared.storage.clone()).flatten()
}

/// Follows the account's storage while `on`, from a stream the server sends it on.
pub fn watch_storage(on: bool) {
    let changed = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
        .is_some_and(|stack| std::mem::replace(&mut stack.storage_watched, on) != on);
    if changed {
        send(Command::Storage(on));
    }
}

pub fn make_contact_link(
    offer: Option<(Uuid, be_block_graph::Access)>,
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
        reply: seen::after_publish(reply),
    }));
}

pub fn open_contact_link(
    code: be_block_keys::InviteCode,
    reply: impl FnOnce(Result<be_block_keys::ContactCard, String>) + Send + 'static,
) {
    send(Command::Contact(worker::ContactCommand::Open {
        code,
        reply: seen::after_publish(reply),
    }));
}

pub fn accept_contact_link(
    code: be_block_keys::InviteCode,
    reply: impl FnOnce(Result<be_block_keys::ContactCard, String>) + Send + 'static,
) {
    send(Command::Contact(worker::ContactCommand::Accept {
        code,
        reply: seen::after_publish(reply),
    }));
}

pub fn remove_contact(account: Uuid) {
    send(Command::Contact(worker::ContactCommand::Remove(account)));
}

pub fn escrow_warning() -> Option<&'static str> {
    with_seen(|shared| shared.escrow_warning).flatten()
}

pub fn status() -> Status {
    let Some((shared, pending)) = with_stack(|stack| (stack.view(), stack.pending())) else {
        return Status::default();
    };
    Status {
        running: true,
        connected: shared.connected,
        connecting: !shared.connected && !shared.reached,
        blocks: shared.blocks.len(),
        wakes: shared.wakes,
        unsaved: shared.undurable + pending + shared.importing,
        kept: shared.waiting,
        saves: shared.saves,
        conflicts: shared.conflicts.values().map(Vec::len).sum(),
        conflicted: shared.conflicts.keys().copied().collect(),
        error: shared.error.clone().or_else(|| shared.sync_error.clone()),
        waiting: shared.unsealed + shared.waiting,
        syncing: shared.draining || (shared.connected && shared.unsealed + shared.waiting > 0),
        unsent: shared.unsent.len(),
    }
}

pub fn unsent() -> Vec<UnsentEdit> {
    with_seen(|shared| {
        shared
            .unsent
            .iter()
            .map(|unsent| UnsentEdit {
                block: unsent.block,
                name: shared
                    .graph
                    .get(unsent.block)
                    .and_then(|node| node.metadata.name())
                    .or_else(|| unsent.name.clone()),
                since: unsent.since,
                offerable: unsent.offerable,
                refused: unsent.refused.map(refusal),
            })
            .collect()
    })
    .unwrap_or_default()
}

pub fn restore_unsent(block: Uuid) {
    send(Command::Unsent {
        block,
        restore: true,
    });
}

pub fn discard_unsent(block: Uuid) {
    send(Command::Unsent {
        block,
        restore: false,
    });
}

fn send(command: Command) {
    let held = stack().lock().unwrap_or_else(PoisonError::into_inner);
    let Some(stack) = held.as_ref() else {
        return;
    };
    let Some(commands) = &stack.commands else {
        return;
    };
    stack.sent.fetch_add(1, Ordering::AcqRel);
    if commands.send(command).is_err() {
        stack.sent.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Lays `edits` over what the page reads at once, and sends them to the worker, which applies
/// them as it takes them (`Command::Edit`). On the worker thread itself, applies them to its
/// `Seen` there.
fn edit(edits: Vec<Edit>) {
    if seen::here_mut(|shared| {
        for edit in &edits {
            for block in edit.apply(&mut shared.seen) {
                shared.touch(block);
            }
        }
    }) {
        return;
    }
    // Numbered and sent under the one lock, so edits reach the worker in the order numbered.
    let Some(touched) = with_stack(|stack| {
        stack.catch_up();
        stack.edited += 1;
        let view = Arc::make_mut(&mut stack.view);
        let touched: Vec<Uuid> = edits.iter().flat_map(|edit| edit.apply(view)).collect();
        stack.edits.push((stack.edited, edits.clone()));
        if let Some(commands) = &stack.commands {
            stack.sent.fetch_add(1, Ordering::AcqRel);
            if commands.send(Command::Edit(stack.edited, edits)).is_err() {
                stack.sent.fetch_sub(1, Ordering::AcqRel);
            }
        }
        touched
    }) else {
        return;
    };
    for block in touched {
        changes::block(block);
    }
}

/// Why the block store keeps a driver waiting (`Shared::busy`). `hurried` says whether the host
/// seals what its blocks owe on every frame (`seal_owed`), as it does for a hurried driven run:
/// only then is an owed seal work to wait for. Otherwise a type module seals after a quiet
/// spell, as for a person, which is a timer rather than work, and counts as out of step
/// (`unsynced`) instead.
pub fn busy(hurried: bool) -> Vec<String> {
    with_stack(|stack| {
        let pending = stack.pending();
        let view = stack.view();
        view.busy_with(hurried, pending, view.search_busy)
    })
    .unwrap_or_default()
}

/// Why the device is not in step with the server yet (`Shared::unsynced`): connected, nothing
/// waiting to be sent, and everything it missed since it connected asked for. The block store
/// not running is a reason too.
pub fn unsynced() -> Vec<String> {
    with_seen(Seen::unsynced)
        .unwrap_or_else(|| vec!["the block store is not running".to_owned()])
}

/// While a held clock keeps the type modules' timers from firing, when the soonest comes due
/// on it (`Shared::modules_due`).
pub fn modules_due() -> Option<std::time::Instant> {
    with_seen(|shared| shared.modules_due).flatten()
}

/// Holds this stack's outbox from draining while `hold`; letting go starts a drain.
#[cfg(test)]
pub(crate) fn hold_drains(hold: bool) {
    with_shared_mut(|shared| shared.hold_drains = hold);
    if !hold {
        send(Command::Sync);
    }
}

/// Holds this stack's next version command's write of a block (`version::HeldWrite`).
#[cfg(test)]
pub(crate) fn hold_next_version_write(held: version::HeldWrite) {
    with_shared_mut(|shared| shared.before_write = Some(held));
}

pub fn seal_owed() {
    if with_stack(|stack| stack.view().owing > 0 && stack.pending() == 0).unwrap_or_default() {
        send(Command::SealOwed);
    }
}

/// Flushes without waiting, and deliberately keeps the flush's late settle (unlike
/// `seal_owed`): edits made before the app is really suspended are sealed with it too.
pub fn backgrounded() {
    let (done, _) = std::sync::mpsc::channel();
    send(Command::Flush(worker::FlushReply(done)));
}

/// How long a test waits for a stack that makes no progress before failing: a wait that never
/// ends would hang the whole test target until the build server's timeout, with no word of which
/// test or wait. Progress (`Shared::progressed`) starts the limit again, so a machine too busy
/// to run the stack quickly is waited for, and a stack that stopped is not, however often its
/// worker wakes.
#[cfg(all(any(test, feature = "testing"), not(target_arch = "wasm32")))]
pub const WAIT_LIMIT: std::time::Duration = std::time::Duration::from_secs(60);

/// How long a wait may go on in all, however much progress the stack makes: one that keeps
/// changing and never gets there (a connection retried over and over) still has to end.
#[cfg(all(any(test, feature = "testing"), not(target_arch = "wasm32")))]
pub const WAIT_CAP: std::time::Duration = std::time::Duration::from_secs(300);

/// Waits until `read` finds what it wants in the stack's state, and panics at the caller after
/// `WAIT_LIMIT` without progress from the stack, or `WAIT_CAP` in all, naming `what` it waited
/// for ("opened the note").
#[cfg(all(any(test, feature = "testing"), not(target_arch = "wasm32")))]
#[track_caller]
pub fn wait_until<T>(what: &str, read: impl Fn(&Shared) -> Option<T>) -> T {
    let (shared, changed) = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|stack| (Arc::clone(&stack.shared), Arc::clone(&stack.changed)))
        .expect("the stack is running");
    let started = std::time::Instant::now();
    let mut deadline = started + WAIT_LIMIT;
    let handover = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|stack| Arc::clone(&stack.handover))
        .expect("the stack is running");
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    let mut seen = held.progressed;
    loop {
        if let Some(value) = read(&held) {
            // What the page reads has what was waited for, though the worker has not handed
            // it over yet.
            seen::publish(&mut held, &handover);
            return value;
        }
        // A driven clock (`Clock::driven`) never reaches the moment the stack waits on to try the
        // server again by itself, so the wait moves it there, as a real clock would have got
        // there: all of the stack's time moves on with it. The lock is let go first, for the
        // stack to run on.
        let reconnect = held
            .reconnect
            .clone()
            .filter(|(clock, at)| clock.is_driven() && *at > clock.now());
        if let Some((clock, at)) = reconnect {
            drop(held);
            clock.advance(at - clock.now());
            held = shared.lock().unwrap_or_else(PoisonError::into_inner);
            continue;
        }
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            let state = format!(
                "\n  connected: {}, reached the server: {}, error: {:?}, sync error: {:?}, waiting to reconnect: {}\n  \
                 commands not run: {}, edits owed: {} {:?} (sealed what was owed {} time(s)), asking the modules for {:?}, not yet durable: {}, draining the outbox: {}\n  \
                 listings loading: {}, listings failed: {}\n  \
                 progressed: {}, wakes: {}, passes in a row that never waited: {}, open blocks: {}, saves: {}, graph revision: {}\n  \
                 versions that failed: {:?}\n",
                held.connected,
                held.reached,
                held.error,
                held.sync_error,
                held.reconnect.is_some(),
                held.pending(),
                held.owing,
                held.owing_blocks
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                held.seals_owed,
                held.asking,
                held.undurable,
                held.draining,
                held.graph.fetching(),
                held.graph.failed_listings(),
                held.progressed,
                held.wakes,
                held.unwaited_passes,
                held.blocks.len(),
                held.saves,
                held.graph.revision,
                held.versions
                    .iter()
                    .filter_map(|(block, state)| Some((
                        block.to_string(),
                        state.status.error.as_ref()?
                    )))
                    .collect::<Vec<_>>()
            );
            drop(held);
            panic!(
                "the stack never reached {what}: it made no progress for {} s, or the wait took {} s in all ({state})",
                WAIT_LIMIT.as_secs(),
                WAIT_CAP.as_secs()
            );
        }
        held = changed
            .wait_timeout(held, left)
            .unwrap_or_else(PoisonError::into_inner)
            .0;
        if held.progressed != seen {
            seen = held.progressed;
            deadline = (std::time::Instant::now() + WAIT_LIMIT).min(started + WAIT_CAP);
        }
    }
}

/// Flushes, then waits until the server holds what the flush kept: `flush` returns once the device
/// keeps it, and the outbox sends it after. For a test that reads the server next.
#[cfg(all(any(test, feature = "testing"), not(target_arch = "wasm32")))]
#[track_caller]
pub fn flush_and_send() {
    flush();
    // A flush that kept anything has already set `draining`, and its command `recounting`, so
    // `unsynced` is not empty before the outbox has been read again.
    wait_until("the server holds what the flush kept", |shared| {
        shared.unsynced().is_empty().then_some(())
    });
}

#[cfg(all(any(test, feature = "testing"), not(target_arch = "wasm32")))]
pub fn fail_listings(query: Query, error: &str) {
    let query = sourced(query);
    with_shared_mut(|shared| {
        for listing in shared.graph.listings_of(query) {
            shared.graph.failed(listing, error.to_owned());
        }
    });
}

/// Queues `blocks` to be indexed again, read whole (`Search::reread`), and wakes the worker to
/// do it: a test's stand-in for an index that asks for text it lacks.
#[cfg(all(any(test, feature = "testing"), not(target_arch = "wasm32")))]
pub fn reread_search(blocks: impl IntoIterator<Item = Uuid>) {
    with_shared_mut(|shared| shared.search.reread(blocks));
    send(Command::Sync);
}

#[cfg(all(any(test, feature = "testing"), not(target_arch = "wasm32")))]
pub fn refetch(query: Query) {
    let query = sourced(query);
    with_shared_mut(|shared| {
        for listing in shared.graph.listings_of(query) {
            shared.graph.relist(listing);
        }
    });
    send(Command::Sync);
}

/// Changes the worker's own state from a test, and hands it over for the page to read.
#[cfg(any(test, feature = "testing"))]
fn with_shared_mut<T>(change: impl FnOnce(&mut Shared) -> T) -> Option<T> {
    let held = stack().lock().unwrap_or_else(PoisonError::into_inner);
    let stack = held.as_ref()?;
    let mut shared = stack.shared.lock().unwrap_or_else(PoisonError::into_inner);
    let changed = change(&mut shared);
    seen::publish(&mut shared, &stack.handover);
    Some(changed)
}

/// Reads the worker's own state from a test.
#[cfg(test)]
fn with_shared<T>(read: impl FnOnce(&Shared) -> T) -> Option<T> {
    let held = stack().lock().unwrap_or_else(PoisonError::into_inner);
    let stack = held.as_ref()?;
    let shared = stack.shared.lock().unwrap_or_else(PoisonError::into_inner);
    Some(read(&shared))
}

pub fn account() -> Option<Uuid> {
    stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|stack| stack.account)
}

pub fn identity() -> Option<(Uuid, Uuid)> {
    stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|stack| (stack.account, stack.organization))
}

/// The signed-in account's Unfiled in the open organization (`be_block::unfiled`), where
/// blocks made without a place the person chose go.
pub fn unfiled() -> Option<Uuid> {
    identity().map(|(account, organization)| be_block::unfiled(organization, account))
}

/// Whether `block` is one an app made, `app_id` by its `CREATED_BY`, that still sits where it
/// was made: directly in the signed-in account's own Unfiled, authored by that account, with
/// nothing below it. `CREATED_BY` alone proves nothing: anyone who may edit a block can set
/// it, and it stays on a block wherever it moves; only the account itself places blocks in its
/// Unfiled, and a block moved out of it or filed into is no longer the app's alone.
pub fn made_by_app_in_unfiled(block: Uuid, app_id: &str) -> bool {
    let Some((account, organization)) = identity() else {
        return false;
    };
    let unfiled = be_block::unfiled(organization, account);
    with_graph(|graph| {
        graph.get(block).is_some_and(|node| {
            node.parent == Some(be_block_graph::BlockParent::Block(unfiled))
                && node.author == Some(account)
                && node.metadata.get(&be_block_content::CREATED_BY).as_deref() == Some(app_id)
        }) && !graph.has_children(block)
    })
    .unwrap_or(false)
}

/// Whether the host is offline and holds no version of the block's content whole, so it can
/// neither open the block nor change it, a child added or moved in included
/// (`BlockInfo::out_of_reach`).
pub fn out_of_reach(block: Uuid) -> bool {
    let block = revisions::source(block).unwrap_or(block);
    with_seen(|seen| seen.out_of_reach.contains(&block)).unwrap_or_default()
}

pub fn ready(query: Query) -> bool {
    let query = sourced(query);
    with_seen(|shared| shared.graph.ready(query)).unwrap_or_default()
}

pub fn failure(query: Query) -> Option<String> {
    let query = sourced(query);
    with_seen(|shared| shared.graph.failure(query)).flatten()
}

pub fn want(owner: Owner, queries: Vec<Query>) {
    let queries: Vec<Query> = queries.into_iter().map(sourced).collect();
    if with_seen(|seen| seen.graph.wanted(owner) == queries.as_slice()).unwrap_or(true) {
        return;
    }
    edit(vec![Edit::Want(owner, queries)]);
    if with_seen(|seen| seen.graph.fetching()).unwrap_or_default() {
        send(Command::Sync);
    }
}

/// Names the full-text searches an owner shows (guides/blocks/host.md, "Search"); `searched`
/// answers each once it ran, and again after an index it read changed (`Changes::search`).
pub fn want_searches(owner: Owner, searches: Vec<SearchAsk>) {
    // Nothing asked, and nothing was: not a change, and callers say so every frame.
    let changed = with_stack(|stack| {
        if stack.searches.get(&owner) == Some(&searches)
            || (searches.is_empty() && !stack.searches.contains_key(&owner))
        {
            return false;
        }
        match searches.is_empty() {
            true => stack.searches.remove(&owner),
            false => stack.searches.insert(owner, searches.clone()),
        };
        true
    })
    .unwrap_or_default();
    if changed {
        send(Command::WantSearches(owner, searches));
    }
}

pub fn searched(search: &SearchAsk) -> Option<Searched> {
    with_seen(|seen| {
        let mut shown = seen.searched.get(search)?.clone();
        shown.complete = seen.search_indexed;
        Some(shown)
    })
    .flatten()
}

fn sourced(query: Query) -> Query {
    match query {
        Query::Children(id) => Query::Children(revisions::source(id).unwrap_or(id)),
        Query::References(id) => Query::References(revisions::source(id).unwrap_or(id)),
        Query::Backrefs(id) => Query::Backrefs(revisions::source(id).unwrap_or(id)),
        Query::Parents(id) => Query::Parents(revisions::source(id).unwrap_or(id)),
        Query::Subtree(id) => Query::Subtree(revisions::source(id).unwrap_or(id)),
        Query::Block(id) => Query::Block(revisions::source(id).unwrap_or(id)),
        Query::OpenSettings(id) => Query::OpenSettings(revisions::source(id).unwrap_or(id)),
        other => other,
    }
}

pub fn with_graph<T>(read: impl FnOnce(&Graph) -> T) -> Option<T> {
    with_seen(|shared| read(&shared.graph))
}

pub fn query(query: Query) -> Vec<Node> {
    if let Query::Block(id) = query
        && revisions::source(id).is_some()
    {
        return node(id).into_iter().collect();
    }
    let query = sourced(query);
    with_seen(|shared| shared.graph.query(query)).unwrap_or_default()
}

pub fn node(block: Uuid) -> Option<Node> {
    with_seen(|shared| {
        if let Some(preview) = shared.revision_previews.get(&block) {
            let mut node = shared.graph.get(preview.source)?.clone();
            node.id = block;
            node.access = if preview.released {
                be_block_graph::Access::None
            } else {
                node.access.min(be_block_graph::Access::View)
            };
            return Some(node);
        }
        shared.graph.get(block).cloned()
    })?
}

pub fn may_load(block: Uuid) -> bool {
    node(block).is_none_or(|node| node.access.can_view())
        && with_seen(|shared| !shared.unreadable.contains_key(&block)).unwrap_or_default()
}

pub fn access(block: Uuid) -> be_block_graph::Access {
    node(block).map_or(be_block_graph::Access::None, |node| node.access)
}

thread_local! {
    static PENDING_NAMES: std::cell::RefCell<std::collections::HashMap<Uuid, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn name_when_created(block: Uuid, name: String) {
    if node(block).is_some() {
        set_name(block, Some(name));
        return;
    }
    PENDING_NAMES.with(|pending| pending.borrow_mut().insert(block, name));
}

/// Makes a block; with `owner_only`, one only this account reads (`KindVisibility::Owner`).
pub fn create(
    block: Uuid,
    content_type: Uuid,
    parent: be_block_graph::BlockParent,
    mut metadata: be_block_content::BlockMetadata,
    content: Option<Vec<u8>>,
    owner_only: bool,
) {
    if let Some(name) = PENDING_NAMES.with(|pending| pending.borrow_mut().remove(&block)) {
        metadata.insert(&be_block_content::NAME, &name);
    }
    if metadata.derived.is_empty()
        && content.is_none()
        && let Some(kind) = kind_of(content_type)
    {
        metadata.derived = kind.described.derived;
    }
    let author = account().unwrap_or_default();
    edit(vec![Edit::Create(Node {
        id: block,
        content_type,
        author: Some(author),
        parent: Some(parent),
        access: be_block_graph::Access::Manage,
        references: Vec::new(),
        metadata: metadata.clone(),
        head: None,
        locked: false,
        held: true,
        sets_access: false,
        size: 0,
        tree_size: 0,
    })]);
    send(Command::Create {
        block,
        content_type,
        parent,
        metadata,
        bytes: content,
        owner_only,
    });
}

pub fn placing(block: Uuid, placing: bool) {
    edit(vec![Edit::Placing(block, placing)]);
}

pub fn set_parent(block: Uuid, parent: be_block_graph::BlockParent) {
    let node = node(block);
    if node
        .as_ref()
        .is_some_and(|node| node.parent == Some(parent))
    {
        return;
    }
    if let Some(node) = node
        && !node.metadata.contains(&be_block_content::VIEW_EDITOR)
        && node.access.can_edit()
    {
        let from = match (node.parent, parent) {
            (Some(be_block_graph::BlockParent::Root), be_block_graph::BlockParent::Detached) => {
                Some(be_block_content::DeletedFrom::Root)
            }
            (
                Some(be_block_graph::BlockParent::Block(old)),
                be_block_graph::BlockParent::Detached,
            ) => Some(be_block_content::DeletedFrom::Block(old)),
            _ => None,
        };
        let restored = parent != be_block_graph::BlockParent::Detached;
        if from.is_some() || (restored && node.metadata.contains(&be_block_content::DELETED_FROM)) {
            define(
                block,
                vec![be_block_content::DELETED_FROM.definition(from.as_ref())],
            );
        }
    }
    edit(vec![Edit::Parent(block, parent)]);
    send(Command::SetParent { block, parent });
}

pub fn define(block: Uuid, changes: Vec<be_block_content::Definition>) {
    let Some(metadata) = node(block).map(|node| node.metadata) else {
        return;
    };
    let mut next = metadata.clone();
    next.define(&changes);
    if next == metadata {
        return;
    }
    let derived = describe_block(block);
    edit(vec![Edit::Define {
        block,
        metadata: next,
        derived,
    }]);
    send(Command::Define { block, changes });
}

pub fn set_name(block: Uuid, name: Option<String>) {
    define(
        block,
        vec![be_block_content::NAME.definition(name.as_ref())],
    );
}

pub fn describe_implicitly(block: Uuid, derived: be_block_content::Entries) {
    edit(vec![Edit::Derive(block, derived)]);
}

/// Gives `account` `access` to `block`; `reply` hears whether the server did, or the refusal
/// to show where the person shared (offline, it needs the server).
pub fn set_access(
    block: Uuid,
    account: Uuid,
    access: be_block_graph::Access,
    reply: impl FnOnce(Result<(), String>) + Send + 'static,
) {
    send(Command::SetAccess {
        block,
        account,
        access,
        reply: seen::after_publish(reply),
    });
}

/// Every block whose notification mode the account set to other than `all`, once the server
/// has said: read when the host connects and again whenever another device changes one.
pub fn notify_modes() -> Option<Vec<(Uuid, be_block_protocol::NotifyMode)>> {
    with_seen(|shared| {
        shared
            .notify_modes
            .as_ref()
            .map(|modes| modes.iter().map(|(block, mode)| (*block, *mode)).collect())
    })
    .flatten()
}

/// Sets how the signed-in account is notified about `block`.
pub fn set_notify_mode(
    block: Uuid,
    mode: be_block_protocol::NotifyMode,
    reply: impl FnOnce(Result<(), String>) + Send + 'static,
) {
    send(Command::SetNotifyMode {
        block,
        mode,
        reply: seen::after_publish(move |set: Result<be_block_protocol::NotifyMode, String>| {
            reply(set.map(|_| ()));
        }),
    });
}

/// The server's VAPID public key, which a browser subscribes with.
pub fn push_key(reply: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static) {
    send(Command::Push(worker::PushCommand::Key(seen::after_publish(
        reply,
    ))));
}

/// Registers this device's push endpoint for the signed-in account.
pub fn register_push_endpoint(
    endpoint: be_block_protocol::PushEndpoint,
    reply: impl FnOnce(Result<(), String>) + Send + 'static,
) {
    send(Command::Push(worker::PushCommand::Register {
        endpoint,
        reply: seen::after_publish(reply),
    }));
}

pub fn unregister_push_endpoint(
    address: String,
    reply: impl FnOnce(Result<(), String>) + Send + 'static,
) {
    send(Command::Push(worker::PushCommand::Unregister {
        address,
        reply: seen::after_publish(reply),
    }));
}

/// Opens a push a device received (`Peer::open_notification`), with no network.
pub fn open_push(
    message: be_block_protocol::PushMessage,
    now: i64,
    reply: impl FnOnce(Result<be_block_client::OpenedNotification, String>) + Send + 'static,
) {
    send(Command::Push(worker::PushCommand::Open {
        message,
        now,
        reply: seen::after_publish(reply),
    }));
}

/// Leaves a block shared with this account; `reply` hears whether it did, or the refusal to show
/// where the person left (offline, it needs the server).
pub fn leave(block: Uuid, reply: impl FnOnce(Result<(), String>) + Send + 'static) {
    send(Command::Leave {
        block,
        reply: seen::after_publish(reply),
    });
}

type MembersReply = Box<dyn FnOnce(Result<Vec<be_block_protocol::Member>, String>) + Send>;

pub fn list_members(reply: MembersReply) {
    send(Command::Members(worker::MembersCommand::List(
        seen::after_publish(reply),
    )));
}

pub fn set_role(account: Uuid, role: be_block_protocol::OrganizationRole, reply: MembersReply) {
    send(Command::Members(worker::MembersCommand::SetRole {
        account,
        role,
        reply: seen::after_publish(reply),
    }));
}

pub fn remove_member(account: Uuid, reply: MembersReply) {
    send(Command::Members(worker::MembersCommand::Remove {
        account,
        reply: seen::after_publish(reply),
    }));
}

/// The person's own agents in the organization open: the members it made, bound to no block.
pub async fn agents() -> Result<Vec<be_block_protocol::Agent>, String> {
    let (reply, listed) = tokio::sync::oneshot::channel();
    list_members(Box::new(move |listed| {
        let _ = reply.send(listed);
    }));
    own_agents(listed.await.unwrap_or_else(|_| Err(NOT_OPEN.to_owned()))?)
}

/// Removes one of the person's own agents (`remove-member`); answers those left. Anything else
/// is refused here, before the server is asked: an administrator's `remove-member` would remove
/// any member.
pub async fn remove_agent(agent: Uuid) -> Result<Vec<be_block_protocol::Agent>, String> {
    let own = agents().await?;
    if own.iter().all(|listed| listed.account != agent) {
        return Err("That is not one of your agents.".to_owned());
    }
    let (reply, listed) = tokio::sync::oneshot::channel();
    remove_member(
        agent,
        Box::new(move |listed| {
            let _ = reply.send(listed);
        }),
    );
    own_agents(listed.await.unwrap_or_else(|_| Err(NOT_OPEN.to_owned()))?)
}

fn own_agents(
    members: Vec<be_block_protocol::Member>,
) -> Result<Vec<be_block_protocol::Agent>, String> {
    let me = account().ok_or_else(|| NOT_OPEN.to_owned())?;
    Ok(members
        .into_iter()
        .filter(|member| member.created_by == Some(me) && member.bound_to.is_none())
        .map(|member| be_block_protocol::Agent {
            account: member.account,
            name: member.display_name,
            email: member.email,
        })
        .collect())
}

pub async fn invite(
    email: String,
    role: be_block_protocol::OrganizationRole,
) -> Result<String, String> {
    let (reply, made) = tokio::sync::oneshot::channel();
    send(Command::Invite {
        email,
        role,
        reply: seen::after_publish(move |made| {
            let _ = reply.send(made);
        }),
    });
    made.await.unwrap_or_else(|_| Err(NOT_OPEN.to_owned()))
}

pub async fn create_agent(name: String) -> Result<crate::accounts::AgentCredentials, String> {
    let (reply, made) = tokio::sync::oneshot::channel();
    send(Command::CreateAgent {
        name,
        reply: seen::after_publish(move |made| {
            let _ = reply.send(made);
        }),
    });
    made.await.unwrap_or_else(|_| Err(NOT_OPEN.to_owned()))
}

/// An agent bound to `block` (an app's `create-block-agent`): its account and the credentials
/// `be` signs in as it with.
pub fn create_block_agent(
    block: Uuid,
    name: String,
    reply: impl FnOnce(Result<(Uuid, crate::accounts::AgentCredentials), String>) + Send + 'static,
) {
    send(Command::CreateBlockAgent {
        block,
        name,
        reply: seen::after_publish(reply),
    });
}

/// A new token for a block's agent; every older one stops signing it in.
pub fn rotate_agent_token(
    agent: Uuid,
    reply: impl FnOnce(Result<String, String>) + Send + 'static,
) {
    send(Command::BlockAgent {
        agent,
        change: worker::AgentChange::Rotate(seen::after_publish(reply)),
    });
}

pub fn remove_block_agent(agent: Uuid, reply: impl FnOnce(Result<(), String>) + Send + 'static) {
    send(Command::BlockAgent {
        agent,
        change: worker::AgentChange::Remove(seen::after_publish(reply)),
    });
}

const NOT_OPEN: &str = "Open the organization first.";

pub fn list_access(
    block: Uuid,
    reply: impl FnOnce(Result<be_block_protocol::AccessList, String>) + Send + 'static,
) {
    send(Command::ListAccess {
        block,
        reply: seen::after_publish(reply),
    });
}

pub fn read_head(
    block: Uuid,
    commit: be_block_commit::CommitId,
    reply: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) {
    send(Command::ReadHead {
        block,
        commit,
        reply: seen::after_publish(reply),
    });
}

type Exported = Result<(String, Vec<u8>), String>;

pub fn export(block: Uuid, reply: impl FnOnce(Exported) + Send + 'static) {
    send(Command::Export {
        block,
        reply: seen::after_publish(reply),
    });
}

pub fn plain_file(
    block: Uuid,
    reply: impl FnOnce(Result<be_block_type_host::PlainFile, String>) + Send + 'static,
) {
    send(Command::PlainFile {
        block,
        reply: seen::after_publish(reply),
    });
}

/// Replaces a block's content with a file's, as its type reads one. False, with nothing sent,
/// when the block is out of reach offline (`out_of_reach`): try again once the host reconnects.
#[must_use]
pub fn from_file(block: Uuid, content_type: Uuid, name: String, bytes: Vec<u8>) -> bool {
    if out_of_reach(block) {
        return false;
    }
    send(Command::FromFile {
        block,
        content_type,
        name,
        bytes,
    });
    true
}

/// Brings in the blocks of a block file under `parent`. `created_by` is the app that asked
/// (`CREATED_BY`), set on the top block; what the file says made each block is dropped.
pub fn import(
    bytes: &[u8],
    parent: be_block_graph::BlockParent,
    created_by: Option<&str>,
) -> Result<(Uuid, Uuid), String> {
    let mut blocks = be_block_client::transfer::renumber(be_block_client::transfer::decode(bytes)?);
    for block in &mut blocks {
        block.metadata.remove(&be_block_content::CREATED_BY);
        if block.parent.is_none() {
            block.metadata.put(
                &be_block_content::CREATED_BY,
                created_by.map(str::to_owned).as_ref(),
            );
        }
    }
    let author = account().ok_or("Sign in to import blocks.")?;
    let top = (blocks[0].id, blocks[0].block_type);
    let mut imported = Vec::with_capacity(blocks.len());
    let mut edits = Vec::with_capacity(blocks.len());
    for block in blocks.iter().cloned() {
        let placed = match block.parent {
            Some(position) => be_block_graph::BlockParent::Block(blocks[position].id),
            None => parent,
        };
        edits.push(Edit::Node(Node {
            id: block.id,
            content_type: block.block_type,
            author: Some(author),
            parent: Some(placed),
            access: be_block_graph::Access::Manage,
            references: block.references.clone(),
            metadata: block.metadata.clone(),
            head: None,
            locked: false,
            held: true,
            sets_access: false,
            size: 0,
            tree_size: 0,
        }));
        if let Some(bytes) = &block.content {
            edits.push(Edit::Stage {
                block: block.id,
                content_type: block.block_type,
                bytes: Arc::new(bytes.clone()),
            });
        }
        imported.push((block, placed));
    }
    edit(edits);
    send(Command::Import(imported));
    Ok(top)
}

/// Makes a block of the type that claims a file, under `parent`; `created_by` is the app that
/// asked (`CREATED_BY`).
pub fn import_file(
    name: String,
    bytes: Vec<u8>,
    parent: be_block_graph::BlockParent,
    created_by: Option<&str>,
    reply: impl FnOnce(Result<(Uuid, Uuid), String>) + Send + 'static,
) {
    if account().is_none() {
        reply(Err("Sign in to import files.".to_owned()));
        return;
    }
    let mut metadata = be_block_content::BlockMetadata::named(&name);
    metadata.put(
        &be_block_content::CREATED_BY,
        created_by.map(str::to_owned).as_ref(),
    );
    send(Command::ContentOfFile {
        name,
        bytes,
        reply: seen::after_publish(move |content: Result<(Uuid, Vec<u8>), String>| {
            reply(content.map(|(content_type, content)| {
                let block = Uuid::now_v7();
                create(block, content_type, parent, metadata, Some(content), false);
                (block, content_type)
            }));
        }),
    });
}

/// Reads what the page reads: the `Seen` the worker last handed over, with the page's own edits
/// laid over it. Never waits on the worker. On the worker thread itself, reads its `Seen` there.
fn with_seen<T>(read: impl FnOnce(&Seen) -> T) -> Option<T> {
    if seen::on_worker() {
        return seen::here(read);
    }
    let view = with_stack(Stack::view)?;
    Some(read(&view))
}

/// What the page reads now, for a test that checks what a call it made changed there before
/// the worker has handed that over.
#[cfg(any(test, feature = "testing"))]
pub fn read_seen<T>(read: impl FnOnce(&Seen) -> T) -> Option<T> {
    with_seen(read)
}

fn with_stack<T>(use_stack: impl FnOnce(&mut Stack) -> T) -> Option<T> {
    let mut held = stack().lock().unwrap_or_else(PoisonError::into_inner);
    Some(use_stack(held.as_mut()?))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
