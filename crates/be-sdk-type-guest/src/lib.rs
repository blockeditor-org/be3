use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashSet},
    future::{Future, ready},
    rc::Rc,
};

use be_block::{
    BlockContent, LiveEdit, Loads, Merge, MergeParts, PartReader, Undo, history::History,
};
use be_block_commit::MergeResult;
use be_sdk_type_api::{
    Change, ChildChange, ClientId, CommitId, Description, EditorId, Endpoint, FileChange, Journal,
    Loaded, Merged, MergedParts, Opening, PartRef, Plain, PlainFile, PublishOutcome, ReorderChild,
    ReplaceChild, ReplicaId, SearchDocument, SessionState, TypeInfo, Written,
    guest::{exports::replicas, host},
    ops::guest::{self as ops, Executor, Pending, Sender},
    types::Failure,
};
use uuid::Uuid;

pub mod editor;
mod effect;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod entropy;
mod file;
mod lacking;
mod record;
mod replica;
pub mod wire;

pub use be_sdk_type_api as api;
pub use be_wasm_guest_abi as abi;
pub use file::{BlobFile, DocumentFile, FileForm};
pub use record::{NoHistory, Recorder};
pub use replica::{
    APPEND_INTERVAL, COMMIT_INTERVAL, HEARTBEAT_INTERVAL, JOURNAL_BYTES, QUIET_INTERVAL,
};

use effect::{Answer, Effect, HeadEvent, RequestId};
use replica::{AnyReplica, Input, Opened, Replica};

type FromFileFn = fn(Option<&[u8]>, &str, Vec<u8>, u64) -> Option<Vec<u8>>;

type MergeFn = fn(Option<&[u8]>, Option<&[u8]>, Option<&[u8]>) -> Result<(Vec<u8>, usize), String>;

type MergePartsFn = fn(
    [Option<Loaded>; 3],
    Uuid,
    PartReader,
)
    -> std::pin::Pin<Box<dyn Future<Output = Result<(Written, usize), String>>>>;

type SearchTextFn = fn(
    Loaded,
    Option<Vec<String>>,
    Vec<be_block::PartRef>,
    PartReader,
) -> std::pin::Pin<
    Box<dyn Future<Output = Result<Vec<be_block::SearchDocument>, String>>>,
>;

type ChildFn = fn(Option<&[u8]>, ChildChange) -> Result<Option<Vec<u8>>, String>;

type ToFileFn = fn(&[u8]) -> Result<Option<PlainFile>, String>;

type ReplayFn = fn(Option<&[u8]>, &Journal) -> Result<Vec<u8>, String>;

pub struct Registration {
    info: TypeInfo,
    open: fn(Opened) -> Box<dyn AnyReplica>,
    plain: fn(Opened) -> Box<dyn AnyReplica>,
    blank: fn() -> Vec<u8>,
    merge: MergeFn,
    merge_parts: Option<MergePartsFn>,
    describe: fn(&[u8], Uuid) -> Result<Description, String>,
    search_text: SearchTextFn,
    replay: ReplayFn,
    child: ChildFn,
    to_file: Option<ToFileFn>,
    from_file: Option<FromFileFn>,
    claims: Option<fn(&str, &[u8]) -> bool>,
    claims_any_file: bool,
    claims_directories: bool,
}

impl Registration {
    pub fn of<C: LiveEdit + Merge + Clone + Default>() -> Self {
        Self {
            info: TypeInfo {
                content_type: C::CONTENT_TYPE,
                frozen: C::FROZEN,
                schema: be_block::schema_fingerprint::<C>(),
                undo: false,
                plain: Plain::None,
                first_part: C::LOADS == Loads::FirstPart,
                merges_parts: false,
            },
            open: open::<C, NoHistory>,
            plain: open::<C, NoHistory>,
            blank: || C::default().encode(),
            merge: merge::<C>,
            merge_parts: None,
            describe: describe_bytes::<C>,
            search_text: search_text::<C>,
            replay: replica::replay::<C>,
            child: child::<C>,
            to_file: None,
            from_file: None,
            claims: None,
            claims_any_file: false,
            claims_directories: false,
        }
    }

    pub fn undo<C: Undo + Merge + Clone + Default>(mut self) -> Self {
        assert_eq!(self.info.content_type, C::CONTENT_TYPE);
        self.info.undo = true;
        self.open = open::<C, History<C, u64>>;
        self
    }

    pub fn without_undo(mut self) -> Self {
        self.info.undo = false;
        self.open = self.plain;
        self
    }

