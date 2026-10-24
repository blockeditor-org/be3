use std::{
    collections::{HashMap, VecDeque},
    convert::Infallible,
    sync::{Arc, LazyLock, RwLock, atomic::AtomicBool},
    time::{Duration, Instant},
};

use be_block::{
    CheckoutContent, DisplaySettingsContent, EditorViewContent, InputRemapperContent,
    InputSettingsContent, RepositoryContent, SettingsContent, SettingsProfileContent,
};
use block_guest_abi::{Flow, Guest, Waker};
use block_guest_host::{Channel, InProcess, Wake, Worker};
use block_type_api::{
    Command, Description, Event, Reply, Request, RequestId, SessionTag, TypeInfo, decode_frame,
    encode_frame,
};
use block_type_guest::{Module as TypeModule, Registration};
use tokio::sync::Notify;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) struct Kind {
    pub(crate) content_type: Uuid,
    pub(crate) frozen: bool,
    pub(crate) schema: String,
    pub(crate) module: usize,
    pub(crate) blank: Vec<u8>,
    pub(crate) described: Description,
}

static REGISTRY: LazyLock<RwLock<HashMap<Uuid, Kind>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub(crate) fn kind_of(content_type: Uuid) -> Option<Kind> {
    REGISTRY
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&content_type)
        .cloned()
}

pub(crate) fn is_known(content_type: Uuid) -> bool {
    REGISTRY
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains_key(&content_type)
}

fn register(kind: Kind) {
    REGISTRY
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(kind.content_type, kind);
}

static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);

pub(crate) fn millis(at: Instant) -> u64 {
    at.saturating_duration_since(*EPOCH).as_millis() as u64
}

pub(crate) fn now_millis() -> u64 {
    millis(Instant::now())
}

pub(crate) struct HostTypes(TypeModule);

pub(crate) fn host_types() -> TypeModule {
    TypeModule::new(vec![
        Registration::of::<DisplaySettingsContent>(),
        Registration::of::<InputSettingsContent>(),
        Registration::of::<InputRemapperContent>(),
        Registration::of::<SettingsContent>().undo::<SettingsContent>(),
        Registration::of::<SettingsProfileContent>().undo::<SettingsProfileContent>(),
        Registration::of::<EditorViewContent>(),
        Registration::of::<RepositoryContent>(),
        Registration::of::<CheckoutContent>(),
    ])
}

impl Guest for HostTypes {
    fn start(_: Waker, outbound: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        let module = host_types();
        outbound.push(module.hello_frame());
        Ok(Self(module))
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        self.0.step(inbound, outbound);
        Ok(Flow::Running)
    }
}

type Boxed = Box<dyn Channel<Presented = Infallible>>;

pub(super) type Started = Result<(Boxed, SessionTag), String>;

pub(super) type Start = Box<dyn Fn(Wake, Arc<AtomicBool>) -> Started>;

pub(super) struct Source {
    pub(super) name: &'static str,
    pub(super) start: Start,
}

fn tag_of(bytes: &[u8]) -> SessionTag {
    SessionTag(*be_store::Hash::of(bytes).as_bytes())
}

pub(super) fn in_process<G: Guest + 'static>(name: &'static str) -> Source {
    Source {
        name,
        start: Box::new(move |wake, waiting| {
            let tag = tag_of(format!("{name} {}", build_id::current().unwrap_or_default()).as_bytes());
            Ok((Box::new(InProcess::<G>::start(wake, waiting)) as Boxed, tag))
        }),
    }
}

pub(super) const BUILTIN: &str = "builtin-types";

pub(super) fn sources() -> Vec<Source> {
    vec![builtin(), in_process::<HostTypes>("host-types")]
}

#[cfg(test)]
fn builtin() -> Source {
    in_process::<builtin_types::BlockTypes>(BUILTIN)
}

#[cfg(all(not(test), not(target_arch = "wasm32")))]
fn builtin() -> Source {
    Source {
        name: BUILTIN,
        start: Box::new(|wake, waiting| {
            let entry = format!("{BUILTIN}.wasm");
            let read = crate::editors::plugin::discovery::read;
            let tag = tag_of(&read(&entry)?);
            let host = block_wasm_host::PureHost::new(None)?;
            let channel = host.spawn(
                BUILTIN.to_owned(),
                move |host| host.load_from(&entry, read),
                wake,
                waiting,
            )?;
            Ok((Box::new(channel) as Boxed, tag))
        }),
    }
}

