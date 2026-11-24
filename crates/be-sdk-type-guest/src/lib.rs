use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashSet},
    future::{Future, ready},
    rc::Rc,
};

use be_block_commit::MergeResult;
use be_block_content::{
    BlockContent, LiveEdit, Loads, Merge, MergeParts, PartReader, Undo, Unread, history::History,
};
use be_sdk_type_api::{
    BlockPlace, Change, ChildChange, ClientId, CommitId, Description, EditorId, Endpoint,
    FileChange, Journal, JournalEntry, Loaded, Merged, MergedParts, Opening, PartRef, Plain,
    PlainFile, PublishOutcome, ReorderChild, ReplaceChild, ReplicaId, SearchDocument, SessionState,
    TypeInfo, Written,
    guest::{exports::replicas, host},
    ops::guest::{self as ops, Executor, LocalFuture, Pending, Sender},
    types::Failure,
};
use uuid::Uuid;

pub mod editor;
mod effect;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod entropy;
mod file;
pub mod holds;
mod lacking;
mod record;
mod replica;
pub mod wire;

pub use be_sdk_type_api as api;
pub use be_wasm_guest_abi as abi;
pub use file::{BlobFile, DocumentFile, FileForm};
pub use record::{NoHistory, Recorder};
pub use replica::{COMMIT_INTERVAL, JOURNAL_BYTES, QUIET_INTERVAL, RESTART_STATE_BYTES};

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
    Vec<be_block_content::PartRef>,
    PartReader,
) -> std::pin::Pin<
    Box<dyn Future<Output = Result<Vec<be_block_content::SearchDocument>, String>>>,
>;

type ChildFn = fn(Option<&[u8]>, ChildChange, u64) -> Result<Option<Vec<u8>>, String>;

type ToFileFn = fn(&[u8]) -> Result<Option<PlainFile>, String>;

type ReplayFn = fn(Option<&[u8]>, &Journal) -> Result<Vec<u8>, String>;

/// A type's `Unread`: `places` of the account's layers of its reads kind, lowest first, each
/// block's place in postcard bytes; and `count_unread` of content as a host loads it (`Loaded`)
/// or of whole content bytes, with a block's place (none: the default place).
#[derive(Clone, Copy)]
struct UnreadFns {
    places: fn(&[Vec<u8>]) -> Result<Vec<(Uuid, Vec<u8>)>, String>,
    count: fn(Content, Option<&[u8]>, &[be_block_content::LogItem], Uuid) -> Result<u32, String>,
}

/// Content handed to a stateless request: as a host loads a commit, or whole bytes.
pub enum Content<'a> {
    Loaded(Loaded),
    Bytes(&'a [u8]),
}