    /// Merges by parts (`MergeParts`) outside a session; the content must load only its first
    /// part.
    pub fn merge_parts<C: MergeParts + Default>(mut self) -> Self {
        assert_eq!(self.info.content_type, C::CONTENT_TYPE);
        assert_eq!(C::LOADS, Loads::FirstPart);
        self.info.merges_parts = true;
        self.merge_parts = Some(merge_parts::<C>);
        self
    }

    pub fn file<C: FileForm + LiveEdit>(mut self) -> Self {
        assert_eq!(self.info.content_type, C::CONTENT_TYPE);
        self.info.plain = Plain::File;
        self.to_file = Some(file::to_file::<C>);
        self.from_file = Some(file::from_file::<C>);
        self.claims = Some(C::claims);
        self.claims_any_file = C::ANY_FILE;
        self
    }

    pub fn directory<C: LiveEdit>(mut self) -> Self {
        self = self.root::<C>();
        self.claims_directories = true;
        self
    }

    pub fn root<C: LiveEdit>(mut self) -> Self {
        assert_eq!(self.info.content_type, C::CONTENT_TYPE);
        self.info.plain = Plain::Directory;
        self
    }

    pub fn info(&self) -> &TypeInfo {
        &self.info
    }
}

fn open<C, H>(opened: Opened) -> Box<dyn AnyReplica>
where
    C: LiveEdit + Merge + Clone + Default,
    H: Recorder<C> + 'static,
{
    Box::new(Replica::<C, H>::new(opened))
}

fn merge<C: Merge + Default>(
    base: Option<&[u8]>,
    ours: Option<&[u8]>,
    theirs: Option<&[u8]>,
) -> Result<(Vec<u8>, usize), String> {
    let decode = |bytes: Option<&[u8]>| match bytes {
        Some(bytes) => C::decode(bytes).map_err(|error| error.to_string()),
        None => Ok(C::default()),
    };
    Ok(
        match C::merge3(&decode(base)?, &decode(ours)?, &decode(theirs)?) {
            MergeResult::Clean(value) => (value.encode(), 0),
            MergeResult::Conflicted { value, conflicts } => (value.encode(), conflicts.max(1)),
        },
    )
}

fn merge_parts<C: MergeParts + Default>(
    sides: [Option<Loaded>; 3],
    organization: Uuid,
    parts: PartReader,
) -> std::pin::Pin<Box<dyn Future<Output = Result<(Written, usize), String>>>> {
    Box::pin(async move {
        let [base, ours, theirs] = sides.map(|side| match side {
            Some(loaded) => {
                let stored = loaded
                    .into_stored()
                    .ok_or_else(|| "the host loaded parts that form no tree".to_owned())?;
                C::from_stored(&stored).map_err(|error| error.to_string())
            }
            None => Ok(C::default()),
        });
        let (value, conflicts) = match C::merge_parts(base?, ours?, theirs?, parts).await? {
            MergeResult::Clean(value) => (value, 0),
            MergeResult::Conflicted { value, conflicts } => (value, conflicts.max(1)),
        };
        Ok((written(&value, organization), conflicts))
    })
}

fn search_text<C: BlockContent>(
    content: Loaded,
    keys: Option<Vec<String>>,
    indexed: Vec<be_block::PartRef>,
    parts: PartReader,
) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<be_block::SearchDocument>, String>>>> {
    Box::pin(async move {
        let stored = content
            .into_stored()
            .ok_or_else(|| "the host loaded parts that form no tree".to_owned())?;
        let content = C::from_stored(&stored).map_err(|error| error.to_string())?;
        let keys: Option<HashSet<String>> = keys.map(|keys| keys.into_iter().collect());
        let indexed: HashSet<be_block::PartRef> = indexed.into_iter().collect();
        content.search_text(keys.as_ref(), &indexed, parts).await
    })
}

/// Content as a commit writes it: its parts, unchanged ones by their refs, and what it
/// references and derives.
pub(crate) fn written<C: BlockContent>(content: &C, organization: Uuid) -> Written {
    let (bytes, parts) = be_sdk_type_api::written_shape(&content.parts())
        .unwrap_or_else(|| (content.encode(), Vec::new()));
    Written {
        bytes,
        parts,
        references: content.references_in(organization),
        derived: be_block::derived_metadata(content),
    }
}