#[cfg(all(not(test), target_arch = "wasm32"))]
fn builtin() -> Source {
    Source {
        name: BUILTIN,
        start: Box::new(|wake, waiting| {
            let tag = tag_of(format!("{BUILTIN} {}", build_id::current().unwrap_or_default()).as_bytes());
            let channel =
                block_guest_host::web::WebGuest::start(&format!("{BUILTIN}.wasm"), wake, waiting)?;
            Ok((Box::new(channel) as Boxed, tag))
        }),
    }
}

struct Module {
    source: Source,
    worker: Option<Worker<Boxed>>,
    tag: SessionTag,
    ticked: Option<u64>,
    wake: Option<u64>,
    hello: bool,
    failure: Option<String>,
    generation: u64,
    restarts: usize,
}

pub(super) struct Modules {
    modules: Vec<Module>,
    notify: Arc<Notify>,
    requests: RequestId,
    backlog: VecDeque<(usize, Command)>,
    restarted: Vec<usize>,
}

const RESTART_LIMIT: usize = 8;

impl Modules {
    pub(super) fn new(sources: Vec<Source>) -> Self {
        let mut modules = Self {
            modules: sources
                .into_iter()
                .map(|source| Module {
                    source,
                    worker: None,
                    tag: SessionTag::default(),
                    ticked: None,
                    wake: None,
                    hello: false,
                    failure: None,
                    generation: 0,
                    restarts: 0,
                })
                .collect(),
            notify: Arc::new(Notify::new()),
            requests: 0,
            backlog: VecDeque::new(),
            restarted: Vec::new(),
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
        held.wake = None;
        match (held.source.start)(wake, Arc::clone(&waiting)) {
            Ok((channel, tag)) => {
                held.tag = tag;
                held.failure = None;
                let mut worker = Worker::new(channel, waiting);
                let seed = *Uuid::new_v4().as_bytes();
                let more = *Uuid::new_v4().as_bytes();
                let mut entropy = [0u8; 32];
                entropy[..16].copy_from_slice(&seed);
                entropy[16..].copy_from_slice(&more);
                let failed = worker.queue(vec![encode_frame(&Event::Entropy { seed: entropy })]);
                held.worker = Some(worker);
                if let Some(failure) = failed {
                    self.fail(module, failure);
                }
            }
            Err(error) => {
                eprintln!(
                    "block-app: the type module {} did not start: {error}",
                    held.source.name
                );
                held.failure = Some(error);
                held.worker = None;
            }
        }
    }

    fn fail(&mut self, module: usize, failure: String) {
        let held = &mut self.modules[module];
        eprintln!(
            "block-app: the type module {} stopped: {failure}",
            held.source.name
        );
        held.worker = None;
        held.failure = Some(failure);
    }

    pub(super) fn tag(&self, module: usize) -> SessionTag {
        self.modules[module].tag
    }

    pub(super) fn notified(&self) -> impl Future<Output = ()> + 'static {
        let notify = Arc::clone(&self.notify);
        async move { notify.notified().await }
    }

    pub(super) async fn ready(&mut self) {
        loop {
            for (module, command) in self.collect() {
                if let Command::Hello { types } = command {
                    self.hello(module, types).await;
                } else {
                    self.backlog.push_back((module, command));
                }
            }
            if self
                .modules
                .iter()
                .all(|module| module.hello || module.worker.is_none())
            {
                return;
            }
            self.notified().await;
        }
    }

    async fn hello(&mut self, module: usize, types: Vec<TypeInfo>) {
        self.modules[module].hello = true;
        for info in types {
            if kind_of(info.content_type).is_some_and(|kind| kind.module == module) {
                continue;
            }
            let blank = match self
                .request(
                    module,
                    Request::Blank {
                        content_type: info.content_type,
                    },
                )
                .await
            {
                Reply::Blank(bytes) => bytes,
                _ => continue,
            };
            let described = match self
                .request(
                    module,
                    Request::Describe {
                        content_type: info.content_type,
                        bytes: blank.clone(),
                        organization: Uuid::nil(),
                    },
                )
                .await
            {
                Reply::Described(description) => description,
                _ => Description::default(),
            };
            register(Kind {
                content_type: info.content_type,
                frozen: info.frozen,
                schema: info.schema,
                module,
                blank,
                described,
            });
        }
    }

