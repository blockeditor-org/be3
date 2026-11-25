use std::sync::{
    Arc, OnceLock, PoisonError,
    atomic::{AtomicUsize, Ordering},
};

use std::sync::Mutex;

use std::path::PathBuf;

use uuid::Uuid;

pub mod accounts;
pub mod app_graph;
mod changes;
pub mod conflicts;
pub mod device_push;
mod diff;
pub mod discovery;
mod graph;
mod hosting;
pub mod invite_link;
mod joining;
pub mod keys;
pub mod link;
mod nudge;
pub mod offline_choice;
mod replicas;
mod replies;
pub mod revisions;
pub mod search;
mod seen;
pub mod terminal_host;
#[cfg(not(target_arch = "wasm32"))]
pub mod testing;
pub mod types;
pub mod unread;
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
pub use web::{Port, STACK_ENTRY, run_first, run_stack, start_in_worker};

pub use be_block_protocol::AccountName;
pub use changes::{ChangeWatch, Changes};
pub use graph::{Graph, Node, Owner, Query, Scope};
pub use search::{SearchAsk, SearchHit, Searched};
pub use seen::{Frame, Part, Seen};
pub use worker::{
    History, OFFLINE_LEAVING, OFFLINE_PAIRING, OFFLINE_SHARING, PairingRequest, SharePreview,
    Shared,
};

use seen::Edit;
use worker::Command;

fn refs(config: &Config) -> Result<be_block_client::RefsDb, String> {
    refs_in(&config.data_dir)
}

/// The refs database in `root`, which one process holds at a time: the hosts of this process
/// share it, and another process using `root` is refused (guides/blocks/host.md).
pub fn refs_in(root: &std::path::Path) -> Result<be_block_client::RefsDb, String> {
    static OPEN: Mutex<
        Vec<(
            std::path::PathBuf,
            be_block_client::RefsDb,
            Option<std::fs::File>,
        )>,
    > = Mutex::new(Vec::new());
    let path = root.join("be-refs.sqlite3");
    let mut open = OPEN.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((_, refs, _)) = open.iter().find(|(held, ..)| *held == path) {
        return Ok(refs.clone());
    }
    let lock = lock_data_dir(root)?;
    let refs = be_block_client::RefsDb::open(&path)
        .map_err(|error| format!("this device's record of what it holds cannot open: {error}"))?;
    open.push((path, refs.clone(), lock));
    Ok(refs)
}

/// Holds `root` for this process until it exits: two processes on one data directory would
/// each number edits under the block's one origin this device keeps (`refs::OpenEdits`).
#[cfg(not(target_arch = "wasm32"))]
fn lock_data_dir(root: &std::path::Path) -> Result<Option<std::fs::File>, String> {
    let path = root.join("be-refs.lock");
    std::fs::create_dir_all(root)
        .map_err(|error| format!("could not make {}: {error}", root.display()))?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|error| format!("could not open {}: {error}", path.display()))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Err(format!(
            "Another be-host is using this data directory ({})",
            root.display()
        )),
        Err(std::fs::TryLockError::Error(error)) => {
            Err(format!("could not lock {}: {error}", path.display()))
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn lock_data_dir(_: &std::path::Path) -> Result<Option<std::fs::File>, String> {
    Ok(None)
}

/// What a stack starts from. On the web it travels to the host's worker as bytes
/// (`web::spawn`), which carry no manual clock: only native tests hold one.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Config {
    /// A local account's, which has no server (guides/blocks/client.md, Saving and the outbox): its stack runs on a
    /// peer of its own (`Peer::local`) and `server_url` is unused. Said outright, not read
    /// from an empty `server_url`, so a remote account whose address went missing fails to
    /// connect rather than opening as an empty local one.
    pub local: bool,
    pub server_url: String,
    pub token: String,
    pub account: Uuid,
    pub organization: Uuid,
    pub identity: [u8; 32],
    pub display_name: String,
    pub email: String,
    pub data_dir: PathBuf,
    #[serde(skip)]
    pub clock: be_block_client::Clock,
}

impl Config {
    fn socket_url(&self) -> String {
        crate::accounts::socket_url(&self.server_url)
    }

    pub fn is_local(&self) -> bool {
        self.local
    }
}

#[derive(Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Content {
    pub content_type: Uuid,
    /// Behind an `Arc`, so copying a `Seen` to hand it over copies no open block's content.
    #[serde(with = "seen::bytes")]
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
    /// Open blocks with edits this device does not keep yet (neither a journal holds them nor
    /// its kept edits, `refs::OpenEdits`), plus commands sent and not yet carried out and
    /// unfinished imports. A kept edit survives a restart and is not counted, though the server
    /// may not hold it yet (guides/blocks/host.md).
    pub unsaved: usize,
    pub kept: usize,
    pub saves: u64,
    pub conflicts: usize,
    pub conflicted: Vec<Uuid>,
    pub error: Option<String>,
    pub waiting: usize,
    pub syncing: bool,
    pub unsent: usize,
    /// A local account's, with no server: saved once on this device, never offline.
    pub local: bool,
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
    /// Why it was set aside, which `refused` words: none when access to it was lost.
    pub aside: Option<be_block_client::Aside>,
    /// Edits kept on this device that never became a commit (`UnsentBlock::kept_edits`).
    pub kept_edits: usize,
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

/// Why edits were set aside among the unsent, in words for the person.
pub fn aside(why: be_block_client::Aside) -> &'static str {
    match why {
        be_block_client::Aside::Refused(code) => refusal(code),
        be_block_client::Aside::KeyOutdated => {
            "it was locked offline with a key someone who lost access may hold: discard it to go on editing"
        }
        be_block_client::Aside::Misplaced => {
            "it was locked offline under a folder it is not really in: discard it to go on editing"
        }
    }
}

pub use seen::KnownKind;
pub use types::{host_block_types, provided};

/// Whether the running stack's modules know `content_type`, as of what the page read
/// (`Seen::kinds`): the page asks the worker's modules nothing directly. So it answers false
/// from a stack's start, or restart, until the worker's first update lands, and what refuses
/// an unknown type (an app's new block, say) refuses it in that window too.
pub fn is_known(content_type: Uuid) -> bool {
    with_stack(|stack| stack.view.kinds.contains_key(&content_type)).unwrap_or(false)
}

/// What the page reads of `content_type` once a module of the running stack described it.
pub fn kind_of(content_type: Uuid) -> Option<KnownKind> {
    with_stack(|stack| stack.view.kinds.get(&content_type).cloned().flatten()).flatten()
}

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
    replace(block, node.content_type, kind.blank.to_vec());
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
    /// The account's own name as the server gave it at sign-in, which this device puts on the
    /// cards it hands its contacts (`Config::display_name`).
    name: String,
    commands: Option<worker::CommandPost>,
    nudges: Option<nudge::Nudges>,
    /// Dropped as the stack stops, which has the worker close its connection
    /// (`Inbox::stopped`).
    _stopping: tokio::sync::oneshot::Sender<()>,
    held: std::collections::HashSet<Uuid>,
    /// The blocks whose version status each owner shows (`want_versions`).
    versioned: std::collections::HashMap<Owner, Vec<Uuid>>,
    tried: std::collections::HashSet<Uuid>,
    /// Where the worker hands over what changed in its `Seen` (`seen::Update`).
    handover: Arc<seen::Handover>,
    /// The worker's `Seen` as of the last update taken from `handover`.
    base: Seen,
    /// What each reply the page is waiting on calls back, by its number (`replies::expect`).
    waiting: std::collections::HashMap<u64, replies::Waiting>,
    /// Updates taken from `handover` and decoded, not applied yet (`fetch`).
    fetched: Vec<seen::Handed>,
    /// What the page reads: `base`, with the page's edits the worker had not applied by then
    /// laid over it again.
    view: Arc<Seen>,
    /// The page's edits the worker had not applied by `view`'s `Seen`, numbered.
    edits: Vec<(u64, Vec<Edit>)>,
    /// The number of the last edit.
    edited: u64,
    /// How many commands the page sent: those the worker has not finished are pending.
    sent: Arc<AtomicUsize>,
    /// Frames the host could not deliver yet (`keep_editor_frames`), by editor.
    #[allow(clippy::type_complexity)] // spelled out where it is used; a name would add nothing
    kept_frames: std::collections::HashMap<u64, Vec<(Uuid, Arc<Vec<u8>>)>>,
    /// Whether an editor watches the account's storage (`watch_storage`).
    storage_watched: bool,
    /// The full-text searches each owner shows (`want_searches`).
    searches: std::collections::HashMap<Owner, Vec<SearchAsk>>,
    /// The block whose host channel each Join editor joins (`want_channel`).
    channels: std::collections::HashMap<Owner, Uuid>,
    /// Natively, the worker's own state, for a test to wait on and reach into, and for `stop` to
    /// close its connection: the page locks it for nothing else. On the web it is the worker's
    /// alone.
    #[cfg(not(target_arch = "wasm32"))]
    shared: Arc<Mutex<Shared>>,
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    changed: Arc<std::sync::Condvar>,
    running: platform::Running,
}