pub struct Registration {
    info: TypeInfo,
    open: fn(Opened) -> Box<dyn AnyReplica>,
    plain: fn(Opened) -> Box<dyn AnyReplica>,
    /// The type's default content and its description, which `Module::types` fills
    /// `info` with once `start` has seeded the module's entropy.
    blank: fn() -> (Vec<u8>, Description),
    merge: MergeFn,
    merge_parts: Option<MergePartsFn>,
    /// The replica's merge by parts, a `replica::PartsMerge` of the type's content.
    replica_merge: Option<std::rc::Rc<dyn std::any::Any>>,
    /// The type's fold of its signed log, a `replica::Fold` of its content, if it has a log
    /// that what its content shows folds (`LogFold::FOLDS`).
    fold: Option<std::rc::Rc<dyn std::any::Any>>,
    /// What an entry of the type's signed log changes in its searchable text
    /// (`LogFold::search`), if it has a log.
    search_log: Option<fn(&be_block_content::LogItem) -> Option<be_block_content::LogSearch>>,
    describe: fn(&[u8], Uuid) -> Result<Description, String>,
    search_text: SearchTextFn,
    unread: Option<UnreadFns>,
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
                schema: be_block_content::schema_fingerprint::<C>(),
                undo: false,
                plain: Plain::None,
                first_part: C::LOADS == Loads::FirstPart,
                merges_parts: false,
                whole_bytes: C::WHOLE_BYTES,
                log: false,
                blank: Vec::new(),
                blank_description: Description::default(),
                reads: None,
            },
            open: open::<C, NoHistory>,
            plain: open::<C, NoHistory>,
            blank: || {
                let blank = C::default();
                (blank.encode(), describe(&blank, Uuid::nil()))
            },
            merge: merge::<C>,
            merge_parts: None,
            replica_merge: None,
            fold: None,
            search_log: None,
            describe: describe_bytes::<C>,
            search_text: search_text::<C>,
            unread: None,
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
        self.replica_merge = Some(std::rc::Rc::new(
            replica_merge::<C> as replica::PartsMerge<C>,
        ));
        self
    }

    /// Counts what the account has not read (`Unread`), from its layers of `C::Reads`.
    pub fn unread<C: Unread>(mut self) -> Self {
        assert_eq!(self.info.content_type, C::CONTENT_TYPE);
        self.info.reads = Some(<C::Reads as be_block_content::Root>::CONTENT_TYPE);
        self.unread = Some(UnreadFns {
            places: places::<C>,
            count: count_unread::<C>,
        });
        self
    }

    /// Keeps a signed log beside the content (`type-info.log`), folded onto what it shows
    /// (`LogFold`): its editors append and read the log through their replica.
    pub fn log<C: be_block_content::LogFold + LiveEdit>(mut self) -> Self {
        assert_eq!(self.info.content_type, C::CONTENT_TYPE);
        self.info.log = true;
        self.search_log = Some(C::search);
        self.fold = C::FOLDS.then(|| {
            std::rc::Rc::new(fold::<C> as replica::Fold<C>) as std::rc::Rc<dyn std::any::Any>
        });
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

fn fold<C: be_block_content::LogFold>(content: &mut C, item: &be_block_content::LogItem) {
    content.fold(item);
}

/// A log entry as the host interface carries it, as a type sees it.
fn refused_entry(entry: be_sdk_type_api::RefusedEntry) -> (Uuid, Vec<u8>, String) {
    (entry.id, entry.payload, entry.why)
}

fn log_item(item: be_sdk_type_api::LogItem) -> be_block_content::LogItem {
    use be_block_content::LogSigner;
    be_block_content::LogItem {
        seq: item.seq,
        id: item.id,
        author: item.author,
        arrival: item.arrival,
        payload: item.payload,
        signer: match item.signer {
            be_sdk_type_api::Signer::Verified => LogSigner::Verified,
            be_sdk_type_api::Signer::Unverified => LogSigner::Unverified,
            be_sdk_type_api::Signer::Changed => LogSigner::Changed,
            be_sdk_type_api::Signer::Replaced => LogSigner::Replaced,
            be_sdk_type_api::Signer::Bad => LogSigner::Bad,
            be_sdk_type_api::Signer::Unknown => LogSigner::Unknown,
            be_sdk_type_api::Signer::Redacted => LogSigner::Redacted,
        },
        imported: item.imported.map(|imported| be_block_content::LogImport {
            block: imported.block,
            seq: imported.seq,
            importer: imported.importer,
        }),
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

fn replica_merge<C: MergeParts + Default>(
    base: C,
    ours: C,
    theirs: C,
    parts: PartReader,
) -> std::pin::Pin<Box<dyn Future<Output = Result<MergeResult<C>, String>>>> {
    Box::pin(C::merge_parts(base, ours, theirs, parts))
}

fn search_text<C: BlockContent>(
    content: Loaded,
    keys: Option<Vec<String>>,
    indexed: Vec<be_block_content::PartRef>,
    parts: PartReader,
) -> std::pin::Pin<Box<dyn Future<Output = Result<Vec<be_block_content::SearchDocument>, String>>>>
{
    Box::pin(async move {
        let stored = content
            .into_stored()
            .ok_or_else(|| "the host loaded parts that form no tree".to_owned())?;
        let content = C::from_stored(&stored).map_err(|error| error.to_string())?;
        let keys: Option<HashSet<String>> = keys.map(|keys| keys.into_iter().collect());
        let indexed: HashSet<be_block_content::PartRef> = indexed.into_iter().collect();
        content.search_text(keys.as_ref(), &indexed, parts).await
    })
}

fn places<C: Unread>(reads: &[Vec<u8>]) -> Result<Vec<(Uuid, Vec<u8>)>, String> {
    let layers = reads
        .iter()
        .map(|bytes| {
            be_block_content::be_block_model::Document::<C::Reads>::decode(bytes)
                .map(|document| document.root())
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    C::places(&be_block_content::layered(layers))
        .into_iter()
        .map(|(block, place)| {
            postcard::to_stdvec(&place)
                .map(|bytes| (block, bytes))
                .map_err(|error| error.to_string())
        })
        .collect()
}

fn count_unread<C: Unread>(
    content: Content,
    place: Option<&[u8]>,
    log: &[be_block_content::LogItem],
    me: Uuid,
) -> Result<u32, String> {
    let content = match content {
        Content::Loaded(loaded) => {
            let stored = loaded
                .into_stored()
                .ok_or_else(|| "the host loaded parts that form no tree".to_owned())?;
            C::from_stored(&stored)
        }
        Content::Bytes(bytes) => C::decode(bytes),
    }
    .map_err(|error| error.to_string())?;
    let place = match place {
        Some(bytes) => postcard::from_bytes(bytes).map_err(|error| error.to_string())?,
        None => C::Place::default(),
    };
    Ok(content.count_unread(&place, log, me))
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
        derived: be_block_content::derived_metadata(content),
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
    client: u64,
) -> Result<Option<Vec<u8>>, String> {
    let mut content = match bytes {
        Some(bytes) => C::decode(bytes).map_err(|error| error.to_string())?,
        None => C::default(),
    };
    let Some(operations) = content.child_operations(be_child(change), client) else {
        return Ok(None);
    };
    for operation in &operations {
        content.apply(operation);
    }
    Ok(Some(content.encode()))
}

pub(crate) fn be_child(change: ChildChange) -> be_block_content::ChildChange {
    match change {
        ChildChange::Add(block) => be_block_content::ChildChange::Add(block),
        ChildChange::Delete(block) => be_block_content::ChildChange::Delete(block),
        ChildChange::Replace(ReplaceChild { old, new }) => {
            be_block_content::ChildChange::Replace { old, new }
        }
        ChildChange::Reorder(ReorderChild { block, after }) => {
            be_block_content::ChildChange::Reorder { block, after }
        }
    }
}

pub fn describe<C: BlockContent>(content: &C, organization: Uuid) -> Description {
    Description {
        derived: be_block_content::derived_metadata(content),
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
    /// The soonest of the replicas' timers that keep a block in step, as last told the host.
    in_step: Option<u64>,
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
                    in_step: None,
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
            .map(|kind| {
                let (blank, blank_description) = (kind.blank)();
                TypeInfo {
                    blank,
                    blank_description,
                    ..kind.info.clone()
                }
            })
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

    /// The settings kind `content_type` counts what is unread from (`Registration::unread`).
    pub fn reads_of(&self, content_type: Uuid) -> Option<Uuid> {
        self.with_kind(content_type, |kind| kind.info.reads)?
    }

    /// `replicas.places` and then `replicas.unread` of whole content bytes, at once, for a host
    /// in process: none for a type that counts nothing unread, or content that does not decode.
    pub fn unread_of_bytes(
        &self,
        content_type: Uuid,
        bytes: &[u8],
        reads: &[Vec<u8>],
        log: &[be_sdk_type_api::LogItem],
        block: Uuid,
        account: Uuid,
    ) -> Option<u32> {
        let fns = self.with_kind(content_type, |kind| kind.unread)??;
        let places = (fns.places)(reads).ok()?;
        let place = places
            .iter()
            .find(|(of, _)| *of == block)
            .map(|(_, place)| place.as_slice());
        let log: Vec<_> = log.iter().cloned().map(log_item).collect();
        (fns.count)(Content::Bytes(bytes), place, &log, account).ok()
    }

    fn unread_fns(&self, content_type: Uuid) -> Result<UnreadFns, Failure> {
        match self.with_kind(content_type, |kind| kind.unread) {
            None => Err(Failure::UnknownType),
            Some(None) => Err(Failure::Malformed(
                "this type counts nothing unread".to_owned(),
            )),
            Some(Some(fns)) => Ok(fns),
        }
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
                account: opening.account,
                client: opening.client,
                tag: opening.tag,
                origin: opening.origin,
                taken_before: opening.taken_before,
                doubtful_through: opening.doubtful_through,
                kept_restart: opening.kept_restart,
                kept_through: opening.kept_through,
                kept_submitted: opening.kept_submitted,
                kept_submissions: opening.kept_submissions.clone(),
                head: opening.head,
                session: opening.session.clone(),
                journal: opening.journal.clone(),
                sealed_head: opening.sealed_head.clone(),
                lost_edits: opening.lost_edits,
                journal_kept: opening.journal_kept,
                commits_kept: opening.commits_kept,
                refused_commits: opening.refused_commits.clone(),
                server_head: opening.server_head,
                epoch: opening.epoch,
                from_file: kind.from_file,
                parts_merge: kind.replica_merge.clone(),
                fold: kind.fold.clone(),
                log: kind.info.log,
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
                // The calls the replica before made were made under the epoch before this one.
                let before_epoch = state.epochs[&replica] - 1;
                let carried = match state.replicas.get_mut(&replica) {
                    Some(before) => before.carry(before_epoch),
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
        match current == Some(epoch) {
            true => self.input(replica, input),
            // Made by a replica of the block before this one opened: handed on to the editor
            // it was made for (`AnyReplica::late`).
            false => self.late(replica, epoch, input),
        }
    }

    fn late(&self, replica: ReplicaId, epoch: u64, input: Input) {
        let mut out = Vec::new();
        {
            let mut state = self.shared.state.borrow_mut();
            let now = state.now;
            if let Some(open) = state.replicas.get_mut(&replica) {
                open.late(now, epoch, input, &mut out);
            }
        }
        self.run(out);
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
                .map(|mut replica| {
                    let epoch = state.epochs.get(&id).copied().unwrap_or_default();
                    // A released replica carries none of what it did for editors (its asks and
                    // its range reads, `EditorCalls`), though a close releases it with editors
                    // still attached: their answers go to its id, which no later replica has.
                    lacking::Carried {
                        editor_calls: lacking::EditorCalls::default(),
                        ..replica.carry(epoch)
                    }
                })
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
        let in_step = state
            .replicas
            .values()
            .filter_map(|replica| replica.in_step_at())
            .min();
        if in_step != state.in_step {
            state.in_step = in_step;
            host::in_step_at(in_step).detach();
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
                sealed_through,
                journal_state,
                restarts,
            } => {
                let pending = ops::with_parent(replica, || {
                    host::publish(
                        expected,
                        merging,
                        content,
                        sealed_through,
                        journal_state,
                        restarts,
                    )
                });
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
                    result: result
                        .map_err(be_sdk_type_api::LoadError::NotYet)
                        .and_then(|loaded| loaded)
                        .and_then(|loaded| {
                            loaded.into_stored().ok_or_else(|| {
                                be_sdk_type_api::LoadError::Unreadable(
                                    "the host loaded parts that form no tree".to_owned(),
                                )
                            })
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
                    result: result
                        .and_then(|put| put)
                        .map(be_block_content::PartRef::from),
                });
            }
            Effect::Append {
                replica,
                token,
                bytes,
                along,
            } => {
                let along = be_sdk_type_api::Along {
                    announcement: along.announcement.map(announcement),
                    redacts: along.redacts,
                };
                let pending = ops::with_parent(replica, || host::append(bytes, along));
                self.answer_later(replica, pending, move |result| Input::Appended {
                    token,
                    result: match result {
                        Ok(Ok(appended)) => Ok(be_block_content::LogAppended {
                            id: appended.id,
                            place: appended.place.map(|place| (place.seq, place.arrival)),
                        }),
                        Ok(Err(not)) => Err(be_block_content::LogNotAppended {
                            id: not.id,
                            why: not.why,
                        }),
                        Err(why) => Err(be_block_content::LogNotAppended { id: None, why }),
                    },
                });
            }
            Effect::ReadLog {
                replica,
                token,
                from,
                limit,
            } => {
                let pending = ops::with_parent(replica, || host::read_log(from, limit));
                self.answer_later(replica, pending, move |result| Input::LogRead {
                    token,
                    result: result
                        .and_then(|read| read)
                        .map(|page| be_block_content::LogPage {
                            size: page.size,
                            items: page.items.into_iter().map(log_item).collect(),
                            kept: page
                                .kept
                                .into_iter()
                                .map(|kept| (kept.id, kept.payload))
                                .collect(),
                            refused: page.refused.into_iter().map(refused_entry).collect(),
                        }),
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
            Effect::Claim { replica } => ops::with_parent(replica, host::claim).detach(),
            Effect::LetGoRefused { replica, id } => {
                ops::with_parent(replica, || host::let_go_refused(id)).detach();
            }
            Effect::Submit {
                replica,
                submission,
                announcements,
            } => {
                let counter = submission.id.counter;
                let submission = be_sdk_type_api::Submission {
                    origin: submission.id.origin,
                    counter,
                    base: submission.base,
                    barrier: submission.barrier,
                    payload: submission.payload,
                };
                let announcements = announcements.into_iter().map(announcement).collect();
                let pending = ops::with_parent(replica, || host::submit(submission, announcements));
                self.answer_later(replica, pending, move |submitted| Input::Submitted {
                    counter,
                    outcome: match submitted {
                        Ok(be_sdk_type_api::host::Submitted::Held) => replica::Outcome::Held,
                        Ok(be_sdk_type_api::host::Submitted::Kept) => replica::Outcome::Kept,
                        Ok(be_sdk_type_api::host::Submitted::Failed(error)) => {
                            replica::Outcome::Failed(error)
                        }
                        Ok(be_sdk_type_api::host::Submitted::Refused(error)) => {
                            replica::Outcome::Refused(error)
                        }
                        Err(error) => replica::Outcome::Failed(error.to_string()),
                    },
                });
            }
            Effect::Rebase {
                replica,
                base,
                restarts,
            } => ops::with_parent(replica, || host::rebase_journal(base, restarts)).detach(),
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

    fn start(&self, entropy: Vec<u8>) -> LocalFuture<Vec<TypeInfo>> {
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
        Box::pin(ready(self.types()))
    }

    fn merge3(
        &self,
        content_type: Uuid,
        base: Option<Vec<u8>>,
        ours: Option<Vec<u8>>,
        theirs: Option<Vec<u8>>,
    ) -> LocalFuture<Result<Merged, Failure>> {
        Box::pin(ready(
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
        ))
    }

    fn merge_parts(
        &self,
        content_type: Uuid,
        base: Option<Loaded>,
        ours: Option<Loaded>,
        theirs: Option<Loaded>,
        organization: Uuid,
    ) -> LocalFuture<Result<MergedParts, Failure>> {
        let merge = self.with_kind(content_type, |kind| kind.merge_parts);
        Box::pin(async move {
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
        })
    }

    fn replay(
        &self,
        content_type: Uuid,
        bytes: Option<Vec<u8>>,
        journal: Journal,
    ) -> LocalFuture<Result<Vec<u8>, Failure>> {
        Box::pin(ready(
            self.with_kind(content_type, |kind| {
                (kind.replay)(bytes.as_deref(), &journal)
            })
            .ok_or(Failure::UnknownType)
            .and_then(|replayed| replayed.map_err(Failure::Malformed)),
        ))
    }

    fn describe(
        &self,
        content_type: Uuid,
        bytes: Vec<u8>,
        organization: Uuid,
    ) -> LocalFuture<Result<Description, Failure>> {
        Box::pin(ready(
            self.with_kind(content_type, |kind| (kind.describe)(&bytes, organization))
                .ok_or(Failure::UnknownType)
                .and_then(|described| described.map_err(Failure::Malformed)),
        ))
    }

    fn to_file(
        &self,
        content_type: Uuid,
        bytes: Vec<u8>,
    ) -> LocalFuture<Result<Option<PlainFile>, Failure>> {
        Box::pin(ready(
            match self.with_kind(content_type, |kind| {
                kind.to_file.map(|to_file| to_file(&bytes))
            }) {
                None => Err(Failure::UnknownType),
                Some(None) => Ok(None),
                Some(Some(file)) => file.map_err(Failure::Malformed),
            },
        ))
    }

    fn search_text(
        &self,
        content_type: Uuid,
        content: Loaded,
        keys: Option<Vec<String>>,
        indexed: Vec<PartRef>,
    ) -> LocalFuture<Result<Vec<SearchDocument>, Failure>> {
        let search = self.with_kind(content_type, |kind| kind.search_text);
        let indexed = indexed
            .into_iter()
            .map(be_block_content::PartRef::from)
            .collect();
        Box::pin(async move {
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
        })
    }

    fn places(
        &self,
        content_type: Uuid,
        reads: Vec<Vec<u8>>,
    ) -> LocalFuture<Result<Vec<BlockPlace>, Failure>> {
        Box::pin(ready(match self.unread_fns(content_type) {
            Ok(fns) => (fns.places)(&reads)
                .map(|places| {
                    places
                        .into_iter()
                        .map(|(block, place)| BlockPlace { block, place })
                        .collect()
                })
                .map_err(Failure::Malformed),
            Err(failure) => Err(failure),
        }))
    }

    fn search_log(
        &self,
        content_type: Uuid,
        items: Vec<be_sdk_type_api::LogItem>,
    ) -> LocalFuture<Result<Vec<be_sdk_type_api::LogSearch>, Failure>> {
        let search = match self.with_kind(content_type, |kind| kind.search_log) {
            None => Err(Failure::UnknownType),
            Some(None) => Err(Failure::Malformed("this type keeps no log".to_owned())),
            Some(Some(search)) => Ok(search),
        };
        Box::pin(ready(search.map(|search| {
            items
                .into_iter()
                .map(log_item)
                .filter_map(|item| {
                    let change = search(&item)?;
                    Some(be_sdk_type_api::LogSearch {
                        seq: item.seq,
                        place: change.place,
                        text: change.text,
                    })
                })
                .collect()
        })))
    }

    fn unread(
        &self,
        content_type: Uuid,
        content: Loaded,
        place: Option<Vec<u8>>,
        log: Vec<be_sdk_type_api::LogItem>,
        account: Uuid,
    ) -> LocalFuture<Result<u32, Failure>> {
        let log: Vec<_> = log.into_iter().map(log_item).collect();
        Box::pin(ready(match self.unread_fns(content_type) {
            Ok(fns) => (fns.count)(Content::Loaded(content), place.as_deref(), &log, account)
                .map_err(Failure::Malformed),
            Err(failure) => Err(failure),
        }))
    }

    fn claims(
        &self,
        name: String,
        first: Vec<u8>,
        directory: bool,
        fallback: bool,
    ) -> LocalFuture<Option<Uuid>> {
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
        Box::pin(ready(claimed))
    }

    fn apply(
        &self,
        content_type: Uuid,
        bytes: Option<Vec<u8>>,
        client: ClientId,
        change: Change,
    ) -> LocalFuture<Result<Option<Vec<u8>>, Failure>> {
        Box::pin(ready(
            self.with_kind(content_type, |kind| {
                apply(kind, bytes.as_deref(), client, change)
            })
            .ok_or(Failure::UnknownType)
            .and_then(|applied| applied),
        ))
    }
}

impl replicas::GuestReplica for OpenReplica {
    fn reopen(&self, opening: Opening) -> LocalFuture<()> {
        self.module.open(self.id, opening);
        Box::pin(ready(()))
    }

    fn message(&self, from: Endpoint, bytes: Vec<u8>, at: u64) -> LocalFuture<()> {
        self.module.tick(at);
        self.module.input(self.id, Input::Message { from, bytes });
        Box::pin(ready(()))
    }

    fn attach(&self, editor: EditorId, editable: bool, acknowledged: u64) -> LocalFuture<()> {
        self.module.input(
            self.id,
            Input::Attach {
                editor,
                editable,
                acknowledged,
            },
        );
        Box::pin(ready(()))
    }

    fn drained(&self, refused: bool, commits: Vec<CommitId>) -> LocalFuture<()> {
        self.module
            .input(self.id, Input::Drained { refused, commits });
        Box::pin(ready(()))
    }

    fn resent(&self, editor: EditorId) -> LocalFuture<()> {
        self.module.input(self.id, Input::Resent { editor });
        Box::pin(ready(()))
    }

    fn detach(&self, editor: EditorId) -> LocalFuture<()> {
        self.module.input(self.id, Input::Detach { editor });
        Box::pin(ready(()))
    }

    fn head_moved(&self, head: CommitId) -> LocalFuture<()> {
        self.module
            .input(self.id, Input::Head(HeadEvent::Moved { head }));
        Box::pin(ready(()))
    }

    fn sequenced(&self, entries: Vec<JournalEntry>) -> LocalFuture<()> {
        let ops = entries
            .into_iter()
            .map(|entry| be_block_session::SessionOp {
                id: be_block_session::OpId {
                    origin: entry.origin,
                    counter: entry.counter,
                },
                sequence: entry.sequence,
                base: entry.base,
                floor: entry.floor,
                received: entry.received,
                payload: entry.payload,
            })
            .collect();
        self.module.input(self.id, Input::Sequenced(ops));
        Box::pin(ready(()))
    }

    fn lease(&self, session: SessionState) -> LocalFuture<()> {
        self.module.input(self.id, Input::Lease { session });
        Box::pin(ready(()))
    }

    fn logged(&self, items: Vec<be_sdk_type_api::LogItem>) -> LocalFuture<()> {
        self.module.input(
            self.id,
            Input::Logged(items.into_iter().map(log_item).collect()),
        );
        Box::pin(ready(()))
    }

    fn refused(&self, entries: Vec<be_sdk_type_api::RefusedEntry>) -> LocalFuture<()> {
        self.module.input(
            self.id,
            Input::Refused(entries.into_iter().map(refused_entry).collect()),
        );
        Box::pin(ready(()))
    }

    fn sealed(&self, commit: CommitId, through: u64, restarts: u64) -> LocalFuture<()> {
        self.module.input(
            self.id,
            Input::Sealed {
                head: commit,
                through,
                restarts,
            },
        );
        Box::pin(ready(()))
    }

    fn claim(&self) -> LocalFuture<()> {
        self.module.input(self.id, Input::Claim);
        Box::pin(ready(()))
    }

    fn change(&self, change: Change) -> LocalFuture<bool> {
        let answered = self.module.ask(self.id, |id| Input::Change { id, change });
        Box::pin(async move { changed(answered.await) })
    }

    fn content(&self) -> LocalFuture<Option<Vec<u8>>> {
        let answered = self.module.ask(self.id, |id| Input::Content { id });
        Box::pin(async move {
            match answered.await {
                Some(Answer::Content(bytes)) => Some(bytes),
                _ => None,
            }
        })
    }

    fn seal(&self, force: bool) -> LocalFuture<()> {
        let answered = self.module.ask(self.id, |id| Input::Seal { id, force });
        Box::pin(async move {
            answered.await;
        })
    }

    fn on_drop(&self) -> LocalFuture<()> {
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
        Box::pin(async move {
            if open {
                closing.await;
            }
        })
    }
}

fn apply(
    kind: &Registration,
    bytes: Option<&[u8]>,
    client: u64,
    change: Change,
) -> Result<Option<Vec<u8>>, Failure> {
    let applied = match change {
        Change::Child(change) => (kind.child)(bytes, change, client),
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

/// An announcement as the host interface carries it.
fn announcement(made: be_block_content::Announcement) -> be_sdk_type_api::Announcement {
    be_sdk_type_api::Announcement {
        key: made.key,
        text: made.text,
        several: made.several,
        mentions: made.mentions,
    }
}

#[cfg(test)]
mod tests;
