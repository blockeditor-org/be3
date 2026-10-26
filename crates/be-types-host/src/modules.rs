use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    convert::Infallible,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use be_client::Clock;

use block_guest_abi::Guest;
use block_guest_host::{Channel, InProcess, Wake, Worker};
use block_type_api::ops::DeserializeOwned;
use block_type_api::{
    Change, ClientId, Description, Failure, Plain, PlainFile, SessionTag, TypeInfo,
    embed::{exports::replicas, host::Call},
    ops::{Connection, Incoming, Latest, OpError, OpId, Outcome, Reply, Side, Ticket},
};
use tokio::sync::Notify;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Kind {
    pub content_type: Uuid,
    pub frozen: bool,
    pub schema: String,
    pub plain: Plain,
    pub blank: Vec<u8>,
    pub described: Description,
}

static REGISTRY: wasi_threads::Mutex<BTreeMap<Uuid, Kind>> =
    wasi_threads::Mutex::new(BTreeMap::new());

pub fn kind_of(content_type: Uuid) -> Option<Kind> {
    REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&content_type)
        .cloned()
}

pub fn is_known(content_type: Uuid) -> bool {
    REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains_key(&content_type)
}

static PLAIN_FORMS: AtomicU64 = AtomicU64::new(0);

pub fn register(kind: Kind) {
    let plain = kind.plain;
    let before = REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(kind.content_type, kind);
    if before.is_none_or(|before| before.plain != plain) {
        PLAIN_FORMS.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn plain_forms_revision() -> u64 {
    PLAIN_FORMS.load(Ordering::Relaxed)
}

static HELD: wasi_threads::Mutex<Option<Instant>> = wasi_threads::Mutex::new(None);

pub fn hold_clock(at: Option<Instant>) {
    *HELD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = at;
}

fn held() -> Option<Instant> {
    *HELD
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub type Boxed = Box<dyn Channel<Presented = Infallible>>;

pub type Started = Result<(Boxed, SessionTag), String>;

pub type Start = Box<dyn Fn(Wake, Arc<AtomicBool>) -> Started>;

pub struct Source {
    pub name: String,
    pub declared: Option<Vec<Uuid>>,
    pub start: Start,
}

impl Source {
    pub fn declaring(mut self, types: Vec<Uuid>) -> Self {
        self.declared = Some(types);
        self
    }
}

pub fn tag_of(bytes: &[u8]) -> SessionTag {
    SessionTag(*be_store::Hash::of(bytes).as_bytes())
}

pub fn in_process<G: Guest + 'static>(name: impl Into<String>) -> Source {
    let name = name.into();
    Source {
        name: name.clone(),
        declared: None,
        start: Box::new(move |wake, waiting| {
            let tag =
                tag_of(format!("{name} {}", build_id::current().unwrap_or_default()).as_bytes());
            Ok((Box::new(InProcess::<G>::start(wake, waiting)) as Boxed, tag))
        }),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn wasm(
    name: impl Into<String>,
    tag: Result<SessionTag, String>,
    cache: Option<std::path::PathBuf>,
    load: impl Fn(&block_wasm_host::PureHost) -> Result<block_wasm_host::PureGuest, String>
    + Clone
    + Send
    + 'static,
) -> Source {
    let name = name.into();
    Source {
        name: name.clone(),
        declared: None,
        start: Box::new(move |wake, waiting| {
            let tag = tag.clone()?;
            let load = load.clone();
            let host = block_wasm_host::PureHost::new(cache.as_deref())?
                .with_imports(block_type_api::embed::IMPORTS);
            let channel = host.spawn(name.clone(), move |host| load(host), wake, waiting)?;
            Ok((Box::new(channel) as Boxed, tag))
        }),
    }
}

#[cfg(target_arch = "wasm32")]
pub fn web(name: impl Into<String>, url: String, tag: Result<SessionTag, String>) -> Source {
    Source {
        name: name.into(),
        declared: None,
        start: Box::new(move |wake, waiting| {
            let tag = tag.clone()?;
            let channel = block_guest_host::web::WebGuest::start(
                &url,
                block_type_api::embed::IMPORTS,
                wake,
                waiting,
            )?;
            Ok((Box::new(channel) as Boxed, tag))
        }),
    }
}

struct Module {
    source: Source,
    worker: Option<Worker<Boxed>>,
    connection: Connection,
    tag: SessionTag,
    clock: Option<Latest<u64>>,
    ticked: Option<u64>,
    sleeping: Option<(u64, Reply<()>)>,
    started: Option<Ticket<Vec<TypeInfo>>>,
    hello: bool,
    failure: Option<String>,
    generation: u64,
    restarts: usize,
}

#[derive(Debug)]
pub enum Heard {
    Hello(Vec<TypeInfo>),
    Call {
        op: Option<OpId>,
        parent: Option<OpId>,
        call: Call,
    },
    Completed {
        op: OpId,
        outcome: Outcome,
    },
    Canceled {
        op: OpId,
    },
}

pub struct Modules {
    modules: Vec<Module>,
    notify: Arc<Notify>,
    backlog: VecDeque<(usize, Heard)>,
    restarted: Vec<usize>,
    routes: HashMap<Uuid, usize>,
    clock: Clock,
}

const RESTART_LIMIT: usize = 8;

const CLAIMED_BYTES: usize = 64 * 1024;

fn unknown(content_type: Uuid) -> String {
    format!("no type module has the block type {content_type}")
}

fn failed(content_type: Uuid, failure: Failure) -> String {
    match failure {
        Failure::UnknownType => unknown(content_type),
        Failure::Malformed(error) => error,
    }
}

pub struct Claim {
    name: String,
    first: Vec<u8>,
    directory: bool,
    fallback: bool,
}

impl Claim {
    pub fn call(self, connection: &mut Connection) -> Ticket<Option<Uuid>> {
        replicas::claims(
            connection,
            self.name,
            self.first,
            self.directory,
            self.fallback,
        )
    }
}

impl Modules {
    pub fn new(sources: Vec<Source>, clock: Clock) -> Self {
        let mut modules = Self {
            modules: sources
                .into_iter()
                .map(|source| Module {
                    source,
                    worker: None,
                    connection: Connection::new(Side::Host),
                    tag: SessionTag::default(),
                    clock: None,
                    ticked: None,
                    sleeping: None,
                    started: None,
                    hello: false,
                    failure: None,
                    generation: 0,
                    restarts: 0,
                })
                .collect(),
            notify: Arc::new(Notify::new()),
            backlog: VecDeque::new(),
            restarted: Vec::new(),
            routes: HashMap::new(),
            clock,
        };
        for module in 0..modules.modules.len() {
            modules.launch(module);
        }
        modules
    }

    fn launch(&mut self, module: usize) {
        let notify = Arc::clone(&self.notify);
        let wake: Wake = Arc::new(move || notify.notify_one());
        let waiting = Arc::new(AtomicBool::new(false));
        let held = &mut self.modules[module];
        held.generation += 1;
        held.hello = false;
        held.ticked = None;
        held.clock = None;
        held.sleeping = None;
        held.connection = Connection::new(Side::Host);
        match (held.source.start)(wake, Arc::clone(&waiting)) {
            Ok((channel, tag)) => {
                held.tag = tag;
                held.failure = None;
                held.worker = Some(Worker::new(channel, waiting));
                let mut entropy = Uuid::new_v4().as_bytes().to_vec();
                entropy.extend_from_slice(Uuid::new_v4().as_bytes());
                held.started = Some(replicas::start(&mut held.connection, entropy));
                self.flush(module);
            }
            Err(error) => {
                eprintln!(
                    "the type module {} did not start: {error}",
                    held.source.name
                );
                held.failure = Some(error);
                held.worker = None;
            }
        }
    }

    fn fail(&mut self, module: usize, failure: String) {
        let held = &mut self.modules[module];
        eprintln!("the type module {} stopped: {failure}", held.source.name);
        held.worker = None;
        held.failure = Some(failure);
    }

    fn flush(&mut self, module: usize) {
        let held = &mut self.modules[module];
        let frames = held.connection.take_outbound();
        let Some(worker) = held.worker.as_mut() else {
            return;
        };
        if frames.is_empty() {
            return;
        }
        if let Some(failure) = worker.queue(frames) {
            self.fail(module, failure);
            self.restart(module);
        }
    }

    pub fn module_of(&self, content_type: Uuid) -> Option<usize> {
        self.routes.get(&content_type).copied()
    }

    pub fn settled(&self) -> bool {
        self.backlog.is_empty()
            && self
                .modules
                .iter()
                .all(|module| module.worker.as_ref().is_none_or(Worker::settled))
    }

    pub fn tag(&self, module: usize) -> SessionTag {
        self.modules[module].tag
    }

    pub fn generation(&self, module: usize) -> u64 {
        self.modules[module].generation
    }

    pub fn notified(&self) -> impl Future<Output = ()> + 'static {
        let notify = Arc::clone(&self.notify);
        async move { notify.notified().await }
    }

    pub async fn ready(&mut self) {
        let mut kept = Vec::new();
        loop {
            let collected = self.collect();
            let heard = !collected.is_empty();
            for (module, heard) in collected {
                if let Heard::Hello(types) = heard {
                    self.hello(module, types).await;
                } else {
                    kept.push((module, heard));
                }
            }
            if self
                .modules
                .iter()
                .all(|module| module.hello || module.worker.is_none())
            {
                kept.extend(self.backlog.drain(..));
                self.backlog = kept.into();
                return;
            }
            if !heard {
                self.notified().await;
            }
        }
    }

    async fn hello(&mut self, module: usize, types: Vec<TypeInfo>) {
        self.modules[module].hello = true;
        let types: Vec<TypeInfo> = types
            .into_iter()
            .filter(|info| self.provides(module, info.content_type))
            .filter(|info| self.routes.get(&info.content_type) != Some(&module))
            .collect();
        let blanks = self
            .ask_all(
                module,
                types
                    .iter()
                    .map(|info| {
                        let content_type = info.content_type;
                        move |connection: &mut Connection| replicas::blank(connection, content_type)
                    })
                    .collect(),
            )
            .await;
        let mut blank = Vec::new();
        for (info, reply) in types.into_iter().zip(blanks) {
            if let Ok(Ok(bytes)) = reply {
                blank.push((info, bytes));
            }
        }
        let described = self
            .ask_all(
                module,
                blank
                    .iter()
                    .map(|(info, bytes)| {
                        let (content_type, bytes) = (info.content_type, bytes.clone());
                        move |connection: &mut Connection| {
                            replicas::describe(connection, content_type, bytes, Uuid::nil())
                        }
                    })
                    .collect(),
            )
            .await;
        for ((info, blank), described) in blank.into_iter().zip(described) {
            self.routes.insert(info.content_type, module);
            register(Kind {
                content_type: info.content_type,
                frozen: info.frozen,
                schema: info.schema,
                plain: info.plain,
                blank,
                described: match described {
                    Ok(Ok(description)) => description,
                    _ => Description::default(),
                },
            });
        }
    }

    fn provides(&self, module: usize, content_type: Uuid) -> bool {
        let declares = |held: &Module| {
            held.source
                .declared
                .as_ref()
                .map(|declared| declared.contains(&content_type))
        };
        match declares(&self.modules[module]) {
            Some(declared) => declared,
            None => !self
                .modules
                .iter()
                .enumerate()
                .any(|(other, held)| other != module && declares(held) == Some(true)),
        }
    }

    fn advance(&mut self, module: usize, now: u64) {
        let held = &mut self.modules[module];
        if held.ticked.is_some_and(|ticked| ticked >= now) {
            return;
        }
        held.ticked = Some(now);
        if let Some(clock) = &mut held.clock {
            clock.send(&mut held.connection, now);
        }
        if held.sleeping.as_ref().is_some_and(|(at, _)| *at <= now)
            && let Some((_, reply)) = held.sleeping.take()
        {
            reply.send(&mut held.connection, &());
        }
    }

    pub fn call<R>(&mut self, module: usize, call: impl FnOnce(&mut Connection) -> R) -> Option<R> {
        self.modules[module].worker.as_ref()?;
        let now = self.now();
        self.advance(module, now);
        let result = call(&mut self.modules[module].connection);
        self.flush(module);
        Some(result)
    }

    pub fn tick_at(&mut self, module: usize, at: Instant) {
        if self.modules[module].worker.is_none() {
            return;
        }
        let now = self.clock.at(at).as_millis() as u64;
        self.advance(module, now);
        self.flush(module);
    }

    fn restart(&mut self, module: usize) {
        self.backlog.retain(|(from, _)| *from != module);
        let held = &mut self.modules[module];
        held.restarts += 1;
        if held.restarts > RESTART_LIMIT {
            eprintln!(
                "the type module {} stopped {RESTART_LIMIT} times and is not started again",
                held.source.name
            );
            return;
        }
        self.launch(module);
        if self.modules[module].worker.is_some() {
            self.restarted.push(module);
        }
    }

    pub fn take_restarted(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.restarted)
    }

    pub fn now(&self) -> u64 {
        held()
            .map_or_else(|| self.clock.now(), |at| self.clock.at(at))
            .as_millis() as u64
    }

    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    pub fn wake_at(&mut self) -> Option<Duration> {
        let now = self.clock.now();
        let mut soonest: Option<Duration> = None;
        let mut hung = Vec::new();
        for (index, module) in self.modules.iter_mut().enumerate() {
            if let Some((wake, _)) = &module.sleeping {
                let due = Duration::from_millis(*wake);
                soonest = Some(soonest.map_or(due, |held| held.min(due)));
            }
            if let Some(worker) = module.worker.as_mut() {
                match worker.hung(now) {
                    Ok(Some(left)) => {
                        soonest = Some(soonest.map_or(now + left, |held| held.min(now + left)));
                    }
                    Ok(None) => {}
                    Err(failure) => hung.push((index, failure)),
                }
            }
        }
        for (module, failure) in hung {
            self.fail(module, failure);
            self.restart(module);
            soonest = Some(now);
        }
        soonest
    }

    pub fn tick_due(&mut self) {
        let now = self.now();
        for module in 0..self.modules.len() {
            if self.modules[module]
                .sleeping
                .as_ref()
                .is_some_and(|(at, _)| *at <= now)
            {
                self.call(module, |_| ());
            }
        }
    }

    fn collect(&mut self) -> Vec<(usize, Heard)> {
        let clock_now = self.now();
        let mut heard: Vec<(usize, Heard)> = self.backlog.drain(..).collect();
        for module in 0..self.modules.len() {
            let Some(worker) = self.modules[module].worker.as_mut() else {
                continue;
            };
            let failure = worker.poll();
            let received = worker.take_received();
            let held = &mut self.modules[module];
            for frame in received {
                if let Err(error) = held.connection.receive(&frame) {
                    eprintln!(
                        "the type module {} sent a frame that did not decode: {error}",
                        held.source.name
                    );
                }
            }
            while let Some(event) = held.connection.next_event() {
                match event {
                    Incoming::Started(started) => {
                        let (op, parent) = (started.op, started.parent);
                        match Call::accept(&mut held.connection, started) {
                            Some(Call::Clock { stream }) => {
                                let now = clock_now;
                                held.ticked = Some(now);
                                let mut clock = Latest::new(stream);
                                clock.send(&mut held.connection, now);
                                held.clock = Some(clock);
                            }
                            Some(Call::SleepUntil { reply, at }) => {
                                if let Some((_, before)) = held.sleeping.replace((at, reply)) {
                                    before.fail(&mut held.connection, OpError::Canceled);
                                }
                            }
                            Some(call) => heard.push((
                                module,
                                Heard::Call {
                                    op: Some(op),
                                    parent,
                                    call,
                                },
                            )),
                            None => {}
                        }
                    }
                    Incoming::Completed { op, outcome } => {
                        match held.started.filter(|started| started.op() == op) {
                            Some(started) => {
                                held.started = None;
                                let types = started.decode(outcome).unwrap_or_default();
                                heard.push((module, Heard::Hello(types)));
                            }
                            None => heard.push((module, Heard::Completed { op, outcome })),
                        }
                    }
                    Incoming::Canceled { op } => {
                        if held
                            .sleeping
                            .as_ref()
                            .is_some_and(|(_, reply)| reply.op() == op)
                            && let Some((_, reply)) = held.sleeping.take()
                        {
                            reply.fail(&mut held.connection, OpError::Canceled);
                        } else if held.clock.as_ref().is_some_and(|clock| clock.op() == op)
                            && let Some(clock) = held.clock.take()
                        {
                            clock.finish(&mut held.connection);
                        } else {
                            heard.push((module, Heard::Canceled { op }));
                        }
                    }
                    Incoming::Called(called) => {
                        if let Some(call) = Call::accept_direct(&called) {
                            heard.push((
                                module,
                                Heard::Call {
                                    op: None,
                                    parent: called.parent,
                                    call,
                                },
                            ));
                        }
                    }
                    Incoming::Credited { op } => {
                        if let Some(clock) = &mut held.clock {
                            clock.credited(&mut held.connection, op);
                        }
                    }
                    Incoming::Yielded { .. } => {}
                }
            }
            self.flush(module);
            if let Some(failure) = failure {
                self.fail(module, failure);
                self.restart(module);
            }
        }
        heard
    }

    pub fn take(&mut self) -> Vec<(usize, Heard)> {
        self.collect()
    }

    pub async fn ask_all<T: DeserializeOwned>(
        &mut self,
        module: usize,
        calls: Vec<impl FnOnce(&mut Connection) -> Ticket<T>>,
    ) -> Vec<Result<T, OpError>> {
        let generation = self.modules[module].generation;
        let tickets: Vec<Ticket<T>> = match self.call(module, |connection| {
            calls
                .into_iter()
                .map(|call| call(connection))
                .collect::<Vec<_>>()
        }) {
            Some(tickets) => tickets,
            None => return Vec::new(),
        };
        let mut answers: HashMap<OpId, Outcome> = HashMap::new();
        while answers.len() < tickets.len() {
            let held = &self.modules[module];
            if held.worker.is_none() || held.generation != generation {
                break;
            }
            let mut answered = false;
            for (from, heard) in self.collect() {
                match heard {
                    Heard::Completed { op, outcome }
                        if from == module && tickets.iter().any(|ticket| ticket.op() == op) =>
                    {
                        answers.insert(op, outcome);
                        answered = true;
                    }
                    heard => self.backlog.push_back((from, heard)),
                }
            }
            if !answered && answers.len() < tickets.len() {
                self.notified().await;
            }
        }
        tickets
            .into_iter()
            .map(|ticket| match answers.remove(&ticket.op()) {
                Some(outcome) => ticket.decode(outcome),
                None => Err(OpError::Canceled),
            })
            .collect()
    }

    pub async fn ask<T: DeserializeOwned>(
        &mut self,
        module: usize,
        call: impl FnOnce(&mut Connection) -> Ticket<T>,
    ) -> Result<T, OpError> {
        self.ask_all(module, vec![call])
            .await
            .pop()
            .unwrap_or(Err(OpError::Canceled))
    }

    async fn ask_type<T: DeserializeOwned>(
        &mut self,
        content_type: Uuid,
        call: impl FnOnce(&mut Connection) -> Ticket<Result<T, Failure>>,
    ) -> Result<T, String> {
        let Some(module) = self.module_of(content_type) else {
            return Err(unknown(content_type));
        };
        match self.ask(module, call).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(failure)) => Err(failed(content_type, failure)),
            Err(error) => Err(format!("the type module answered: {error}")),
        }
    }

    pub async fn blank(&mut self, content_type: Uuid) -> Result<Vec<u8>, String> {
        self.ask_type(content_type, |connection| {
            replicas::blank(connection, content_type)
        })
        .await
    }

    pub async fn merge3(
        &mut self,
        content_type: Uuid,
        base: Option<Vec<u8>>,
        ours: Option<Vec<u8>>,
        theirs: Option<Vec<u8>>,
    ) -> Result<(Vec<u8>, bool), String> {
        self.ask_type(content_type, |connection| {
            replicas::merge3(connection, content_type, base, ours, theirs)
        })
        .await
        .map(|merged| (merged.bytes, merged.conflicts > 0))
    }

    pub async fn to_files(
        &mut self,
        blocks: Vec<(Uuid, Vec<u8>)>,
    ) -> Vec<Result<Option<PlainFile>, String>> {
        let mut answers: Vec<Result<Option<PlainFile>, String>> = Vec::with_capacity(blocks.len());
        let mut asked: HashMap<usize, Vec<(usize, Uuid, Vec<u8>)>> = HashMap::new();
        for (index, (content_type, bytes)) in blocks.into_iter().enumerate() {
            answers.push(Err(unknown(content_type)));
            if let Some(module) = self.module_of(content_type) {
                asked
                    .entry(module)
                    .or_default()
                    .push((index, content_type, bytes));
            }
        }
        for (module, blocks) in asked {
            let indices: Vec<(usize, Uuid)> = blocks
                .iter()
                .map(|(index, content_type, _)| (*index, *content_type))
                .collect();
            let calls = blocks
                .into_iter()
                .map(|(_, content_type, bytes)| {
                    move |connection: &mut Connection| {
                        replicas::to_file(connection, content_type, bytes)
                    }
                })
                .collect();
            for ((index, content_type), reply) in
                indices.into_iter().zip(self.ask_all(module, calls).await)
            {
                answers[index] = match reply {
                    Ok(Ok(file)) => Ok(file),
                    Ok(Err(failure)) => Err(failed(content_type, failure)),
                    Err(error) => Err(format!("the type module answered: {error}")),
                };
            }
        }
        answers
    }

    pub fn claim_requests(&self, name: &str, first: &[u8], directory: bool) -> Vec<(usize, Claim)> {
        let first = &first[..first.len().min(CLAIMED_BYTES)];
        [false, true]
            .into_iter()
            .flat_map(|fallback| {
                (0..self.modules.len()).map(move |module| {
                    let claim = Claim {
                        name: name.to_owned(),
                        first: first.to_vec(),
                        directory,
                        fallback,
                    };
                    (module, claim)
                })
            })
            .collect()
    }

    pub async fn claims(&mut self, name: &str, first: &[u8], directory: bool) -> Option<Uuid> {
        for (module, claim) in self.claim_requests(name, first, directory) {
            if let Ok(Some(content_type)) =
                self.ask(module, |connection| claim.call(connection)).await
            {
                return Some(content_type);
            }
        }
        None
    }

    pub async fn apply(
        &mut self,
        content_type: Uuid,
        bytes: Option<Vec<u8>>,
        client: ClientId,
        change: Change,
    ) -> Result<Option<Vec<u8>>, String> {
        self.ask_type(content_type, |connection| {
            replicas::apply(connection, content_type, bytes, client, change)
        })
        .await
    }

    pub async fn describe(
        &mut self,
        content_type: Uuid,
        bytes: &[u8],
        organization: Uuid,
    ) -> Result<Description, String> {
        let bytes = bytes.to_vec();
        self.ask_type(content_type, |connection| {
            replicas::describe(connection, content_type, bytes, organization)
        })
        .await
    }
}

pub fn plain_of(content_type: Uuid) -> Plain {
    kind_of(content_type).map_or(Plain::None, |kind| kind.plain)
}

pub async fn started(sources: Vec<Source>, clock: Clock) -> Modules {
    let mut modules = Modules::new(sources, clock);
    modules.ready().await;
    modules
}

#[cfg(test)]
mod tests;