/// Reads parts with `read-part` under the stateless request being served, which the host
/// allows only for the parts of the `loaded` content that request was handed. Made while
/// the request's future runs, before its first await or after.
pub fn request_parts() -> PartReader {
    let request = ops::request();
    PartReader::new(move |part, offset, length| {
        let range = be_sdk_type_api::ByteRange { offset, length };
        let read = || host::read_part(part.into(), range);
        let pending = match request {
            Some(request) => ops::with_parent(request, read),
            None => read(),
        };
        Box::pin(async move {
            pending
                .await
                .map_err(|error| error.to_string())
                .and_then(|read| read)
        })
    })
}

fn child<C: LiveEdit + Default>(
    bytes: Option<&[u8]>,
    change: ChildChange,
) -> Result<Option<Vec<u8>>, String> {
    let mut content = match bytes {
        Some(bytes) => C::decode(bytes).map_err(|error| error.to_string())?,
        None => C::default(),
    };
    let Some(operations) = content.child_operations(be_child(change)) else {
        return Ok(None);
    };
    for operation in &operations {
        content.apply(operation);
    }
    Ok(Some(content.encode()))
}

pub(crate) fn be_child(change: ChildChange) -> be_block::ChildChange {
    match change {
        ChildChange::Add(block) => be_block::ChildChange::Add(block),
        ChildChange::Delete(block) => be_block::ChildChange::Delete(block),
        ChildChange::Replace(ReplaceChild { old, new }) => {
            be_block::ChildChange::Replace { old, new }
        }
        ChildChange::Reorder(ReorderChild { block, after }) => {
            be_block::ChildChange::Reorder { block, after }
        }
    }
}

pub fn describe<C: BlockContent>(content: &C, organization: Uuid) -> Description {
    Description {
        name: content.name(),
        derived: be_block::derived_metadata(content),
        references: content.references_in(organization),
    }
}

fn describe_bytes<C: BlockContent>(
    bytes: &[u8],
    organization: Uuid,
) -> Result<Description, String> {
    let content = C::decode(bytes).map_err(|error| error.to_string())?;
    Ok(describe(&content, organization))
}

/// How many closed blocks' unheld edits the module keeps for when they open again.
const CARRIED_BLOCKS: usize = 64;

struct State {
    replicas: BTreeMap<ReplicaId, Box<dyn AnyReplica>>,
    epochs: BTreeMap<ReplicaId, u64>,
    now: u64,
    wake: Option<u64>,
    sleep: Option<Pending<()>>,
    waiting: BTreeMap<RequestId, (ReplicaId, Sender<Answer>)>,
    closing: BTreeMap<ReplicaId, Sender<()>>,
    requests: RequestId,
    /// Each replica's block, and what the server lacked of the edits made in a block whose
    /// replica ended, for the next one to carry on with (`ToEditor::Lacking`).
    blocks: BTreeMap<ReplicaId, Uuid>,
    carried: BTreeMap<Uuid, lacking::Carried>,
    /// The blocks in `carried`, oldest first, for dropping the longest unopened.
    carried_order: std::collections::VecDeque<Uuid>,
}

struct Shared {
    types: RefCell<Vec<Registration>>,
    state: RefCell<State>,
    started: Cell<bool>,
}

#[derive(Clone)]
pub struct Module {
    shared: Rc<Shared>,
}

pub struct OpenReplica {
    module: Module,
    id: ReplicaId,
}

impl Module {
    pub fn new(types: Vec<Registration>) -> Self {
        Self {
            shared: Rc::new(Shared {
                types: RefCell::new(types),
                state: RefCell::new(State {
                    replicas: BTreeMap::new(),
                    epochs: BTreeMap::new(),
                    now: 0,
                    wake: None,
                    sleep: None,
                    waiting: BTreeMap::new(),
                    closing: BTreeMap::new(),
                    requests: 0,
                    blocks: BTreeMap::new(),
                    carried: BTreeMap::new(),
                    carried_order: std::collections::VecDeque::new(),
                }),
                started: Cell::new(false),
            }),
        }
    }

    pub fn executor(&self) -> Executor {
        be_sdk_type_api::guest::executor(self.clone())
    }

    pub fn types(&self) -> Vec<TypeInfo> {
        self.shared
            .types
            .borrow()
            .iter()
            .map(|kind| kind.info.clone())
            .collect()
    }

    pub fn register(&self, registration: Registration) {
        let content_type = registration.info.content_type;
        let mut types = self.shared.types.borrow_mut();
        match types
            .iter_mut()
            .find(|kind| kind.info.content_type == content_type)
        {
            Some(held) => *held = registration,
            None => types.push(registration),
        }
    }

