use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    future::Future,
    rc::Rc,
};

use be_block::{BlockContent, LiveEdit, Merge, Undo, history::History};
use be_commit::MergeResult;
use block_type_api::{
    Change, ChildChange, ClientId, CommitId, Description, EditorId, Endpoint, FileChange, Merged,
    Opening, Plain, PlainFile, PublishOutcome, ReplaceChild, ReplicaId, SessionState, TypeInfo,
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
mod record;
mod replica;
pub mod wire;

pub use block_guest_abi as abi;
pub use block_type_api as api;
pub use file::{BlobFile, DocumentFile, FileForm};
pub use record::{NoHistory, Recorder};
pub use replica::SEAL_INTERVAL;

use effect::{Answer, Effect, HeadEvent, RequestId};
use replica::{AnyReplica, Input, Opened, Replica};

type FromFileFn = fn(Option<&[u8]>, &str, Vec<u8>, u64) -> Option<Vec<u8>>;

type MergeFn = fn(Option<&[u8]>, Option<&[u8]>, Option<&[u8]>) -> Result<(Vec<u8>, usize), String>;

type ChildFn = fn(Option<&[u8]>, ChildChange) -> Result<Option<Vec<u8>>, String>;

type ToFileFn = fn(&[u8]) -> Result<Option<PlainFile>, String>;

pub struct Registration {
    info: TypeInfo,
    open: fn(Opened) -> Box<dyn AnyReplica>,
    plain: fn(Opened) -> Box<dyn AnyReplica>,
    blank: fn() -> Vec<u8>,
    merge: MergeFn,
    describe: fn(&[u8], Uuid) -> Result<Description, String>,
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
            },
            open: open::<C, NoHistory>,
            plain: open::<C, NoHistory>,
            blank: || C::default().encode(),
            merge: merge::<C>,
            describe: describe_bytes::<C>,
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

struct State {
    replicas: BTreeMap<ReplicaId, Box<dyn AnyReplica>>,
    epochs: BTreeMap<ReplicaId, u64>,
    now: u64,
    wake: Option<u64>,
    sleep: Option<Pending<()>>,
    waiting: BTreeMap<RequestId, (ReplicaId, Sender<Answer>)>,
    closing: BTreeMap<ReplicaId, Sender<()>>,
    requests: RequestId,
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
                }),
                started: Cell::new(false),
            }),
        }
    }

    pub fn executor(&self) -> Executor {
        block_type_api::guest::executor(self.clone())
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

    fn answer_later<T: serde::de::DeserializeOwned + 'static>(
        &self,
        replica: ReplicaId,
        pending: Pending<T>,
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
        state.replicas.retain(|_, replica| !replica.released());
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
                ops::with_parent(replica, || host::send(to, bytes));
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
                        Ok(PublishOutcome::Published(head)) => HeadEvent::Published { token, head },
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
            Effect::Read {
                replica,
                token,
                commit,
            } => {
                let pending = ops::with_parent(replica, || host::read(commit, None));
                self.answer_later(replica, pending, move |result| Input::Read {
                    token,
                    result: result.and_then(|read| read),
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
            } => ops::with_parent(replica, || host::claim_lease(generation)),
            Effect::ReportClean {
                replica,
                generation,
                clean_at,
            } => ops::with_parent(replica, || host::report_clean(generation, clean_at)),
            Effect::Release { replica } => {
                let closing = self.shared.state.borrow_mut().closing.remove(&replica);
                ops::end(replica);
                if let Some(closing) = closing {
                    closing.send(());
                }
            }
            Effect::Status { replica, status } => {
                ops::with_parent(replica, || host::report_status(status));
            }
            Effect::Described {
                replica,
                description,
            } => ops::with_parent(replica, || host::described(description)),
            Effect::History {
                replica,
                can_undo,
                can_redo,
            } => ops::with_parent(replica, || host::history(can_undo, can_redo)),
            Effect::Conflict {
                replica,
                ours,
                theirs,
                count,
            } => ops::with_parent(replica, || {
                host::conflict(ours, theirs, count.try_into().unwrap_or(u32::MAX));
            }),
            Effect::Error { replica, message } => {
                ops::with_parent(replica, || host::error(message));
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

    fn start(&self, entropy: Vec<u8>) -> Vec<TypeInfo> {
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
        self.types()
    }

    fn blank(&self, content_type: Uuid) -> Result<Vec<u8>, Failure> {
        self.with_kind(content_type, |kind| (kind.blank)())
            .ok_or(Failure::UnknownType)
    }

    fn merge3(
        &self,
        content_type: Uuid,
        base: Option<Vec<u8>>,
        ours: Option<Vec<u8>>,
        theirs: Option<Vec<u8>>,
    ) -> Result<Merged, Failure> {
        self.with_kind(content_type, |kind| {
            (kind.merge)(base.as_deref(), ours.as_deref(), theirs.as_deref())
        })
        .ok_or(Failure::UnknownType)?
        .map(|(bytes, conflicts)| Merged {
            bytes,
            conflicts: conflicts.try_into().unwrap_or(u32::MAX),
        })
        .map_err(Failure::Malformed)
    }

    fn describe(
        &self,
        content_type: Uuid,
        bytes: Vec<u8>,
        organization: Uuid,
    ) -> Result<Description, Failure> {
        self.with_kind(content_type, |kind| (kind.describe)(&bytes, organization))
            .ok_or(Failure::UnknownType)?
            .map_err(Failure::Malformed)
    }

    fn to_file(&self, content_type: Uuid, bytes: Vec<u8>) -> Result<Option<PlainFile>, Failure> {
        match self.with_kind(content_type, |kind| {
            kind.to_file.map(|to_file| to_file(&bytes))
        }) {
            None => Err(Failure::UnknownType),
            Some(None) => Ok(None),
            Some(Some(file)) => file.map_err(Failure::Malformed),
        }
    }

    fn claims(
        &self,
        name: String,
        first: Vec<u8>,
        directory: bool,
        fallback: bool,
    ) -> Option<Uuid> {
        self.shared
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
            .map(|kind| kind.info.content_type)
    }

    fn apply(
        &self,
        content_type: Uuid,
        bytes: Option<Vec<u8>>,
        client: ClientId,
        change: Change,
    ) -> Result<Option<Vec<u8>>, Failure> {
        self.with_kind(content_type, |kind| {
            apply(kind, bytes.as_deref(), client, change)
        })
        .ok_or(Failure::UnknownType)?
    }
}

impl replicas::GuestReplica for OpenReplica {
    fn reopen(&self, opening: Opening) {
        self.module.open(self.id, opening);
    }

    fn message(&self, from: Endpoint, bytes: Vec<u8>) {
        self.module.input(self.id, Input::Message { from, bytes });
    }

    fn attach(&self, editor: EditorId, editable: bool) {
        self.module
            .input(self.id, Input::Attach { editor, editable });
    }

    fn detach(&self, editor: EditorId) {
        self.module.input(self.id, Input::Detach { editor });
    }

    fn head_moved(&self, head: CommitId, from_this_device: bool) {
        self.module.input(
            self.id,
            Input::Head(HeadEvent::Moved {
                head,
                from_this_device,
            }),
        );
    }

    fn lease(&self, session: SessionState, resynchronize: bool) {
        self.module.input(
            self.id,
            Input::Lease {
                session,
                resynchronize,
            },
        );
    }

    fn claim(&self) {
        self.module.input(self.id, Input::Claim);
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
                .enter(|| host::error(format!("a frame did not decode: {error}")));
            let _ = self.executor.step(Vec::new(), outbound);
        }
    }
}

#[macro_export]
macro_rules! type_module {
    ($guest:ty) => {
        $crate::api::export_type_module!();
        $crate::abi::guest!($guest, __register_type_module);
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