impl Stack {
    /// Applies the updates the worker handed over since the last take, and answers what to tell
    /// of them once the stack is let go of: watchers' changes, and the replies' callbacks with
    /// their answers.
    fn catch_up(&mut self) -> Taken {
        let mut taken = Taken::default();
        self.fetch();
        let handed = std::mem::take(&mut self.fetched);
        if handed.is_empty() {
            return taken;
        }
        for handed in handed {
            match handed.update {
                Some(seen::Update::Whole(seen)) => self.base = *seen,
                Some(seen::Update::Changed(delta)) => diff::Diff::apply(&mut self.base, delta),
                None => {}
            }
            taken.changes.merge(handed.changes);
            taken.notices.extend(handed.notices);
            for (id, answer) in handed.answers {
                if let Some(waiting) = self.waiting.remove(&id) {
                    taken.answers.push((waiting, answer));
                }
            }
            #[cfg(debug_assertions)]
            if let Some(mut check) = handed.check {
                // A moment travels as an offset from now, so the two copies read it back a
                // moment apart.
                if let (Some(checked), Some(made)) = (check.modules_due, self.base.modules_due)
                    && checked.max(made) - checked.min(made) < std::time::Duration::from_secs(1)
                {
                    check.modules_due = self.base.modules_due;
                }
                assert!(
                    self.base == check,
                    "what the page made of the worker's updates is not the worker's Seen"
                );
            }
        }
        self.edits
            .retain(|(number, _)| *number > self.base.edits_applied);
        let mut view = self.base.clone();
        #[cfg(target_arch = "wasm32")]
        if self.handover.gone() && view.error.is_none() {
            view.error =
                Some("be-host's worker stopped. Reload the page to start it again.".to_owned());
        }
        // The page heard of these edits as it made them: laying them over a newer `Seen` tells
        // it nothing again.
        changes::quietly(|| {
            for edit in self.edits.iter().flat_map(|(_, edits)| edits) {
                edit.apply(&mut view);
            }
        });
        self.view = Arc::new(view);
        taken
    }

    fn view(&mut self) -> Arc<Seen> {
        Arc::clone(&self.view)
    }

    /// Decodes the updates handed over since, to apply at the next take (`catch_up`).
    fn fetch(&mut self) {
        for bytes in self.handover.take() {
            // The worker encoded it from a `Handed`: bytes that do not decode are a bug.
            self.fetched
                .push(postcard::from_bytes(&bytes).expect("the worker's update decodes"));
        }
    }

    /// The frames the worker has for `editor` that the page has not taken, as of the newest
    /// update it handed over, though the page has not applied it yet: frames are messages to
    /// the editor, which go as soon as they arrive.
    fn frames_for(&mut self, editor: u64) -> Option<Vec<seen::Frame>> {
        self.fetch();
        let mut frames = self.view.editor_frames.get(&editor).cloned();
        for handed in &self.fetched {
            match &handed.update {
                Some(seen::Update::Whole(seen)) => {
                    frames = seen.editor_frames.get(&editor).cloned();
                }
                Some(seen::Update::Changed(delta)) => {
                    if let Some(changed) = &delta.editor_frames {
                        if changed.removed.contains(&editor) {
                            frames = None;
                        }
                        if let Some((_, put)) = changed.put.iter().find(|(id, _)| *id == editor) {
                            frames = Some(put.clone());
                        }
                    }
                }
                None => {}
            }
        }
        let took = self
            .edits
            .iter()
            .flat_map(|(_, edits)| edits)
            .filter_map(|edit| match edit {
                Edit::TookFrames { editor: took, upto } if *took == editor => Some(*upto),
                _ => None,
            })
            .max();
        frames
            .map(|frames| {
                frames
                    .into_iter()
                    .filter(|frame| took.is_none_or(|upto| frame.number > upto))
                    .collect::<Vec<_>>()
            })
            .filter(|frames| !frames.is_empty())
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

/// What taking the worker's updates has to tell, once the stack is let go of (`Stack::catch_up`).
#[derive(Default)]
struct Taken {
    changes: Changes,
    answers: Vec<(replies::Waiting, Option<Vec<u8>>)>,
    notices: Vec<seen::Notice>,
}

impl Taken {
    fn tell(self) {
        for notice in self.notices {
            notice.show();
        }
        changes::deliver(self.changes);
        for (waiting, answer) in self.answers {
            waiting(answer.as_deref());
        }
    }
}

/// Takes the worker's updates as a frame starts (`take_updates`), as be-host does in each.
pub fn take_updates_in_frame() {
    with_stack(|stack| stack.handover.frames_take_now());
    take_updates();
}

/// Applies what the worker handed over since the last take: what the page reads changes only
/// here, so every read between two takes reads one snapshot. be-host takes as each frame starts
/// (`take_updates_in_frame`), and on the web the page also takes between frames while a reply is
/// awaited outside one (`taking_updates`); then the watchers hear what changed and the replies' callbacks
/// run with their answers, which is when what they read has what the worker did before
/// answering. Answers that land after the stack stopped are lost with it: a callback for a
/// command sent just before a stop or a restart may not run.
///
/// Natively a take may race another thread's: a test that runs no frames has a thread take each
/// update as it lands (`start`). A take that finds another under way, one that took before it and
/// has not told what it took yet, returns only once that one has told, so whoever took returns
/// with the watchers having heard everything up to what it read. A take made while its own
/// thread tells (a callback that takes again) does not wait, so two threads never wait on each
/// other. So a callback the taking thread runs as it tells must not block on another thread that
/// takes: that take waits for the callback, and the two never finish.
pub fn take_updates() {
    let me = std::thread::current().id();
    let (taken, number) = {
        let mut held = stack().lock().unwrap_or_else(PoisonError::into_inner);
        (held.as_mut().map(Stack::catch_up), telling::began(me))
    };
    let told = telling::Told(number);
    if let Some(taken) = taken {
        taken.tell();
    }
    drop(told);
    #[cfg(not(target_arch = "wasm32"))]
    telling::await_earlier(me, number);
}

/// The takes whose updates are not told yet (`take_updates`), by the order they took in.
mod telling {
    use std::sync::{Condvar, Mutex, OnceLock, PoisonError};
    use std::thread::ThreadId;

    #[derive(Default)]
    struct Telling {
        next: u64,
        under_way: Vec<(u64, ThreadId)>,
        /// Takes waiting on an earlier one, for a test to see one wait.
        #[cfg(test)]
        waiting: usize,
    }

    fn telling() -> &'static (Mutex<Telling>, Condvar) {
        static TELLING: OnceLock<(Mutex<Telling>, Condvar)> = OnceLock::new();
        TELLING.get_or_init(Default::default)
    }

    /// A take by `thread` is under way; called under the stack's lock, so takes are numbered in
    /// the order they took.
    pub(super) fn began(thread: ThreadId) -> u64 {
        let mut held = telling().0.lock().unwrap_or_else(PoisonError::into_inner);
        let number = held.next;
        held.next += 1;
        held.under_way.push((number, thread));
        number
    }

    /// Its take has told what it took, or panicked telling it.
    pub(super) struct Told(pub(super) u64);

    impl Drop for Told {
        fn drop(&mut self) {
            let (lock, told) = telling();
            let mut held = lock.lock().unwrap_or_else(PoisonError::into_inner);
            held.under_way.retain(|(number, _)| *number != self.0);
            drop(held);
            told.notify_all();
        }
    }

    /// Waits until every take before `number` by another thread has told, unless `thread` is
    /// telling one of its own.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn await_earlier(thread: ThreadId, number: u64) {
        let (lock, told) = telling();
        let held = lock.lock().unwrap_or_else(PoisonError::into_inner);
        let waits = |held: &Telling| {
            !held.under_way.iter().any(|(_, by)| *by == thread)
                && held
                    .under_way
                    .iter()
                    .any(|(earlier, by)| *earlier < number && *by != thread)
        };
        if !waits(&held) {
            return;
        }
        #[cfg(test)]
        let held = {
            let mut held = held;
            held.waiting += 1;
            told.notify_all();
            held
        };
        let held = told
            .wait_while(held, |held| waits(held))
            .unwrap_or_else(PoisonError::into_inner);
        #[cfg(test)]
        let held = {
            let mut held = held;
            held.waiting -= 1;
            held
        };
        drop(held);
    }