    pub fn knows(&self, content_type: Uuid) -> bool {
        self.with_kind(content_type, |_| ()).is_some()
    }

    fn with_kind<R>(&self, content_type: Uuid, run: impl FnOnce(&Registration) -> R) -> Option<R> {
        self.shared
            .types
            .borrow()
            .iter()
            .find(|kind| kind.info.content_type == content_type)
            .map(run)
    }

    fn open(&self, replica: ReplicaId, opening: Opening) {
        let mut out = Vec::new();
        let now = self.shared.state.borrow().now;
        let opened = self.with_kind(opening.content_type, |kind| {
            (kind.open)(Opened {
                replica,
                content_type: opening.content_type,
                organization: opening.organization,
                client: opening.client,
                tag: opening.tag,
                head: opening.head,
                session: opening.session.clone(),
                journal: opening.journal.clone(),
                lost_edits: opening.lost_edits,
                journal_kept: opening.journal_kept,
                commits_kept: opening.commits_kept,
                refused_commits: opening.refused_commits.clone(),
                from_file: kind.from_file,
                now,
            })
        });
        let mut state = self.shared.state.borrow_mut();
        *state.epochs.entry(replica).or_default() += 1;
        let stale: Vec<RequestId> = state
            .waiting
            .iter()
            .filter(|(_, (held, _))| *held == replica)
            .map(|(id, _)| *id)
            .collect();
        for id in stale {
            state.waiting.remove(&id);
        }
        match opened {
            Some(mut opened) => {
                let carried = match state.replicas.get_mut(&replica) {
                    Some(before) => before.carry(),
                    None => {
                        state.carried_order.retain(|block| *block != opening.block);
                        state.carried.remove(&opening.block).unwrap_or_default()
                    }
                };
                opened.adopt(carried);
                state.blocks.insert(replica, opening.block);
                opened.start(&mut out);
                state.replicas.insert(replica, opened);
            }
            None => {
                state.replicas.remove(&replica);
                out.push(Effect::Error {
                    replica,
                    message: format!("this module has no block type {}", opening.content_type),
                });
                out.push(Effect::Release { replica });
            }
        }
        drop(state);
        self.run(out);
    }

    fn input(&self, replica: ReplicaId, input: Input) {
        let mut out = Vec::new();
        let unknown = {
            let mut state = self.shared.state.borrow_mut();
            let now = state.now;
            match state.replicas.get_mut(&replica) {
                Some(open) => {
                    open.input(now, input, &mut out);
                    None
                }
                None => match input {
                    Input::Change { id, .. } | Input::Content { id } | Input::Seal { id, .. } => {
                        Some(id)
                    }
                    _ => None,
                },
            }
        };
        if let Some(id) = unknown {
            out.push(Effect::Answer {
                id,
                answer: Answer::Unknown,
            });
        }
        self.run(out);
    }

    fn answered(&self, replica: ReplicaId, epoch: u64, input: Input) {
        let current = self.shared.state.borrow().epochs.get(&replica).copied();
        if current == Some(epoch) {
            self.input(replica, input);
        }
    }

    fn tick(&self, now: u64) {
        let mut out = Vec::new();
        {
            let mut state = self.shared.state.borrow_mut();
            state.now = state.now.max(now);
            let now = state.now;
            for replica in state.replicas.values_mut() {
                replica.tick(now, &mut out);
            }
        }
        self.run(out);
    }

    fn ask(
        &self,
        replica: ReplicaId,
        request: impl FnOnce(RequestId) -> Input,
    ) -> ops::Oneshot<Answer> {
        let (answer, answered) = ops::oneshot();
        let id = {
            let mut state = self.shared.state.borrow_mut();
            state.requests += 1;
            let id = state.requests;
            state.waiting.insert(id, (replica, answer));
            id
        };
        self.input(replica, request(id));
        answered
    }

    fn epoch(&self, replica: ReplicaId) -> u64 {
        self.shared
            .state
            .borrow()
            .epochs
            .get(&replica)
            .copied()
            .unwrap_or_default()
    }

    fn answer_later<T: 'static, E: be_wasm_ops::wire::Decode<T> + 'static>(
        &self,
        replica: ReplicaId,
        pending: Pending<T, E>,
        input: impl FnOnce(Result<T, String>) -> Input + 'static,
    ) {
        let module = self.clone();
        let epoch = self.epoch(replica);
        ops::spawn(async move {
            let result = pending.await.map_err(|error| error.to_string());
            module.answered(replica, epoch, input(result));
        });
    }

