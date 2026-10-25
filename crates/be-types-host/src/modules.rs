use std::{
    collections::{HashMap, VecDeque},
    convert::Infallible,
    sync::{
        Arc, LazyLock, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use be_client::Clock;

use block_guest_abi::Guest;
use block_guest_host::{Channel, InProcess, Wake, Worker};
use block_type_api::{
    Change, ClientId, Command, Description, Event, Plain, PlainFile, Reply, Request, RequestId,
    SessionTag, TypeInfo, decode_frame, encode_frame,
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

static REGISTRY: LazyLock<RwLock<HashMap<Uuid, Kind>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn kind_of(content_type: Uuid) -> Option<Kind> {
    REGISTRY
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&content_type)
        .cloned()
}

pub fn is_known(content_type: Uuid) -> bool {
    REGISTRY
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains_key(&content_type)
}

static PLAIN_FORMS: AtomicU64 = AtomicU64::new(0);

pub fn register(kind: Kind) {
    let plain = kind.plain;
    let before = REGISTRY
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(kind.content_type, kind);
    if before.is_none_or(|before| before.plain != plain) {
        PLAIN_FORMS.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn plain_forms_revision() -> u64 {
    PLAIN_FORMS.load(Ordering::Relaxed)
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
            let host = block_wasm_host::PureHost::new(cache.as_deref())?;
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
            let channel = block_guest_host::web::WebGuest::start(&url, wake, waiting)?;
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

pub struct Modules {
    modules: Vec<Module>,
    notify: Arc<Notify>,
    requests: RequestId,
    backlog: VecDeque<(usize, Command)>,
    restarted: Vec<usize>,
    routes: HashMap<Uuid, usize>,
    clock: Clock,
}

const RESTART_LIMIT: usize = 8;

impl Modules {
    pub fn new(sources: Vec<Source>, clock: Clock) -> Self {
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

    pub fn notified(&self) -> impl Future<Output = ()> + 'static {
        let notify = Arc::clone(&self.notify);
        async move { notify.notified().await }
    }

    pub async fn ready(&mut self) {
        let mut kept = Vec::new();
        loop {
            let collected = self.collect();
            let heard = !collected.is_empty();
            for (module, command) in collected {
                if let Command::Hello { types } = command {
                    self.hello(module, types).await;
                } else {
                    kept.push((module, command));
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
            .requests_all(
                module,
                types
                    .iter()
                    .map(|info| Request::Blank {
                        content_type: info.content_type,
                    })
                    .collect(),
            )
            .await;
        let mut blank = Vec::new();
        for (info, reply) in types.into_iter().zip(blanks) {
            if let Reply::Blank(bytes) = reply {
                blank.push((info, bytes));
            }
        }
        let described = self
            .requests_all(
                module,
                blank
                    .iter()
                    .map(|(info, bytes)| Request::Describe {
                        content_type: info.content_type,
                        bytes: bytes.clone(),
                        organization: Uuid::nil(),
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
                    Reply::Described(description) => description,
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

    pub fn now(&self) -> u64 {
        self.clock.now().as_millis() as u64
    }

    pub fn send(&mut self, module: usize, events: Vec<Event>) {
        let now = self.now();
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

    pub fn tick_at(&mut self, module: usize, at: Instant) {
        let now = self.clock.at(at).as_millis() as u64;
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

    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    pub fn wake_at(&mut self) -> Option<Duration> {
        let now = self.clock.now();
        let mut soonest: Option<Duration> = None;
        let mut hung = Vec::new();
        for (index, module) in self.modules.iter_mut().enumerate() {
            if let Some(wake) = module.wake {
                let due = Duration::from_millis(wake);
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
            if self.modules[module].wake.is_some_and(|wake| wake <= now) {
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
                        "the type module {} sent a frame that did not decode: {error}",
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

    pub fn take(&mut self) -> Vec<(usize, Command)> {
        self.collect()
    }

    pub fn next_request(&mut self) -> RequestId {
        self.requests += 1;
        self.requests
    }

    pub async fn requests_all(&mut self, module: usize, requests: Vec<Request>) -> Vec<Reply> {
        let generation = self.modules[module].generation;
        let ids: Vec<RequestId> = requests.iter().map(|_| self.next_request()).collect();
        self.send(
            module,
            ids.iter()
                .zip(requests)
                .map(|(id, request)| Event::Request { id: *id, request })
                .collect(),
        );
        let mut replies: HashMap<RequestId, Reply> = HashMap::new();
        while replies.len() < ids.len() {
            let held = &self.modules[module];
            if held.worker.is_none() || held.generation != generation {
                break;
            }
            let mut answered = false;
            for (from, command) in self.collect() {
                match command {
                    Command::Reply { id, reply } if from == module && ids.contains(&id) => {
                        replies.insert(id, reply);
                        answered = true;
                    }
                    command => self.backlog.push_back((from, command)),
                }
            }
            if !answered && replies.len() < ids.len() {
                self.notified().await;
            }
        }
        ids.iter()
            .map(|id| replies.remove(id).unwrap_or(Reply::Unknown))
            .collect()
    }

    pub async fn ask(&mut self, content_type: Uuid, request: Request) -> Reply {
        let Some(module) = self.module_of(content_type) else {
            return Reply::Unknown;
        };
        self.requests_all(module, vec![request])
            .await
            .pop()
            .unwrap_or(Reply::Unknown)
    }

    pub async fn to_files(
        &mut self,
        blocks: Vec<(Uuid, Vec<u8>)>,
    ) -> Vec<Result<Option<PlainFile>, String>> {
        let mut answers: Vec<Result<Option<PlainFile>, String>> = Vec::with_capacity(blocks.len());
        let mut asked: HashMap<usize, (Vec<usize>, Vec<Request>)> = HashMap::new();
        for (index, (content_type, bytes)) in blocks.into_iter().enumerate() {
            answers.push(Err(format!(
                "no type module has the block type {content_type}"
            )));
            if let Some(module) = self.module_of(content_type) {
                let (indices, requests) = asked.entry(module).or_default();
                indices.push(index);
                requests.push(Request::ToFile {
                    content_type,
                    bytes,
                });
            }
        }
        for (module, (indices, requests)) in asked {
            for (index, reply) in indices
                .into_iter()
                .zip(self.requests_all(module, requests).await)
            {
                answers[index] = match reply {
                    Reply::File(file) => Ok(file),
                    Reply::Malformed(error) => Err(error),
                    other => Err(format!("the type module answered {other:?}")),
                };
            }
        }
        answers
    }

    pub async fn claims(&mut self, name: &str, first: &[u8], directory: bool) -> Option<Uuid> {
        for fallback in [false, true] {
            for module in 0..self.modules.len() {
                let request = Request::Claims {
                    name: name.to_owned(),
                    first: first.to_vec(),
                    directory,
                    fallback,
                };
                if let Some(Reply::Claimed(Some(content_type))) =
                    self.requests_all(module, vec![request]).await.pop()
                {
                    return Some(content_type);
                }
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
        let request = Request::Apply {
            content_type,
            bytes,
            client,
            change,
        };
        match self.ask(content_type, request).await {
            Reply::Content(bytes) => Ok(Some(bytes)),
            Reply::Changed(false) => Ok(None),
            Reply::Malformed(error) => Err(error),
            Reply::Unknown => Err(format!("no type module has the block type {content_type}")),
            other => Err(format!("the type module answered {other:?}")),
        }
    }

    pub async fn describe(
        &mut self,
        content_type: Uuid,
        bytes: &[u8],
        organization: Uuid,
    ) -> Result<Description, String> {
        let request = Request::Describe {
            content_type,
            bytes: bytes.to_vec(),
            organization,
        };
        match self.ask(content_type, request).await {
            Reply::Described(description) => Ok(description),
            Reply::Malformed(error) => Err(error),
            Reply::Unknown => Err(format!("no type module has the block type {content_type}")),
            other => Err(format!("the type module answered {other:?}")),
        }
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