    /// Waits until a take is waiting on an earlier one.
    #[cfg(test)]
    pub(crate) fn await_a_waiting_take() {
        let (lock, told) = telling();
        let held = lock.lock().unwrap_or_else(PoisonError::into_inner);
        drop(
            told.wait_while(held, |held| held.waiting == 0)
                .unwrap_or_else(PoisonError::into_inner),
        );
    }
}

/// Natively, waits until `answer` has what a reply's callback sends it or `patience` passes,
/// taking the worker's updates as they come, since the callback runs only as they are taken
/// (`take_updates`). Only what must block on the worker, such as closing, waits like this: the
/// web's page never does.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn wait_for_answer<T>(
    answer: &std::sync::mpsc::Receiver<T>,
    patience: std::time::Duration,
) -> Option<T> {
    let until = std::time::Instant::now() + patience;
    let handover = with_stack(|stack| Arc::clone(&stack.handover));
    loop {
        let puts = handover.as_ref().map(|handover| handover.puts());
        take_updates();
        if let Ok(answered) = answer.try_recv() {
            return Some(answered);
        }
        match (&handover, puts) {
            (Some(handover), Some(puts)) if handover.wait_past(puts, Some(until)) => {}
            _ => return answer.try_recv().ok(),
        }
    }
}

/// Awaits `answer`, which a reply's callback gives, while the worker's updates are taken as
/// they land: the callback runs only as they are taken (`take_updates`), and a page that runs no
/// frames (a hidden tab, which the browser gives no animation frames) takes none otherwise. What
/// answers the browser outside a frame, such as a push the service worker asks the page to
/// open, awaits its reply through this; the page never waits. However many await at once, one
/// task on the page takes the updates for them all (`pump`), the one that awaits the handover.
#[cfg(target_arch = "wasm32")]
pub async fn taking_updates<F: std::future::Future>(answer: F) -> F::Output {
    struct Awaiting;
    impl Drop for Awaiting {
        fn drop(&mut self) {
            AWAITING.set(AWAITING.get() - 1);
        }
    }
    AWAITING.set(AWAITING.get() + 1);
    let _awaiting = Awaiting;
    if !PUMPING.replace(true) {
        platform::spawn_local(pump());
    }
    answer.await
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    /// How many `taking_updates` await their answer.
    static AWAITING: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Whether the task that takes updates for them runs (`pump`).
    static PUMPING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Takes the worker's updates as each lands, while any `taking_updates` awaits its answer.
#[cfg(target_arch = "wasm32")]
async fn pump() {
    while AWAITING.get() > 0 {
        let Some(handover) = with_stack(|stack| Arc::clone(&stack.handover)) else {
            break;
        };
        let handed = handover.handed();
        take_updates();
        // A stack that stops closes its handover, which ends this wait too.
        handover.handed_past(handed).await;
    }
    PUMPING.set(false);
}

fn stack() -> &'static Mutex<Option<Stack>> {
    static STACK: OnceLock<Mutex<Option<Stack>>> = OnceLock::new();
    STACK.get_or_init(|| Mutex::new(None))
}

pub fn start(config: Config) {
    stop();
    let (commands, receiver) = worker::command_channel();
    let (nudges, nudged) = nudge::channel();
    let (stopping, stopped) = tokio::sync::oneshot::channel();
    let sent = Arc::new(AtomicUsize::new(0));
    let handover = Arc::new(seen::Handover::default());
    let account = config.account;
    let organization = config.organization;
    let name = config.display_name.clone();
    // A test that runs no frames has nothing to take the worker's updates in, so a thread takes
    // each as it lands, until a frame takes them (`take_updates_in_frame`): a test that runs
    // frames reads what they took, as the host does.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let pumped = Arc::clone(&handover);
        std::thread::spawn(move || {
            let mut puts = 0;
            while pumped.wait_past(puts, None) && !pumped.frames_take() {
                puts = pumped.puts();
                take_updates();
            }
        });
    }
    let start = worker::Start {
        config,
        held_clock: held_clock(),
        offline_choices: offline_choice::current(),
        reads: unread::current(),
        #[cfg(target_arch = "wasm32")]
        apps: discovery::fetched(),
    };
    let inbox = worker::Inbox {
        commands: receiver,
        nudged,
        handover: Arc::clone(&handover),
        stopped,
    };
    // Natively the worker is a thread of this process, which a test reaches into; on the web it
    // keeps its own `Shared`, and these are the page's ends of the channels to it (`web::spawn`).
    #[cfg(not(target_arch = "wasm32"))]
    let (running, shared, changed) = {
        let shared = Arc::new(Mutex::new(Shared {
            wall: worker::Wall::new(start.config.clock.clone()),
            sent: Arc::clone(&sent),
            ..Shared::default()
        }));
        let changed = Arc::new(std::sync::Condvar::new());
        let running = platform::spawn(start, inbox, Arc::clone(&shared), Arc::clone(&changed));
        (running, shared, changed)
    };
    #[cfg(target_arch = "wasm32")]
    let running = platform::spawn(start, inbox);
    *stack().lock().unwrap_or_else(PoisonError::into_inner) = Some(Stack {
        account,
        organization,
        name,
        commands: Some(commands),
        nudges: Some(nudges),
        _stopping: stopping,
        held: std::collections::HashSet::new(),
        versioned: std::collections::HashMap::new(),
        tried: std::collections::HashSet::new(),
        handover,
        base: Seen::default(),
        waiting: std::collections::HashMap::new(),
        fetched: Vec::new(),
        view: Arc::new(Seen::default()),
        edits: Vec::new(),
        edited: 0,
        sent,
        kept_frames: std::collections::HashMap::new(),
        storage_watched: false,
        searches: std::collections::HashMap::new(),
        channels: std::collections::HashMap::new(),
        #[cfg(not(target_arch = "wasm32"))]
        shared,
        #[cfg(not(target_arch = "wasm32"))]
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

/// Stops the stack. It first waits, as a flush does and no longer, for the server to order the
/// edits on their way to it, which closing the connection would lose. Then it closes the
/// connection so whatever the worker waits on fails at once, and the worker keeps what open
/// replicas hold as local commits. Returns without waiting for the worker; the next stack's
/// worker waits for it (use `stop_and_wait` where the process ends or its files go).
pub fn stop() {
    let running = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some();
    if running {
        flush();
    }
    stop_now();
}

fn stop_now() {
    // Numbered as a take is (`take_updates`), so a take meanwhile waits for what this tells.
    let (stack, number) = {
        let mut held = stack().lock().unwrap_or_else(PoisonError::into_inner);
        (held.take(), telling::began(std::thread::current().id()))
    };
    let told = telling::Told(number);
    let stopped = stack.is_some();
    if let Some(mut stack) = stack {
        let taken = stack.catch_up();
        stack.handover.close();
        // The connection is shared (`link::signed_in`): closed now rather than once the worker
        // hears it stopped, so whatever asks for one next (the organizations listed for the
        // next, the next stack) is handed a new one rather than this, closed under it.
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(connection) = stack
            .shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .connection
            .take()
        {
            connection.close();
        }
        let mut kept = OWN_INVITATIONS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for own in stack.view.own_invitations.iter() {
            if !kept.contains(own) {
                kept.push(own.clone());
            }
        }
        drop(kept);
        // What still waits on a reply hears none, as it would if the worker dropped it.
        drop(stack);
        taken.tell();
    }
    if stopped {
        changes::everything();
    }
    drop(told);
}

/// Stops as a crash would: like `stop`, but the worker ends without keeping what open replicas
/// hold, so only what the device had already written survives.
#[cfg(not(target_arch = "wasm32"))]
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
    stop_now();
}

pub fn stop_and_wait() {
    stop();
    platform::wait_for_stopped();
}

#[cfg(not(target_arch = "wasm32"))]
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
    send(Command::Flush(worker::FlushReply(replies::expect(
        move |unsaved| {
            let _ = done.send(unsaved);
        },
    ))));
    platform::await_flush(sealed).unwrap_or(unsaved)
}