    fn run(&self, effects: Vec<Effect>) {
        for effect in effects {
            self.effect(effect);
        }
        let mut state = self.shared.state.borrow_mut();
        let state = &mut *state;
        let released: Vec<ReplicaId> = state
            .replicas
            .iter()
            .filter(|(_, replica)| replica.released())
            .map(|(id, _)| *id)
            .collect();
        for id in released {
            let carried = state
                .replicas
                .remove(&id)
                .map(|mut replica| replica.carry())
                .unwrap_or_default();
            if let Some(block) = state.blocks.remove(&id)
                && (!carried.entries.is_empty() || !carried.refused.is_empty())
            {
                state.carried_order.retain(|held| *held != block);
                state.carried_order.push_back(block);
                state.carried.insert(block, carried);
                // Kept for the blocks opened again soon; the others ask the host when they open.
                while state.carried.len() > CARRIED_BLOCKS {
                    let Some(oldest) = state.carried_order.pop_front() else {
                        break;
                    };
                    state.carried.remove(&oldest);
                }
            }
        }
        let wake = state
            .replicas
            .values()
            .filter_map(|replica| replica.wake_at())
            .min();
        if wake != state.wake {
            state.wake = wake;
            state.sleep = wake.map(host::sleep_until);
        }
    }

    fn effect(&self, effect: Effect) {
        match effect {
            Effect::Send { replica, to, bytes } => {
                ops::with_parent(replica, || host::send(to, bytes)).detach();
            }
            Effect::Publish {
                replica,
                token,
                expected,
                merging,
                content,
            } => {
                let pending =
                    ops::with_parent(replica, || host::publish(expected, merging, content));
                self.answer_later(replica, pending, move |result| {
                    Input::Head(match result.and_then(|published| published) {
                        Ok(PublishOutcome::Published(head)) => HeadEvent::Published {
                            token,
                            head,
                            held: true,
                        },
                        Ok(PublishOutcome::Kept(head)) => HeadEvent::Published {
                            token,
                            head,
                            held: false,
                        },
                        Ok(PublishOutcome::Rejected(head)) => HeadEvent::Rejected { token, head },
                        Err(error) => HeadEvent::Failed { token, error },
                    })
                });
            }
            Effect::Stash {
                replica,
                token,
                parent,
                content,
            } => {
                let pending = ops::with_parent(replica, || host::stash(parent, content));
                self.answer_later(replica, pending, move |result| {
                    Input::Head(match result.and_then(|stashed| stashed) {
                        Ok(commit) => HeadEvent::Stashed { token, commit },
                        Err(error) => HeadEvent::Failed { token, error },
                    })
                });
            }
            Effect::Load {
                replica,
                token,
                commit,
                first_part,
            } => {
                let pending = ops::with_parent(replica, || host::load(commit, first_part));
                self.answer_later(replica, pending, move |result| Input::Read {
                    token,
                    result: result.and_then(|loaded| loaded).and_then(|loaded| {
                        loaded
                            .into_stored()
                            .ok_or_else(|| "the host loaded parts that form no tree".to_owned())
                    }),
                });
            }
            Effect::ReadPart {
                replica,
                token,
                part,
                offset,
                length,
            } => {
                let range = be_sdk_type_api::ByteRange { offset, length };
                let pending = ops::with_parent(replica, || host::read_part(part.into(), range));
                self.answer_later(replica, pending, move |result| Input::PartRead {
                    token,
                    result: result.and_then(|read| read),
                });
            }
            Effect::PutPart {
                replica,
                token,
                bytes,
            } => {
                let pending = ops::with_parent(replica, || host::put_part(bytes));
                self.answer_later(replica, pending, move |result| Input::PartPut {
                    token,
                    result: result.and_then(|put| put).map(be_block::PartRef::from),
                });
            }
            Effect::Resume {
                replica,
                token,
                local,
            } => {
                let pending = ops::with_parent(replica, || host::resume_session(local));
                self.answer_later(replica, pending, move |result| Input::Resumed {
                    token,
                    result: result.and_then(|resumed| resumed),
                });
            }
            Effect::ClaimLease {
                replica,
                generation,
            } => ops::with_parent(replica, || host::claim_lease(generation)).detach(),
            Effect::Notify {
                replica,
                text,
                mentions,
            } => {
                let mentions = mentions.iter().map(Uuid::to_string).collect();
                ops::with_parent(replica, || host::notify(text, mentions)).detach()
            }
            Effect::ReportClean {
                replica,
                generation,
                clean_at,
            } => ops::with_parent(replica, || host::report_clean(generation, clean_at)).detach(),
            Effect::Append {
                replica,
                generation,
                entries,
                relay,
                from,
            } => {
                let through = entries
                    .iter()
                    .map(|entry| entry.sequence)
                    .max()
                    .unwrap_or_default();
                let pending =
                    ops::with_parent(replica, || host::append(generation, entries, relay));
                self.answer_later(replica, pending, move |held| Input::Appended {
                    from,
                    through,
                    held: held == Ok(true),
                });
            }
            Effect::Rebase {
                replica,
                generation,
                base,
            } => ops::with_parent(replica, || host::rebase_journal(generation, base)).detach(),
            Effect::Release { replica } => {
                let closing = self.shared.state.borrow_mut().closing.remove(&replica);
                ops::end(replica);
                if let Some(closing) = closing {
                    closing.send(());
                }
            }
            Effect::Status { replica, status } => {
                ops::with_parent(replica, || host::report_status(status)).detach();
            }
            Effect::Described {
                replica,
                description,
            } => ops::with_parent(replica, || host::described(description)).detach(),
            Effect::History { replica, history } => {
                ops::with_parent(replica, || host::history(history)).detach();
            }
            Effect::Conflict {
                replica,
                ours,
                theirs,
                count,
            } => ops::with_parent(replica, || {
                host::conflict(ours, theirs, count.try_into().unwrap_or(u32::MAX))
            })
            .detach(),
            Effect::Error { replica, message } => {
                ops::with_parent(replica, || host::error(message)).detach();
            }
            Effect::Answer { id, answer } => {
                let waiting = self.shared.state.borrow_mut().waiting.remove(&id);
                if let Some((_, waiting)) = waiting {
                    waiting.send(answer);
                }
            }
        }
    }
}