    pub(super) fn send(&mut self, module: usize, events: Vec<Event>) {
        let now = now_millis();
        let held = &mut self.modules[module];
        let Some(worker) = held.worker.as_mut() else {
            return;
        };
        let mut frames = Vec::with_capacity(events.len() + 1);
        if held.ticked.is_none_or(|ticked| ticked < now) {
            held.ticked = Some(now);
            frames.push(encode_frame(&Event::Tick { now }));
        }
        frames.extend(events.iter().map(encode_frame));
        if let Some(failure) = worker.queue(frames) {
            self.fail(module, failure);
            self.restart(module);
        }
    }

    pub(super) fn tick_at(&mut self, module: usize, at: Instant) {
        let now = millis(at);
        let held = &mut self.modules[module];
        if held.ticked.is_some_and(|ticked| ticked >= now) {
            return;
        }
        held.ticked = Some(now);
        if let Some(worker) = held.worker.as_mut()
            && let Some(failure) = worker.queue(vec![encode_frame(&Event::Tick { now })])
        {
            self.fail(module, failure);
            self.restart(module);
        }
    }

    fn restart(&mut self, module: usize) {
        self.backlog.retain(|(from, _)| *from != module);
        let held = &mut self.modules[module];
        held.restarts += 1;
        if held.restarts > RESTART_LIMIT {
            eprintln!(
                "block-app: the type module {} stopped {RESTART_LIMIT} times and is not started again",
                held.source.name
            );
            return;
        }
        self.launch(module);
        if self.modules[module].worker.is_some() {
            self.restarted.push(module);
        }
    }

    pub(super) fn take_restarted(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.restarted)
    }

    pub(super) fn deadline(&mut self) -> Option<Duration> {
        let now = now_millis();
        let mut soonest: Option<Duration> = None;
        let mut hung = Vec::new();
        for (index, module) in self.modules.iter_mut().enumerate() {
            if let Some(wake) = module.wake {
                let due = Duration::from_millis(wake.saturating_sub(now));
                soonest = Some(soonest.map_or(due, |held| held.min(due)));
            }
            if let Some(worker) = module.worker.as_mut() {
                match worker.hung(Instant::now().saturating_duration_since(*EPOCH)) {
                    Ok(Some(left)) => soonest = Some(soonest.map_or(left, |held| held.min(left))),
                    Ok(None) => {}
                    Err(failure) => hung.push((index, failure)),
                }
            }
        }
        for (module, failure) in hung {
            self.fail(module, failure);
            self.restart(module);
            soonest = Some(Duration::ZERO);
        }
        soonest
    }

    pub(super) fn tick_due(&mut self) {
        let now = now_millis();
        for module in 0..self.modules.len() {
            if self.modules[module]
                .wake
                .is_some_and(|wake| wake <= now)
            {
                self.modules[module].wake = None;
                self.send(module, Vec::new());
            }
        }
    }

    fn collect(&mut self) -> Vec<(usize, Command)> {
        let mut commands: Vec<(usize, Command)> = self.backlog.drain(..).collect();
        for module in 0..self.modules.len() {
            let Some(worker) = self.modules[module].worker.as_mut() else {
                continue;
            };
            let failure = worker.poll();
            let received = worker.take_received();
            for frame in received {
                match decode_frame::<Command>(&frame) {
                    Ok(Command::WakeAt { at }) => self.modules[module].wake = Some(at),
                    Ok(command) => commands.push((module, command)),
                    Err(error) => eprintln!(
                        "block-app: the type module {} sent a frame that did not decode: {error}",
                        self.modules[module].source.name
                    ),
                }
            }
            if let Some(failure) = failure {
                self.fail(module, failure);
                self.restart(module);
            }
        }
        commands
    }

    pub(super) fn take(&mut self) -> Vec<(usize, Command)> {
        self.collect()
    }

    pub(super) fn next_request(&mut self) -> RequestId {
        self.requests += 1;
        self.requests
    }

    pub(super) async fn request(&mut self, module: usize, request: Request) -> Reply {
        let id = self.next_request();
        let generation = self.modules[module].generation;
        self.send(module, vec![Event::Request { id, request }]);
        loop {
            let held = &self.modules[module];
            if held.worker.is_none() || held.generation != generation {
                return Reply::Unknown;
            }
            let mut answered = None;
            for (from, command) in self.collect() {
                match command {
                    Command::Reply { id: replied, reply } if from == module && replied == id => {
                        answered = Some(reply);
                    }
                    command => self.backlog.push_back((from, command)),
                }
            }
            if let Some(reply) = answered {
                return reply;
            }
            if self.modules[module].generation != generation {
                return Reply::Unknown;
            }
            self.notified().await;
        }
    }
}