/// Keeps `block`'s replica open with no editor on it, until `close`: what a host does for a
/// block it keeps hosting (be-host's `Hosts::keep`), so its session, and the lease, can come
/// back after a disconnect.
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

/// Lets go of a block `open` kept open: its replica closes once nothing else holds it.
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

/// What a child change (`change_child`) came to: none while the container's type has yet to
/// answer, then whether it made the change. Polled on a frame; the host is woken when it answers.
#[derive(Clone, Debug)]
pub struct ChildChanged(Arc<OnceLock<bool>>);

impl ChildChanged {
    /// A change refused before it was sent, such as one the container's manifest does not take.
    pub fn refused() -> Self {
        let refused = Self(Arc::default());
        let _ = refused.0.set(false);
        refused
    }

    pub fn answer(&self) -> Option<bool> {
        self.0.get().copied()
    }
}

/// The worker's end of a `ChildChanged`: a reply answered with whether the type made the change.
pub(crate) type ChildReply = replies::Reply<bool>;

/// Sets a `ChildChanged`'s answer on the page. One dropped unanswered (the stack stopped before
/// the type answered, or the command was never run) answers refused, so nothing waits on it for
/// good.
struct ChildAnswer(Arc<OnceLock<bool>>);

impl ChildAnswer {
    fn answer(self, made: bool) {
        let _ = self.0.set(made);
        be_host_base::host::wake();
    }
}

impl Drop for ChildAnswer {
    fn drop(&mut self) {
        if self.0.set(false).is_ok() {
            be_host_base::host::wake();
        }
    }
}

/// Adds, deletes, replaces or reorders a child in `block`'s content through its type, which
/// answers whether it made the change (`ChildChanged`). None while the type is not known yet; a
/// container this device cannot reach (`out_of_reach`) is refused at once.
pub fn change_child(
    block: Uuid,
    content_type: Uuid,
    change: be_block_content::ChildChange,
) -> Option<ChildChanged> {
    if !is_known(content_type) {
        return None;
    }
    if out_of_reach(block) {
        return Some(ChildChanged::refused());
    }
    Some(send_child_change(block, content_type, change, None))
}

fn send_child_change(
    block: Uuid,
    content_type: Uuid,
    change: be_block_content::ChildChange,
    copying: Option<Uuid>,
) -> ChildChanged {
    let answer = Arc::new(OnceLock::new());
    send(Command::ChangeChild {
        block,
        content_type,
        change,
        copying,
        reply: {
            let answer = ChildAnswer(Arc::clone(&answer));
            replies::expect(move |made: bool| answer.answer(made))
        },
    });
    ChildChanged(answer)
}

/// Asks `block`'s type to put a new copy of its child `source`, `copy` (a fresh id,
/// `Uuid::now_v7`), in the child's place (Unlink), answering what the type did. The copy is made
/// where the type's answer is heard, before the answer reaches the caller, so the container never
/// names a copy that is not made: one that cannot be made after all puts `source` back. The
/// caller picks `copy`, as for `duplicate`, so it can ask for it (`want`) before it is made: a
/// detached block nothing asks for leaves the graph as its creation settles. None while the type
/// is not known yet or the source cannot be copied (`duplicate`); a container out of reach is
/// refused.
pub fn replace_with_copy(
    block: Uuid,
    content_type: Uuid,
    source: Uuid,
    copy: Uuid,
) -> Option<ChildChanged> {
    if !is_known(content_type) || !node(source).is_some_and(|node| is_known(node.content_type)) {
        return None;
    }
    if out_of_reach(block) {
        return Some(ChildChanged::refused());
    }
    let change = be_block_content::ChildChange::Replace {
        old: source,
        new: copy,
    };
    Some(send_child_change(block, content_type, change, Some(source)))
}