fn changed(answer: Option<Answer>) -> bool {
    matches!(answer, Some(Answer::Changed(true)))
}

impl replicas::Guest for Module {
    type Replica = OpenReplica;

    fn new_replica(&self, resource: ReplicaId, opening: Opening) -> OpenReplica {
        self.open(resource, opening);
        OpenReplica {
            module: self.clone(),
            id: resource,
        }
    }

    fn start(&self, entropy: Vec<u8>) -> impl Future<Output = Vec<TypeInfo>> + 'static {
        #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
        {
            let mut seed = [0u8; 32];
            for (held, byte) in seed.iter_mut().zip(entropy) {
                *held = byte;
            }
            entropy::seed(seed);
        }
        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        let _ = entropy;
        if !self.shared.started.replace(true) {
            let module = self.clone();
            ops::spawn(async move {
                let mut clock = host::clock();
                while let Some(now) = clock.recv().await {
                    module.tick(now);
                }
            });
        }
        ready(self.types())
    }

    fn blank(
        &self,
        content_type: Uuid,
    ) -> impl Future<Output = Result<Vec<u8>, Failure>> + 'static {
        ready(
            self.with_kind(content_type, |kind| (kind.blank)())
                .ok_or(Failure::UnknownType),
        )
    }

    fn merge3(
        &self,
        content_type: Uuid,
        base: Option<Vec<u8>>,
        ours: Option<Vec<u8>>,
        theirs: Option<Vec<u8>>,
    ) -> impl Future<Output = Result<Merged, Failure>> + 'static {
        ready(
            self.with_kind(content_type, |kind| {
                (kind.merge)(base.as_deref(), ours.as_deref(), theirs.as_deref())
            })
            .ok_or(Failure::UnknownType)
            .and_then(|merged| {
                merged
                    .map(|(bytes, conflicts)| Merged {
                        bytes,
                        conflicts: conflicts.try_into().unwrap_or(u32::MAX),
                    })
                    .map_err(Failure::Malformed)
            }),
        )
    }

    fn merge_parts(
        &self,
        content_type: Uuid,
        base: Option<Loaded>,
        ours: Option<Loaded>,
        theirs: Option<Loaded>,
        organization: Uuid,
    ) -> impl Future<Output = Result<MergedParts, Failure>> + 'static {
        let merge = self.with_kind(content_type, |kind| kind.merge_parts);
        async move {
            let merge = merge.ok_or(Failure::UnknownType)?.ok_or_else(|| {
                Failure::Malformed("this type merges whole values, with merge3".to_owned())
            })?;
            let (content, conflicts) = merge([base, ours, theirs], organization, request_parts())
                .await
                .map_err(Failure::Malformed)?;
            Ok(MergedParts {
                content,
                conflicts: conflicts.try_into().unwrap_or(u32::MAX),
            })
        }
    }

    fn replay(
        &self,
        content_type: Uuid,
        bytes: Option<Vec<u8>>,
        journal: Journal,
    ) -> impl Future<Output = Result<Vec<u8>, Failure>> + 'static {
        ready(
            self.with_kind(content_type, |kind| {
                (kind.replay)(bytes.as_deref(), &journal)
            })
            .ok_or(Failure::UnknownType)
            .and_then(|replayed| replayed.map_err(Failure::Malformed)),
        )
    }

    fn describe(
        &self,
        content_type: Uuid,
        bytes: Vec<u8>,
        organization: Uuid,
    ) -> impl Future<Output = Result<Description, Failure>> + 'static {
        ready(
            self.with_kind(content_type, |kind| (kind.describe)(&bytes, organization))
                .ok_or(Failure::UnknownType)
                .and_then(|described| described.map_err(Failure::Malformed)),
        )
    }

    fn to_file(
        &self,
        content_type: Uuid,
        bytes: Vec<u8>,
    ) -> impl Future<Output = Result<Option<PlainFile>, Failure>> + 'static {
        ready(
            match self.with_kind(content_type, |kind| {
                kind.to_file.map(|to_file| to_file(&bytes))
            }) {
                None => Err(Failure::UnknownType),
                Some(None) => Ok(None),
                Some(Some(file)) => file.map_err(Failure::Malformed),
            },
        )
    }

    fn search_text(
        &self,
        content_type: Uuid,
        content: Loaded,
        keys: Option<Vec<String>>,
        indexed: Vec<PartRef>,
    ) -> impl Future<Output = Result<Vec<SearchDocument>, Failure>> + 'static {
        let search = self.with_kind(content_type, |kind| kind.search_text);
        let indexed = indexed.into_iter().map(be_block::PartRef::from).collect();
        async move {
            let search = search.ok_or(Failure::UnknownType)?;
            let documents = search(content, keys, indexed, request_parts())
                .await
                .map_err(Failure::Malformed)?;
            Ok(documents
                .into_iter()
                .map(|document| SearchDocument {
                    key: document.key,
                    source: document.source.map(PartRef::from),
                    name: document.name,
                    body: document.body,
                })
                .collect())
        }
    }

    fn claims(
        &self,
        name: String,
        first: Vec<u8>,
        directory: bool,
        fallback: bool,
    ) -> impl Future<Output = Option<Uuid>> + 'static {
        let claimed = self
            .shared
            .types
            .borrow()
            .iter()
            .find(|kind| match directory {
                true => kind.claims_directories && !fallback,
                false => {
                    kind.claims_any_file == fallback
                        && kind.claims.is_some_and(|claims| claims(&name, &first))
                }
            })
            .map(|kind| kind.info.content_type);
        ready(claimed)
    }

    fn apply(
        &self,
        content_type: Uuid,
        bytes: Option<Vec<u8>>,
        client: ClientId,
        change: Change,
    ) -> impl Future<Output = Result<Option<Vec<u8>>, Failure>> + 'static {
        ready(
            self.with_kind(content_type, |kind| {
                apply(kind, bytes.as_deref(), client, change)
            })
            .ok_or(Failure::UnknownType)
            .and_then(|applied| applied),
        )
    }
}

