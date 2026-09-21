use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use be_block::{BlockContent, CounterContent, LiveEdit};
use be_client::{ClientError, Credentials, Live, Peer, PeerConfig, Saved};
use be_graph::BlockParent;
use be_store::ContentKey;
use futures_util::future::{Either, select};
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

use super::{Config, Content, platform};

const SEAL_INTERVAL: Duration = Duration::from_millis(750);
const RECONNECT_DELAY: Duration = Duration::from_secs(2);

pub(super) enum Command {
    Open(Uuid, Uuid),
    Close(Uuid),
    Operate(Uuid, Vec<u8>),
    Flush(std::sync::mpsc::Sender<()>),
}

#[derive(Default)]
pub(super) struct Shared {
    pub(super) blocks: HashMap<Uuid, Content>,
    pub(super) wakes: u64,
    pub(super) unsealed: usize,
    pub(super) connected: bool,
    pub(super) error: Option<String>,
}

enum Session {
    Counter(Live<Store, CounterContent>),
}

#[cfg(not(target_arch = "wasm32"))]
type Store = be_store::FileStore;
#[cfg(target_arch = "wasm32")]
type Store = be_store::MemoryStore;

impl Session {
    async fn join(
        peer: &Arc<Peer<Store>>,
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

    fn is_clean(&self) -> bool {
        match self {
            Self::Counter(live) => live.is_clean(),
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

    async fn seal(&mut self) -> Result<(), ClientError> {
        if self.is_clean() {
            return Ok(());
        }
        match self {
            Self::Counter(live) => {
                if matches!(live.seal().await?, Saved::Rejected { .. }) {
                    live.reconcile().await?;
                    live.seal().await?;
                }
                Ok(())
            }
        }
    }
}

pub(super) async fn serve<S: Fn() -> Result<Store, String>>(
    config: Config,
    make_store: S,
    mut commands: UnboundedReceiver<Command>,
    shared: Arc<Mutex<Shared>>,
) {
    let context = config.context.clone();
    let mut open: HashMap<Uuid, Uuid> = HashMap::new();
    loop {
        if let Outcome::Stopped = connected(
            &config,
            &make_store,
            &mut commands,
            &shared,
            &mut open,
            &context,
        )
        .await
        {
            return;
        }
        shared.lock().unwrap().connected = false;
        context.request_repaint();
        platform::sleep(RECONNECT_DELAY).await;
    }
}

enum Outcome {
    Stopped,
    Lost,
}

async fn connected<S: Fn() -> Result<Store, String>>(
    config: &Config,
    make_store: &S,
    commands: &mut UnboundedReceiver<Command>,
    shared: &Arc<Mutex<Shared>>,
    open: &mut HashMap<Uuid, Uuid>,
    context: &eframe::egui::Context,
) -> Outcome {
    let store = match make_store() {
        Ok(store) => store,
        Err(error) => {
            fail(shared, error);
            return Outcome::Lost;
        }
    };
    let peer = match connect(config, store).await {
        Ok(peer) => Arc::new(peer),
        Err(error) => {
            fail(shared, error.to_string());
            return Outcome::Lost;
        }
    };
    {
        let mut held = shared.lock().unwrap();
        held.connected = true;
        held.error = None;
    }
    context.request_repaint();
    let mut events = peer.connection().subscribe();
    let mut gone = peer.connection().closed();
    let mut sessions: HashMap<Uuid, Session> = HashMap::new();
    for (block, content_type) in open.clone() {
        rejoin(&peer, &mut sessions, shared, block, content_type).await;
    }
    publish(&sessions, shared);
    context.request_repaint();
    let mut unsealed_since: Option<Instant> = None;
    loop {
        let woken = match unsealed_since {
            Some(since) => {
                let remaining = SEAL_INTERVAL.saturating_sub(since.elapsed());
                let waited = std::pin::pin!(wait(commands, &mut events, &mut gone));
                let timer = std::pin::pin!(platform::sleep(remaining));
                match select(waited, timer).await {
                    Either::Left((woken, _)) => woken,
                    Either::Right(_) => Woken::Deadline,
                }
            }
            None => wait(commands, &mut events, &mut gone).await,
        };
        shared.lock().unwrap().wakes += 1;
        let sealed = match woken {
            Woken::Stopped => {
                seal_all(&mut sessions, shared).await;
                return Outcome::Stopped;
            }
            Woken::Disconnected => return Outcome::Lost,
            Woken::Command(command) => apply(&peer, &mut sessions, shared, open, command).await,
            Woken::Event => {
                for session in sessions.values_mut() {
                    if let Err(error) = session.poll().await {
                        record(shared, error);
                    }
                }
                false
            }
            Woken::Deadline => {
                seal_all(&mut sessions, shared).await;
                true
            }
        };
        if sealed {
            unsealed_since = None;
        }
        unsealed_since = match sessions.values().any(|session| !session.is_clean()) {
            true => unsealed_since.or_else(|| Some(Instant::now())),
            false => None,
        };
        if publish(&sessions, shared) {
            context.request_repaint();
        }
    }
}

enum Woken {
    Command(Command),
    Event,
    Deadline,
    Disconnected,
    Stopped,
}

async fn wait(
    commands: &mut UnboundedReceiver<Command>,
    events: &mut tokio::sync::broadcast::Receiver<be_protocol::ServerMessage>,
    gone: &mut tokio::sync::watch::Receiver<bool>,
) -> Woken {
    let command = std::pin::pin!(commands.recv());
    let event = std::pin::pin!(events.recv());
    let closed = std::pin::pin!(gone.changed());
    match select(select(command, event), closed).await {
        Either::Left((Either::Left((Some(command), _)), _)) => Woken::Command(command),
        Either::Left((Either::Left((None, _)), _)) => Woken::Stopped,
        Either::Left((Either::Right((Ok(_), _)), _)) => Woken::Event,
        Either::Left((Either::Right((Err(error), _)), _)) => match error {
            tokio::sync::broadcast::error::RecvError::Lagged(_) => Woken::Event,
            tokio::sync::broadcast::error::RecvError::Closed => Woken::Disconnected,
        },
        Either::Right(_) => Woken::Disconnected,
    }
}

async fn rejoin(
    peer: &Arc<Peer<Store>>,
    sessions: &mut HashMap<Uuid, Session>,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
    content_type: Uuid,
) {
    match Session::join(peer, block, content_type).await {
        Ok(Some(session)) => {
            sessions.insert(block, session);
        }
        Ok(None) => {}
        Err(error) => record(shared, error),
    }
}

async fn apply(
    peer: &Arc<Peer<Store>>,
    sessions: &mut HashMap<Uuid, Session>,
    shared: &Arc<Mutex<Shared>>,
    open: &mut HashMap<Uuid, Uuid>,
    command: Command,
) -> bool {
    match command {
        Command::Open(block, content_type) => {
            open.insert(block, content_type);
            if sessions.contains_key(&block) {
                return false;
            }
            rejoin(peer, sessions, shared, block, content_type).await;
            false
        }
        Command::Close(block) => {
            open.remove(&block);
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
            false
        }
        Command::Flush(done) => {
            seal_all(sessions, shared).await;
            publish(sessions, shared);
            let _ = done.send(());
            true
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

async fn connect(config: &Config, store: Store) -> Result<Peer<Store>, ClientError> {
    Peer::connect(
        PeerConfig::new(
            config.socket_url(),
            ContentKey::from_bytes(config.content_key()),
            Credentials::Adopted,
        )
        .workspace(Some(config.workspace)),
        store,
    )
    .await
}

fn record(shared: &Arc<Mutex<Shared>>, error: ClientError) {
    shared.lock().unwrap().error = Some(error.to_string());
}

fn fail(shared: &Arc<Mutex<Shared>>, error: String) {
    let mut held = shared.lock().unwrap();
    held.connected = false;
    held.error = Some(error);
}