/// Makes the copy `replace_with_copy` put in `block`, once its type did: whether it was made. One
/// that cannot be (the source left the graph meanwhile) gives the container its source back.
pub(crate) fn copied_in_place(block: Uuid, content_type: Uuid, source: Uuid, copy: Uuid) -> bool {
    if duplicate(source, copy).is_some() {
        return true;
    }
    let back = be_block_content::ChildChange::Replace {
        old: copy,
        new: source,
    };
    send_child_change(block, content_type, back, None);
    false
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

/// What the host component running on `block` has yet to hear of its host channel's members
/// (guides/blocks/hosted.md, "Hosting"), taken: the runtime passes it on as `member-event`s.
pub fn take_member_events(block: Uuid) -> Vec<be_sdk_api::blocks::MemberEvent> {
    let Some((events, upto)) = with_seen(|seen| {
        let (news, last) = seen.member_news.of(block)?;
        let events = news.into_iter().map(hosting::event_of).collect();
        Some((events, last))
    })
    .flatten() else {
        return Vec::new();
    };
    edit(vec![Edit::TookMemberNews { block, upto }]);
    events
}

/// What the Join editors of `block` have yet to hear of its host channel
/// (guides/blocks/hosted.md, "Hosting"), taken: the runtime passes it on as `channel-event`s.
pub fn take_channel_events(block: Uuid) -> Vec<be_sdk_api::blocks::ChannelEvent> {
    let Some((events, upto)) = with_seen(|seen| {
        let (news, last) = seen.channel_news.of(block)?;
        let events = news.into_iter().map(joining::event_of).collect();
        Some((events, last))
    })
    .flatten() else {
        return Vec::new();
    };
    edit(vec![Edit::TookChannelNews { block, upto }]);
    events
}

/// How many times the blocks whose lease this device holds changed: be-host looks again at
/// which host components should run only when this, or the editors open, moved.
pub fn lease_revision() -> u64 {
    with_seen(|seen| seen.leases_changed).unwrap_or_default()
}

/// A host component starts on `block`: it claims the session's lease, so this device hosts it,
/// and hears every member already admitted join, and none of what an earlier component left
/// untaken.
pub fn restart_host(block: Uuid) {
    // The worker drops the rest and replays the joins in one step; the page drops what it holds
    // now, so the new component takes none of it before that step reaches it. The replayed
    // joins are numbered after all of it.
    if let Some(upto) = with_seen(|seen| seen.member_news.of(block).map(|(_, last)| last)).flatten()
    {
        edit(vec![Edit::TookMemberNews { block, upto }]);
    }
    send(Command::RestartHost(block));
}

/// A host component stopped on `block`: if this device claimed its session since its replica
/// last joined it, and the replica stays open (a preview, a child), the replica joins the
/// session again, which drops the claim, so another device may host and this one is never
/// named host again by that claim once a later claimant goes (guides/blocks/hosted.md, "Who
/// hosts"). A replica that closes drops it anyway.
pub fn release_host(block: Uuid) {
    send(Command::ReleaseHost(block));
}

/// The blocks this device hosts now, by the server's word (`session-event.host` naming this
/// device): each has a component running here, or one that should not (a claim since let go),
/// which `release_host` drops.
pub fn hosted() -> std::collections::HashSet<Uuid> {
    with_seen(|seen| seen.hosted.iter().copied().collect()).unwrap_or_default()
}

/// Who hosts `block`'s session, by the last word the server sent: `None` while this device's
/// replica is not in it, else no one (`Some(None)`), this device (`Some(Some(true))`) or
/// another (`Some(Some(false))`).
pub fn session_host(block: Uuid) -> Option<Option<bool>> {
    with_seen(|seen| seen.session_hosts.get(&block).copied()).flatten()
}

/// Whether a host component may start on `block` here: this device's replica is in its session
/// and no one hosts it, or this device does, by the last word the server sent of who hosts.
/// Starting one claims the lease (`restart_host`).
pub fn may_host(block: Uuid) -> bool {
    with_seen(|seen| matches!(seen.session_hosts.get(&block), Some(None | Some(true))))
        .unwrap_or_default()
}

/// Names the block whose host channel `owner`, an editor opened as a Join member, joins
/// (`opening-kind.join`), or none: this device joins the session of each block some owner
/// names, and leaves it once none does.
pub fn want_channel(owner: Owner, block: Option<Uuid>) {
    let changed = with_stack(|stack| {
        let before = stack.channels.get(&owner).copied();
        if before == block {
            return None;
        }
        match block {
            Some(block) => stack.channels.insert(owner, block),
            None => stack.channels.remove(&owner),
        };
        Some(stack.channels.values().copied().collect())
    })
    .flatten();
    if let Some(blocks) = changed {
        send(Command::WantChannels(blocks));
    }
}

/// The account hosting `block`, while the channel to it of an editor here that joins it is
/// open.
pub fn channel_host(block: Uuid) -> Option<Uuid> {
    with_seen(|seen| seen.channel_hosts.get(&block).copied()).flatten()
}

/// A Join editor of `block` tells its host `payload`, sealed on this device's channel to it:
/// nothing while the channel is not open.
pub fn tell_host(block: Uuid, payload: Vec<u8>) {
    send(Command::TellHost { block, payload });
}

/// A host component of `block` tells `member` of its host channel `payload`, sealed on the
/// member's channel; nothing when it is no admitted member.
pub fn tell_member(block: Uuid, member: u64, payload: Vec<u8>) {
    send(Command::TellMember {
        block,
        said: be_sdk_api::blocks::MemberSaid { member, payload },
    });
}

/// What `app`'s editor sends its block's replica.
pub fn from_editor(block: Uuid, editor: u64, app: &str, frame: &[u8]) {
    let Some(bytes) = replicas::unframe(frame) else {
        return;
    };
    send(Command::FromEditor {
        block,
        editor,
        app: app.to_owned(),
        bytes,
        at: held_clock().unwrap_or_else(be_host_base::host::clock),
    });
}

/// The modules' clock as the host holds it (`hold_clock`): only the page reads and writes it,
/// and a stack is handed it as it starts (`worker::Start::held_clock`), then each change as a
/// command.
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
        let queued = stack.frames_for(editor);
        (
            kept.into_iter()
                .chain(
                    queued
                        .iter()
                        .flatten()
                        .map(|frame| (frame.block, Arc::clone(&frame.bytes))),
                )
                .collect::<Vec<_>>(),
            queued
                .as_ref()
                .and_then(|queued| queued.last())
                .map(|frame| frame.number),
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
#[cfg(not(target_arch = "wasm32"))]
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

/// Has the worker hear who hosts `block` as the server's session event would tell it, through
/// what it does with one (`hosting::heard`).
#[cfg(test)]
fn hear_host(block: Uuid, host: Option<be_block_protocol::SessionHost>, joined: bool) {
    send(Command::HearHost {
        block,
        host,
        joined,
    });
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

/// Copies `from` into a new block `to` (a fresh id, `Uuid::now_v7`), detached, and answers its
/// type; none when the source is not in the graph or its type is not known. The caller picks
/// `to`, so it can name the copy before making it (`replace_with_copy` asks the container
/// first).
pub fn duplicate(from: Uuid, to: Uuid) -> Option<Uuid> {
    let source = node(from)?;
    if !is_known(source.content_type) {
        return None;
    }
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
        references: Some(references),
        metadata: metadata.clone(),
        head: None,
        locked: false,
        held: true,
        sets_access: false,
        size: 0,
        tree_size: 0,
        // Its tree is the server's word, once the drain sends it.
        changed: 0,
        entry: None,
        unread: 0,
    })]);
    send(Command::Duplicate {
        from,
        to,
        content_type: source.content_type,
        metadata,
    });
    Some(source.content_type)
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
        reply: replies::expect(reply),
    });
}

pub fn dismiss_pairing(from: u64) {
    send(Command::DismissPairing(from));
}

pub fn contacts() -> Vec<be_block_client::Contact> {
    with_seen(|shared| shared.contacts.to_vec()).unwrap_or_default()
}

/// The contacts' records this device pinned, as the stack last read them.
pub fn contact_pins() -> std::collections::HashMap<Uuid, be_block_client::contacts::OwnContact> {
    with_seen(|shared| (*shared.contact_pins).clone()).unwrap_or_default()
}

/// The account's storage as the server last said, while `watch_storage` is on.
pub fn storage_usage() -> Option<be_block_protocol::AccountStorage> {
    with_seen(|shared| shared.storage).flatten()
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
        reply: replies::expect(reply),
    }));
}

pub fn open_contact_link(
    code: be_block_keys::InviteCode,
    reply: impl FnOnce(Result<be_block_keys::ContactCard, String>) + Send + 'static,
) {
    send(Command::Contact(worker::ContactCommand::Open {
        code,
        reply: replies::expect(reply),
    }));
}

pub fn accept_contact_link(
    code: be_block_keys::InviteCode,
    reply: impl FnOnce(Result<be_block_keys::ContactCard, String>) + Send + 'static,
) {
    send(Command::Contact(worker::ContactCommand::Accept {
        code,
        reply: replies::expect(reply),
    }));
}

/// Reads the share link with `code` before it is opened: the block it shares and its name, its
/// organization, and whether opening it joins that organization as a guest. None when no share
/// link has the code, as a contact link that carries none.
pub fn preview_share_link(
    code: be_block_keys::InviteCode,
    reply: impl FnOnce(Result<Option<worker::SharePreview>, String>) + Send + 'static,
) {
    send(Command::Contact(worker::ContactCommand::PreviewShare {
        code,
        reply: replies::expect(reply),
    }));
}

