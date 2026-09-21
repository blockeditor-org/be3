use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        mpsc::{Receiver, Sender, TryRecvError, channel},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use be_block::{BlockContent, CounterContent, LiveEdit};
use be_client::{ClientError, Credentials, Live, Peer, PeerConfig};
use be_graph::BlockParent;
use be_protocol::ErrorCode;
use be_store::{ContentKey, FileStore};
use uuid::Uuid;

use super::{Config, Content, Status};

const TICK: Duration = Duration::from_millis(16);
const SEAL_INTERVAL: Duration = Duration::from_millis(750);
const FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

enum Command {
    Open(Uuid, Uuid),
    Close(Uuid),
    Operate(Uuid, Vec<u8>),
    Flush(Sender<()>),
}

#[derive(Default)]
struct Shared {
    blocks: HashMap<Uuid, Content>,
    unsealed: usize,
    workspace: Option<Uuid>,
    connected: bool,
    error: Option<String>,
}

struct Stack {
    account: Uuid,
    workspace: Uuid,
    commands: Option<Sender<Command>>,
    shared: Arc<Mutex<Shared>>,
    worker: Option<JoinHandle<()>>,
}

impl Drop for Stack {
    fn drop(&mut self) {
        self.commands = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn stack() -> &'static Mutex<Option<Stack>> {
    static STACK: OnceLock<Mutex<Option<Stack>>> = OnceLock::new();
    STACK.get_or_init(|| Mutex::new(None))
}

pub(crate) fn start(config: Config) {
    stop();
    let (commands, receiver) = channel();
    let shared = Arc::new(Mutex::new(Shared::default()));
    let account = config.account;
    let workspace = config.workspace;
    let worker = {
        let shared = Arc::clone(&shared);
        thread::Builder::new()
            .name("block-app-be".into())
            .spawn(move || run(config, receiver, shared))
            .ok()
    };
    *stack().lock().unwrap() = Some(Stack {
        account,
        workspace,
        commands: Some(commands),
        shared,
        worker,
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
    let (done, sealed) = channel();
    send(Command::Flush(done));
    let _ = sealed.recv_timeout(FLUSH_TIMEOUT);
}

pub(crate) fn workspace() -> Option<Uuid> {
    with_shared(|shared| shared.workspace)?
}

pub(crate) fn open(block: Uuid, content_type: Uuid) {
    send(Command::Open(block, content_type));
}

pub(crate) fn close(block: Uuid) {
    send(Command::Close(block));
}

pub(crate) fn operate(block: Uuid, operation: Vec<u8>) {
    send(Command::Operate(block, operation));
}

pub(crate) fn content(block: Uuid) -> Option<Content> {
    with_shared(|shared| shared.blocks.get(&block).cloned())?
}

pub(crate) fn status() -> Status {
    with_shared(|shared| Status {
        running: true,
        connected: shared.connected,
        blocks: shared.blocks.len(),
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

fn with_shared<T>(read: impl FnOnce(&Shared) -> T) -> Option<T> {
    let held = stack().lock().unwrap();
    let stack = held.as_ref()?;
    let shared = stack.shared.lock().unwrap();
    Some(read(&shared))
}

enum Session {
    Counter(Live<FileStore, CounterContent>),
}

impl Session {
    async fn join(
        peer: &Arc<Peer<FileStore>>,
        block: Uuid,
        content_type: Uuid,
    ) -> Result<Option<Self>, ClientError> {
        if content_type != CounterContent::CONTENT_TYPE {
            return Ok(None);
        }
        peer.ensure::<CounterContent>(block, BlockParent::Root)
            .await?;
        let mut live = Live::join(Arc::clone(peer), block).await?;
        live.reconcile().await?;
        Ok(Some(Self::Counter(live)))
    }

    fn content_type(&self) -> Uuid {
        match self {
            Self::Counter(_) => CounterContent::CONTENT_TYPE,
        }
    }

    fn bytes(&self) -> Vec<u8> {
        match self {
            Self::Counter(live) => live.content().encode(),
        }
    }

    async fn edit(&mut self, operation: &[u8]) -> Result<(), ClientError> {
        match self {
            Self::Counter(live) => match CounterContent::decode_operation(operation) {
                Ok(operation) => live.edit(operation).await,
                Err(_) => Ok(()),
            },
        }
    }

    async fn poll(&mut self) -> Result<(), ClientError> {
        match self {
            Self::Counter(live) => live.poll().await.map(|_| ()),
        }
    }

    fn is_clean(&self) -> bool {
        match self {
            Self::Counter(live) => live.is_clean(),
        }
    }

    async fn seal(&mut self) -> Result<(), ClientError> {
        if self.is_clean() {
            return Ok(());
        }
        match self {
            Self::Counter(live) => live.seal().await.map(|_| ()),
        }
    }
}

fn run(config: Config, commands: Receiver<Command>, shared: Arc<Mutex<Shared>>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            return fail(
                &shared,
                format!("the new block stack cannot start: {error}"),
            );
        }
    };
    runtime.block_on(serve(config, commands, shared));
}

async fn serve(config: Config, commands: Receiver<Command>, shared: Arc<Mutex<Shared>>) {
    let context = config.context.clone();
    let peer = match connect(&config).await {
        Ok(peer) => Arc::new(peer),
        Err(error) => return fail(&shared, error.to_string()),
    };
    {
        let mut held = shared.lock().unwrap();
        held.workspace = Some(peer.workspace());
        held.connected = true;
    }
    context.request_repaint();
    let mut sessions: HashMap<Uuid, Session> = HashMap::new();
    let mut sealed_at = Instant::now();
    loop {
        let mut changed = false;
        loop {
            match commands.try_recv() {
                Ok(command) => changed |= apply(&peer, &mut sessions, &shared, command).await,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return seal_all(&mut sessions, &shared).await,
            }
        }
        for session in sessions.values_mut() {
            if let Err(error) = session.poll().await {
                record(&shared, error);
            }
        }
        if sealed_at.elapsed() >= SEAL_INTERVAL {
            sealed_at = Instant::now();
            for session in sessions.values_mut() {
                if let Err(error) = session.seal().await {
                    record(&shared, error);
                }
            }
        }
        changed |= publish(&sessions, &shared);
        if changed {
            context.request_repaint();
        }
        tokio::time::sleep(TICK).await;
    }
}

async fn apply(
    peer: &Arc<Peer<FileStore>>,
    sessions: &mut HashMap<Uuid, Session>,
    shared: &Arc<Mutex<Shared>>,
    command: Command,
) -> bool {
    match command {
        Command::Open(block, content_type) => {
            if sessions.contains_key(&block) {
                return false;
            }
            match Session::join(peer, block, content_type).await {
                Ok(Some(session)) => {
                    sessions.insert(block, session);
                    true
                }
                Ok(None) => false,
                Err(error) => {
                    record(shared, error);
                    false
                }
            }
        }
        Command::Close(block) => {
            let Some(mut session) = sessions.remove(&block) else {
                return false;
            };
            if let Err(error) = session.seal().await {
                record(shared, error);
            }
            let _ = peer.leave_session(block).await;
            shared.lock().unwrap().blocks.remove(&block);
            true
        }
        Command::Operate(block, operation) => {
            let Some(session) = sessions.get_mut(&block) else {
                return false;
            };
            if let Err(error) = session.edit(&operation).await {
                record(shared, error);
            }
            true
        }
        Command::Flush(done) => {
            seal_all(sessions, shared).await;
            let changed = publish(sessions, shared);
            let _ = done.send(());
            changed
        }
    }
}

async fn seal_all(sessions: &mut HashMap<Uuid, Session>, shared: &Arc<Mutex<Shared>>) {
    for session in sessions.values_mut() {
        if let Err(error) = session.seal().await {
            record(shared, error);
        }
    }
}

fn publish(sessions: &HashMap<Uuid, Session>, shared: &Arc<Mutex<Shared>>) -> bool {
    let mut held = shared.lock().unwrap();
    held.unsealed = sessions
        .values()
        .filter(|session| !session.is_clean())
        .count();
    let mut changed = false;
    for (block, session) in sessions {
        let bytes = session.bytes();
        match held.blocks.get_mut(block) {
            Some(content) if content.bytes == bytes => {}
            Some(content) => {
                content.bytes = bytes;
                content.revision += 1;
                changed = true;
            }
            None => {
                held.blocks.insert(
                    *block,
                    Content {
                        content_type: session.content_type(),
                        bytes,
                        revision: 1,
                    },
                );
                changed = true;
            }
        }
    }
    changed
}

async fn connect(config: &Config) -> Result<Peer<FileStore>, ClientError> {
    let key = ContentKey::from_bytes(config.key);
    let store = FileStore::open(&config.directory)?;
    let login = PeerConfig::new(
        &config.url,
        key,
        Credentials::Login {
            email: config.email.clone(),
            password: config.password.clone(),
        },
    )
    .workspace(config.be_workspace);
    match Peer::connect(login, store).await {
        Err(ClientError::Refused(ErrorCode::InvalidCredentials, _)) => {}
        outcome => return outcome,
    }
    let store = FileStore::open(&config.directory)?;
    let register = PeerConfig::new(
        &config.url,
        key,
        Credentials::Register {
            email: config.email.clone(),
            display_name: config.display_name.clone(),
            password: config.password.clone(),
        },
    )
    .workspace(config.be_workspace);
    Peer::connect(register, store).await
}

fn record(shared: &Arc<Mutex<Shared>>, error: ClientError) {
    shared.lock().unwrap().error = Some(error.to_string());
}

fn fail(shared: &Arc<Mutex<Shared>>, error: String) {
    let mut held = shared.lock().unwrap();
    held.connected = false;
    held.error = Some(error);
}