impl replicas::GuestReplica for OpenReplica {
    fn reopen(&self, opening: Opening) -> impl Future<Output = ()> + 'static {
        self.module.open(self.id, opening);
        ready(())
    }

    fn message(
        &self,
        from: Endpoint,
        bytes: Vec<u8>,
        at: u64,
    ) -> impl Future<Output = ()> + 'static {
        self.module.tick(at);
        self.module.input(self.id, Input::Message { from, bytes });
        ready(())
    }

    fn attach(&self, editor: EditorId, editable: bool) -> impl Future<Output = ()> + 'static {
        self.module
            .input(self.id, Input::Attach { editor, editable });
        ready(())
    }

    fn drained(&self, refused: bool, commits: Vec<CommitId>) -> impl Future<Output = ()> + 'static {
        self.module
            .input(self.id, Input::Drained { refused, commits });
        ready(())
    }

    fn detach(&self, editor: EditorId) -> impl Future<Output = ()> + 'static {
        self.module.input(self.id, Input::Detach { editor });
        ready(())
    }

    fn head_moved(&self, head: CommitId) -> impl Future<Output = ()> + 'static {
        self.module
            .input(self.id, Input::Head(HeadEvent::Moved { head }));
        ready(())
    }

    fn lease(
        &self,
        session: SessionState,
        resynchronize: bool,
    ) -> impl Future<Output = ()> + 'static {
        self.module.input(
            self.id,
            Input::Lease {
                session,
                resynchronize,
            },
        );
        ready(())
    }

    fn claim(&self) -> impl Future<Output = ()> + 'static {
        self.module.input(self.id, Input::Claim);
        ready(())
    }

    fn change(&self, change: Change) -> impl Future<Output = bool> + 'static {
        let answered = self.module.ask(self.id, |id| Input::Change { id, change });
        async move { changed(answered.await) }
    }

    fn content(&self) -> impl Future<Output = Option<Vec<u8>>> + 'static {
        let answered = self.module.ask(self.id, |id| Input::Content { id });
        async move {
            match answered.await {
                Some(Answer::Content(bytes)) => Some(bytes),
                _ => None,
            }
        }
    }

    fn seal(&self, force: bool) -> impl Future<Output = ()> + 'static {
        let answered = self.module.ask(self.id, |id| Input::Seal { id, force });
        async move {
            answered.await;
        }
    }

    fn on_drop(&self) -> impl Future<Output = ()> + 'static {
        let (closed, closing) = ops::oneshot();
        let open = {
            let mut state = self.module.shared.state.borrow_mut();
            let open = state.replicas.contains_key(&self.id);
            if open {
                state.closing.insert(self.id, closed);
            }
            open
        };
        if open {
            self.module.input(self.id, Input::Close);
        }
        async move {
            if open {
                closing.await;
            }
        }
    }
}