/// Opens the share link with `code` as the signed-in account.
pub fn open_share_link(
    code: be_block_keys::InviteCode,
    expected: (Uuid, be_block_graph::Access),
    reply: impl FnOnce(Result<worker::SharePreview, String>) + Send + 'static,
) {
    send(Command::Contact(worker::ContactCommand::OpenShare {
        code,
        expected,
        reply: replies::expect(reply),
    }));
}

pub fn remove_contact(account: Uuid) {
    send(Command::Contact(worker::ContactCommand::Remove(account)));
}

pub fn escrow_warning() -> Option<String> {
    with_seen(|shared| shared.escrow_warning.clone()).flatten()
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
        waiting: shared.unsealed() + shared.waiting,
        syncing: shared.draining || (shared.connected && shared.unsealed() + shared.waiting > 0),
        unsent: shared.unsent.len(),
        local: shared.local,
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
                refused: unsent.refused.map(aside),
                aside: unsent.refused,
                kept_edits: unsent.kept_edits,
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
        view.busy_with(hurried, pending, view.search_busy.as_deref())
    })
    .unwrap_or_default()
}

/// Why the device is not in step with the server yet (`Shared::unsynced`): connected, nothing
/// waiting to be sent, and everything it missed since it connected asked for. The block store
/// not running is a reason too.
pub fn unsynced() -> Vec<String> {
    with_seen(Seen::unsynced).unwrap_or_else(|| vec!["the block store is not running".to_owned()])
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
    send(Command::Flush(worker::FlushReply(replies::expect(
        |_: usize| {},
    ))));
}

/// How long a test waits for a stack that makes no progress before failing: a wait that never
/// ends would hang the whole test target until the build server's timeout, with no word of which
/// test or wait. Progress (`Shared::progressed`) starts the limit again, so a machine too busy
/// to run the stack quickly is waited for, and a stack that stopped is not, however often its
/// worker wakes.
#[cfg(not(target_arch = "wasm32"))]
pub const WAIT_LIMIT: std::time::Duration = std::time::Duration::from_secs(60);

/// How long a wait may go on in all, however much progress the stack makes: one that keeps
/// changing and never gets there (a connection retried over and over) still has to end.
#[cfg(not(target_arch = "wasm32"))]
pub const WAIT_CAP: std::time::Duration = std::time::Duration::from_secs(300);

/// Waits until `read` finds what it wants in the stack's state, and panics at the caller after
/// `WAIT_LIMIT` without progress from the stack, or `WAIT_CAP` in all, naming `what` it waited
/// for ("opened the note").
#[cfg(not(target_arch = "wasm32"))]
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
        // What `read` asks of the page (`is_known`, say) reads what the worker handed over:
        // hand over what it holds now and take it first, so the page is not behind it.
        if seen::publish(&mut held, &handover) {
            drop(held);
            take_updates();
            held = shared.lock().unwrap_or_else(PoisonError::into_inner);
        }
        if let Some(value) = read(&held) {
            // What the page reads has what was waited for, though the worker has not handed
            // it over yet.
            seen::publish(&mut held, &handover);
            drop(held);
            take_updates();
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

/// Takes the worker's updates and, unless `done` then holds, waits for the worker to hand over
/// more and takes those: one step of a test whose stack is taken by frames
/// (`take_updates_in_frame`) and that waits on what a reply's callback does. The callback runs
/// only as updates are taken, and a served command's answer makes none of the progress
/// `wait_until` waits on. Panics after `WAIT_LIMIT` without an update, naming `what`.
#[cfg(not(target_arch = "wasm32"))]
#[track_caller]
pub fn take_updates_or_wait(what: &str, done: impl Fn() -> bool) {
    let handover = with_stack(|stack| Arc::clone(&stack.handover)).expect("the stack is running");
    let puts = handover.puts();
    take_updates();
    if done() {
        return;
    }
    let until = std::time::Instant::now() + WAIT_LIMIT;
    assert!(
        handover.wait_past(puts, Some(until)),
        "the stack never handed over {what}"
    );
    take_updates();
}

/// Waits until what `block`'s host component has yet to take holds `ready`, then takes it. It
/// reads the page's own record, which drops what was taken at once, not the worker's, which
/// drops it only once the take reaches it: news taken a moment before would otherwise still
/// count there.
#[cfg(not(target_arch = "wasm32"))]
#[track_caller]
pub fn take_member_events_until(
    what: &str,
    block: Uuid,
    ready: impl Fn(&[be_sdk_api::blocks::MemberEvent]) -> bool,
) -> Vec<be_sdk_api::blocks::MemberEvent> {
    let held = || {
        with_seen(|seen| {
            let events: Vec<_> = seen
                .member_news
                .of(block)
                .map(|(news, _)| news.into_iter().map(hosting::event_of).collect())
                .unwrap_or_default();
            ready(&events)
        })
        .unwrap_or(false)
    };
    while !held() {
        take_updates_or_wait(what, held);
    }
    take_member_events(block)
}

/// Waits until what `block`'s Join editors have yet to hear holds `ready`, then takes it, read
/// from the page's own record as `take_member_events_until` reads.
#[cfg(not(target_arch = "wasm32"))]
#[track_caller]
pub fn take_channel_events_until(
    what: &str,
    block: Uuid,
    ready: impl Fn(&[be_sdk_api::blocks::ChannelEvent]) -> bool,
) -> Vec<be_sdk_api::blocks::ChannelEvent> {
    let held = || {
        with_seen(|seen| {
            let events: Vec<_> = seen
                .channel_news
                .of(block)
                .map(|(news, _)| news.into_iter().map(joining::event_of).collect())
                .unwrap_or_default();
            ready(&events)
        })
        .unwrap_or(false)
    };
    while !held() {
        take_updates_or_wait(what, held);
    }
    take_channel_events(block)
}

/// Flushes, then waits until the server holds what the flush kept: `flush` returns once the device
/// keeps it, and the outbox sends it after. For a test that reads the server next.
#[cfg(not(target_arch = "wasm32"))]
#[track_caller]
pub fn flush_and_send() {
    flush();
    // A flush that kept anything has already set `draining`, and its command `recounting`, so
    // `unsynced` is not empty before the outbox has been read again.
    wait_until("the server holds what the flush kept", |shared| {
        shared.unsynced().is_empty().then_some(())
    });
}

#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
pub fn reread_search(blocks: impl IntoIterator<Item = Uuid>) {
    with_shared_mut(|shared| shared.search.reread(blocks));
    send(Command::Sync);
}

#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
fn with_shared_mut<T>(change: impl FnOnce(&mut Shared) -> T) -> Option<T> {
    let held = stack().lock().unwrap_or_else(PoisonError::into_inner);
    let stack = held.as_ref()?;
    let mut shared = stack.shared.lock().unwrap_or_else(PoisonError::into_inner);
    let changed = change(&mut shared);
    seen::publish(&mut shared, &stack.handover);
    drop(shared);
    drop(held);
    take_updates();
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

/// The signed-in account and its own name: the display name the server gave as this device
/// signed in (`Config::display_name`), kept as it was then, which this device also puts on the
/// cards it hands its contacts. A name changed on another device since reaches it at the next
/// sign-in.
pub fn own_name() -> Option<(Uuid, String)> {
    stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|stack| (stack.account, stack.name.clone()))
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

/// What this account may do with `block`'s content now: the graph's access, but at most `View`
/// while its editors are attached read-only because its local edits were discarded
/// (`Seen::marked`). Editors are attached, and told whether they may edit, by this.
pub fn access(block: Uuid) -> be_block_graph::Access {
    let access = node(block).map_or(be_block_graph::Access::None, |node| node.access);
    match with_seen(|seen| seen.marked.contains(&block)).unwrap_or_default() {
        true => access.min(be_block_graph::Access::View),
        false => access,
    }
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
/// Whether the account is a guest of the organization open, which keeps there only its shell:
/// its settings, readable by it alone (decisions/a_guest_makes_only_its_own_shell.md).
static GUEST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Says whether the account is a guest of the organization open (`guest`).
pub fn set_guest(guest: bool) {
    GUEST.store(guest, std::sync::atomic::Ordering::Relaxed);
}

/// Whether the account is a guest of the organization open: it makes only its own settings,
/// owner-only, and what lies in them.
pub fn guest() -> bool {
    GUEST.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn create(
    block: Uuid,
    content_type: Uuid,
    parent: be_block_graph::BlockParent,
    mut metadata: be_block_content::BlockMetadata,
    content: Option<Vec<u8>>,
    owner_only: bool,
) {
    // Everything a guest makes lies in its own settings, which only it reads already: nothing
    // below them is owner-only of its own, which the server refuses.
    let owner_only =
        owner_only && !(guest() && matches!(parent, be_block_graph::BlockParent::Block(_)));
    if let Some(name) = PENDING_NAMES.with(|pending| pending.borrow_mut().remove(&block)) {
        metadata.insert(&be_block_content::NAME, &name);
    }
    if metadata.derived.is_empty()
        && content.is_none()
        && let Some(kind) = kind_of(content_type)
    {
        metadata.derived = kind.derived;
    }
    let author = account().unwrap_or_default();
    edit(vec![Edit::Create(Node {
        id: block,
        content_type,
        author: Some(author),
        parent: Some(parent),
        access: be_block_graph::Access::Manage,
        references: Some(Vec::new()),
        metadata: metadata.clone(),
        head: None,
        locked: false,
        held: true,
        sets_access: false,
        size: 0,
        tree_size: 0,
        // Its tree is the server's word, once the drain sends it.
        changed: 0,
        entry: None,
        unread: 0,
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
        reply: replies::expect(reply),
    });
}

/// Bars `account` from joining `block`'s host (guides/blocks/hosted.md, "Hosting"); `reply`
/// hears whether the server took it. A member of what this device hosts that it bars goes.
pub fn ban(block: Uuid, account: Uuid, reply: impl FnOnce(Result<(), String>) + Send + 'static) {
    send(Command::Ban {
        block,
        account,
        reply: replies::expect(reply),
    });
}

/// Offers ownership of `block`, which the account owns, to `account`, or withdraws the offer
/// when `account` is the signed-in one (`Peer::transfer`); `reply` hears whether the server took
/// it, or the refusal to show (offline, it needs the server).
pub fn transfer_ownership(
    block: Uuid,
    account: Uuid,
    reply: impl FnOnce(Result<(), String>) + Send + 'static,
) {
    send(Command::TransferOwnership {
        block,
        account,
        reply: replies::expect(reply),
    });
}

/// Accepts ownership of `block`, which its owner offered the account (`Peer::accept_ownership`);
/// `reply` hears as for `transfer_ownership`.
pub fn accept_ownership(block: Uuid, reply: impl FnOnce(Result<(), String>) + Send + 'static) {
    send(Command::AcceptOwnership {
        block,
        reply: replies::expect(reply),
    });
}

/// Every block whose notification mode the account set to other than `all`, once the server
/// has said: read when the host connects and again whenever another device changes one.
pub fn mutes() -> Option<Vec<(Uuid, be_block_protocol::NotifyMode)>> {
    with_seen(|shared| {
        shared
            .mutes
            .as_ref()
            .map(|modes| modes.iter().map(|(block, mode)| (*block, *mode)).collect())
    })
    .flatten()
}

/// Sets how the signed-in account is notified about `block`.
pub fn set_mute(
    block: Uuid,
    mode: be_block_protocol::NotifyMode,
    reply: impl FnOnce(Result<(), String>) + Send + 'static,
) {
    send(Command::SetMute {
        block,
        mode,
        reply: replies::expect(move |set: Result<be_block_protocol::NotifyMode, String>| {
            reply(set.map(|_| ()));
        }),
    });
}

/// The server's VAPID public key, which a browser subscribes with.
pub fn push_key(reply: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static) {
    send(Command::Push(worker::PushCommand::Key(replies::expect(
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
        reply: replies::expect(reply),
    }));
}

pub fn unregister_push_endpoint(
    address: String,
    reply: impl FnOnce(Result<(), String>) + Send + 'static,
) {
    send(Command::Push(worker::PushCommand::Unregister {
        address,
        reply: replies::expect(reply),
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
        reply: replies::expect(move |opened: Result<replies::Opened, String>| {
            reply(opened.map(|replies::Opened(opened)| opened));
        }),
    }));
}

/// Leaves a block shared with this account; `reply` hears whether it did, or the refusal to show
/// where the person left (offline, it needs the server).
pub fn leave(block: Uuid, reply: impl FnOnce(Result<(), String>) + Send + 'static) {
    send(Command::Leave {
        block,
        reply: replies::expect(reply),
    });
}

type MembersReply = Box<dyn FnOnce(Result<MemberList, String>) + Send>;

/// The organization's members, and those of them no record in its log signed in
/// (`Peer::unsigned_members`): every such member, to an administrator. A person signs in as they
/// accept an invitation, so one listed here is a member the server added on its own.
#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct MemberList {
    pub members: Vec<be_block_protocol::Member>,
    pub unsigned: Vec<Uuid>,
}

pub fn list_members(reply: MembersReply) {
    send(Command::Members(worker::MembersCommand::List(
        replies::expect(reply),
    )));
}

pub fn set_role(account: Uuid, role: be_block_protocol::OrganizationRole, reply: MembersReply) {
    send(Command::Members(worker::MembersCommand::SetRole {
        account,
        role,
        reply: replies::expect(reply),
    }));
}

pub fn remove_member(account: Uuid, reply: MembersReply) {
    send(Command::Members(worker::MembersCommand::Remove {
        account,
        reply: replies::expect(reply),
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

/// Removes one of the person's own agents (`delete-agent`); answers those left. Anything else
/// is refused here, before the server is asked: an administrator removes any member.
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

fn own_agents(listed: MemberList) -> Result<Vec<be_block_protocol::Agent>, String> {
    let me = account().ok_or_else(|| NOT_OPEN.to_owned())?;
    Ok(listed
        .members
        .into_iter()
        .filter(|member| member.created_by == Some(me) && member.bound_to.is_none())
        .map(|member| be_block_protocol::Agent {
            account: member.account,
            name: member.display_name,
            email: member.email,
        })
        .collect())
}

/// Makes an invitation into the organization open, as an editor, and answers its link.
pub async fn invite() -> Result<String, String> {
    let (reply, made) = tokio::sync::oneshot::channel();
    create_invitation(Box::new(move |made| {
        let _ = reply.send(made);
    }));
    made.await.unwrap_or_else(|_| Err(NOT_OPEN.to_owned()))
}

/// `invite`, answering through `reply`.
pub fn create_invitation(reply: Box<dyn FnOnce(Result<String, String>) + Send>) {
    send(Command::Invitations(worker::InvitationsCommand::Create(
        replies::expect(reply),
    )));
}

/// Invitation codes earlier stacks made or listed (`Seen::own_invitations`), kept by the page
/// as each stack stops: only the page reads and writes it. Opening one of these links as the
/// account whose own it is reads nothing from the server, so the inviter checking their own link
/// never marks it opened; another account on the device opens it as anyone would. An invitation
/// made or listed after `stop` took the stack's last `Seen` is lost with it, so a later stack
/// opens that link as someone else's would; that needs a link made as the stack stops.
static OWN_INVITATIONS: std::sync::Mutex<Vec<seen::OwnInvitation>> =
    std::sync::Mutex::new(Vec::new());

/// On the worker thread, notes invitation codes its account made or listed.
fn own_invitations(server_url: &str, account: Uuid, codes: Vec<be_block_keys::InviteCode>) {
    let noted = seen::here_mut(|shared| {
        for code in codes {
            let own = (server_url.to_owned(), account, code);
            if !shared.own_invitations.contains(&own) {
                shared.own_invitations.push(own);
            }
        }
    });
    debug_assert!(noted, "only the worker notes its own invitations");
}

/// Whether `code` is an invitation `account` on `server_url` made or listed as its own here.
pub fn is_own_invitation(
    server_url: &str,
    account: Uuid,
    code: &be_block_keys::InviteCode,
) -> bool {
    let own = |invitations: &[seen::OwnInvitation]| {
        invitations
            .iter()
            .any(|(server, owner, own)| server == server_url && *owner == account && own == code)
    };
    with_seen(|seen| own(&seen.own_invitations)).unwrap_or(false)
        || own(&OWN_INVITATIONS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))
}

/// An invitation in the organization's invite list.
#[derive(Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct Invite {
    pub id: Uuid,
    /// Who made it.
    pub inviter: String,
    /// Its link, for one the person made; the code is sealed to its inviter alone.
    pub link: Option<String>,
    pub state: be_block_protocol::InvitationState,
    /// Who accepted it, once accepted.
    pub accepter: String,
    /// In milliseconds since the epoch; `opened` and `ended` are 0 for not yet.
    pub created: i64,
    pub expires: i64,
    pub opened: i64,
    pub ended: i64,
}

/// Leaves out the link, a bearer secret, so no log ever holds it.
impl std::fmt::Debug for Invite {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Invite")
            .field("id", &self.id)
            .field("inviter", &self.inviter)
            .field("link", &self.link.as_ref().map(|_| ".."))
            .field("state", &self.state)
            .field("accepter", &self.accepter)
            .field("created", &self.created)
            .field("expires", &self.expires)
            .field("opened", &self.opened)
            .field("ended", &self.ended)
            .finish()
    }
}

type InvitationsReply = Box<dyn FnOnce(Result<Vec<Invite>, String>) + Send>;

/// The organization's invitations, newest first; an administrator's only.
pub fn list_invitations(reply: InvitationsReply) {
    send(Command::Invitations(worker::InvitationsCommand::List(
        replies::expect(reply),
    )));
}

/// Revokes an invitation still outstanding, and answers the list after.
pub fn revoke_invitation(link: Uuid, reply: InvitationsReply) {
    send(Command::Invitations(worker::InvitationsCommand::Revoke {
        link,
        reply: replies::expect(reply),
    }));
}

/// Makes an agent of the organization named `name`: its account and the credentials it signs in
/// with, which cannot be had again.
pub async fn create_agent(
    name: String,
) -> Result<(Uuid, crate::accounts::AgentCredentials), String> {
    let (reply, made) = tokio::sync::oneshot::channel();
    send(Command::CreateAgent {
        name,
        reply: replies::expect(move |made| {
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
        reply: replies::expect(reply),
    });
}

/// A new token for a block's agent; every older one stops signing it in.
pub fn rotate_agent_token(
    agent: Uuid,
    reply: impl FnOnce(Result<String, String>) + Send + 'static,
) {
    send(Command::BlockAgent {
        agent,
        change: worker::AgentChange::Rotate(replies::expect(reply)),
    });
}

pub fn remove_block_agent(agent: Uuid, reply: impl FnOnce(Result<(), String>) + Send + 'static) {
    send(Command::BlockAgent {
        agent,
        change: worker::AgentChange::Remove(replies::expect(reply)),
    });
}

const NOT_OPEN: &str = "Open the organization first.";

pub fn list_access(
    block: Uuid,
    reply: impl FnOnce(Result<be_block_protocol::AccessList, String>) + Send + 'static,
) {
    send(Command::ListAccess {
        block,
        reply: replies::expect(reply),
    });
}

/// The share links on `block`, each with the link to send when this account keeps its code.
pub fn list_share_links(
    block: Uuid,
    reply: impl FnOnce(Result<worker::ShareLinks, String>) + Send + 'static,
) {
    send(Command::ShareLink {
        block,
        change: worker::ShareLinkCommand::List(replies::expect(reply)),
    });
}

/// Makes a share link on `block` granting `access` to whoever `admits` lets open it; `reply`
/// hears the link to send.
pub fn make_share_link(
    block: Uuid,
    access: be_block_graph::Access,
    admits: be_block_keys::Admits,
    reply: impl FnOnce(Result<String, String>) + Send + 'static,
) {
    send(Command::ShareLink {
        block,
        change: worker::ShareLinkCommand::Make {
            access,
            admits,
            reply: replies::expect(reply),
        },
    });
}

/// Revokes share link `link` on `block`.
pub fn revoke_share_link(
    block: Uuid,
    link: Uuid,
    reply: impl FnOnce(Result<(), String>) + Send + 'static,
) {
    send(Command::ShareLink {
        block,
        change: worker::ShareLinkCommand::Revoke {
            link,
            reply: replies::expect(reply),
        },
    });
}

/// The accounts that may view `block`, and its author, with the names they gave the server
/// (`query.names`); an editor shows them through `app_graph::members_of`.
pub fn list_names(
    block: Uuid,
    reply: impl FnOnce(Result<Vec<be_block_protocol::AccountName>, String>) + Send + 'static,
) {
    send(Command::ListNames {
        block,
        reply: replies::expect(reply),
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
        reply: replies::expect(reply),
    });
}

type Exported = Result<(String, Vec<u8>), String>;

pub fn export(block: Uuid, reply: impl FnOnce(Exported) + Send + 'static) {
    send(Command::Export {
        block,
        reply: replies::expect(reply),
    });
}

pub fn plain_file(
    block: Uuid,
    reply: impl FnOnce(Result<be_block_type_host::PlainFile, String>) + Send + 'static,
) {
    send(Command::PlainFile {
        block,
        reply: replies::expect(reply),
    });
}

/// `content` of `content_type` as the plain file its type writes it as, named after `name` when
/// the file names nothing itself: what `plain_file` reads for a block that holds it.
pub fn file_of_content(
    content_type: Uuid,
    content: Vec<u8>,
    name: String,
    reply: impl FnOnce(Result<be_block_type_host::PlainFile, String>) + Send + 'static,
) {
    send(Command::FileOfContent {
        content_type,
        content,
        name,
        reply: replies::expect(reply),
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
            references: Some(block.references.clone()),
            metadata: block.metadata.clone(),
            head: None,
            locked: false,
            held: true,
            sets_access: false,
            size: 0,
            tree_size: 0,
            // Its tree is the server's word, once the drain sends it.
            changed: 0,
            entry: None,
            unread: 0,
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
        reply: replies::expect(move |content: Result<(Uuid, Vec<u8>), String>| {
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
#[cfg(not(target_arch = "wasm32"))]
pub fn read_seen<T>(read: impl FnOnce(&Seen) -> T) -> Option<T> {
    with_seen(read)
}

/// Sets the access the page and the worker see this account has to `block`, as a node edit,
/// for a test of what follows a change of access that the test's accounts cannot make (this
/// account's own access falling from View to Join, which only another account can lower).
#[cfg(not(target_arch = "wasm32"))]
pub fn set_access_seen(block: Uuid, access: be_block_graph::Access) {
    let Some(mut node) = with_seen(|seen| seen.graph.get(block).cloned()).flatten() else {
        return;
    };
    node.access = access;
    edit(vec![Edit::Node(node)]);
}

fn with_stack<T>(use_stack: impl FnOnce(&mut Stack) -> T) -> Option<T> {
    let mut held = stack().lock().unwrap_or_else(PoisonError::into_inner);
    Some(use_stack(held.as_mut()?))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