fn apply(
    kind: &Registration,
    bytes: Option<&[u8]>,
    client: u64,
    change: Change,
) -> Result<Option<Vec<u8>>, Failure> {
    let applied = match change {
        Change::Child(change) => (kind.child)(bytes, change),
        Change::FromFile(FileChange { name, bytes: file }) => match kind.from_file {
            Some(from_file) => Ok(from_file(bytes, &name, file, client)),
            None => Ok(None),
        },
        Change::Replace(_) | Change::Undo | Change::Redo => return Err(Failure::UnknownType),
    };
    applied.map_err(Failure::Malformed)
}

pub struct Serving {
    module: Module,
    executor: Executor,
}

impl Serving {
    pub fn new(module: Module) -> Self {
        let executor = module.executor();
        Self { module, executor }
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn step(&self, inbound: Vec<Vec<u8>>, outbound: &mut Vec<Vec<u8>>) {
        if let Err(error) = self.executor.step(inbound, outbound) {
            self.executor
                .enter(|| host::error(format!("a frame did not decode: {error}")).detach());
            let _ = self.executor.step(Vec::new(), outbound);
        }
    }
}

#[macro_export]
macro_rules! type_module {
    ($guest:ty) => {
        $crate::abi::guest!($guest);
    };
}

#[macro_export]
macro_rules! be_block_types {
    ($($kind:ty $({ $($part:ident),* $(,)? })?),* $(,)?) => {
        pub fn registrations() -> ::std::vec::Vec<$crate::Registration> {
            ::std::vec![
                $($crate::Registration::of::<$kind>()$($(.$part::<$kind>())*)?),*
            ]
        }

        pub fn block_types() -> $crate::Module {
            $crate::Module::new(registrations())
        }

        pub struct BlockTypes($crate::Serving);

        impl $crate::abi::Guest for BlockTypes {
            fn start(
                _: $crate::abi::Waker,
                _: &mut ::std::vec::Vec<::std::vec::Vec<u8>>,
            ) -> ::std::result::Result<Self, ::std::string::String> {
                ::std::result::Result::Ok(Self($crate::Serving::new(block_types())))
            }

            fn step(
                &mut self,
                inbound: ::std::vec::Vec<::std::vec::Vec<u8>>,
                _: bool,
                outbound: &mut ::std::vec::Vec<::std::vec::Vec<u8>>,
            ) -> ::std::result::Result<$crate::abi::Flow, ::std::string::String> {
                self.0.step(inbound, outbound);
                ::std::result::Result::Ok($crate::abi::Flow::Running)
            }
        }
    };
}

#[cfg(test)]
mod tests;
