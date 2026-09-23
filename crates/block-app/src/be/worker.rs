use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use be_block::{BlockContent, LiveEdit, Merge, Undo};
use be_client::{ClientError, Credentials, Journaled, Live, Peer, PeerConfig, Saved};
use be_graph::BlockParent;
use be_store::ContentKey;
use futures_util::future::{Either, LocalBoxFuture, select};
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

use super::{Config, Content, platform};

const SEAL_INTERVAL: Duration = Duration::from_millis(750);
const RECONNECT_DELAY: Duration = Duration::from_secs(2);
const EDIT_BURST: Duration = Duration::from_millis(750);
const HISTORY_STEPS: usize = 200;

pub(super) enum Command {
    Open(Uuid, Uuid),
    Close(Uuid),
    Operate(Uuid, Option<u64>, Vec<u8>),
    Duplicate {
        from: Uuid,
        to: Uuid,
        content_type: Uuid,
    },
    Seed {
        block: Uuid,
        content_type: Uuid,
        bytes: Vec<u8>,
    },
    Flush(std::sync::mpsc::Sender<()>),
    History {
        block: Uuid,
        redo: bool,
    },
}

#[derive(Default)]
pub(crate) struct Shared {
    pub(crate) blocks: HashMap<Uuid, Content>,
    pub(crate) histories: HashMap<Uuid, History>,
    pub(crate) wakes: u64,
    pub(crate) unsealed: usize,
    pub(crate) connected: bool,
    pub(crate) error: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) type Store = be_store::FileStore;
#[cfg(target_arch = "wasm32")]
pub(super) type Store = be_store::MemoryStore;

pub(super) type Join = for<'a> fn(
    &'a Arc<Peer<Store>>,
    Uuid,
) -> LocalBoxFuture<'a, Result<Box<dyn Session>, ClientError>>;

pub(super) type Copy = for<'a> fn(
    &'a Arc<Peer<Store>>,
    Uuid,
    Uuid,
    Option<Vec<u8>>,
) -> LocalBoxFuture<'a, Result<(), ClientError>>;

pub(super) fn copy<C>(
    peer: &Arc<Peer<Store>>,
    from: Uuid,
    to: Uuid,
    shown: Option<Vec<u8>>,
) -> LocalBoxFuture<'_, Result<(), ClientError>>
where
    C: BlockContent + Default,
{
    Box::pin(async move {
        let content = match shown.and_then(|bytes| C::decode(&bytes).ok()) {
            Some(content) => content,
            None => peer.open::<C>(from).await?.unwrap_or_default(),
        };
        peer.ensure::<C>(to, BlockParent::Root).await?;
        peer.save(to, &content, None).await?;
        Ok(())
    })
}

pub(super) type Seed =
    for<'a> fn(&'a Arc<Peer<Store>>, Uuid, Vec<u8>) -> LocalBoxFuture<'a, Result<(), ClientError>>;

pub(super) fn seed<C>(
    peer: &Arc<Peer<Store>>,
    block: Uuid,
    bytes: Vec<u8>,
) -> LocalBoxFuture<'_, Result<(), ClientError>>
where
    C: BlockContent + Default,
{
    Box::pin(async move {
        let Ok(content) = C::decode(&bytes) else {
            return Ok(());
        };
        peer.ensure::<C>(block, BlockParent::Root).await?;
        if peer.remote_head(block).await?.is_some() {
            return Ok(());
        }
        peer.save(block, &content, None).await?;
        Ok(())
    })
}

pub(super) trait Session {
    fn content_type(&self) -> Uuid;

    fn bytes(&self) -> Vec<u8>;

    fn is_clean(&self) -> bool;

    fn owes_seal(&self) -> bool;

    fn edit<'a>(
        &'a mut self,
        operation: &'a [u8],
        origin: Option<u64>,
    ) -> LocalBoxFuture<'a, Result<(), ClientError>>;

    fn take_log(&mut self) -> Vec<Logged>;

    fn poll(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>>;

    fn seal(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>>;

    fn history(&self) -> History {
        History::default()
    }

    fn step_history(&mut self, redo: bool) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        let _ = redo;
        Box::pin(async { Ok(()) })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct History {
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
}

pub(super) fn join_with_history<C>(
    peer: &Arc<Peer<Store>>,
    block: Uuid,
) -> LocalBoxFuture<'_, Result<Box<dyn Session>, ClientError>>
where
    C: Undo + Merge + Clone + Default,
{
    Box::pin(async move {
        peer.ensure::<C>(block, BlockParent::Root).await?;
        let mut live = Live::<Store, C>::join(Arc::clone(peer), block).await?;
        live.reconcile().await?;
        Ok(Box::new(WithHistory {
            live,
            log: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
        }) as Box<dyn Session>)
    })
}

struct WithHistory<C: Undo> {
    live: Live<Store, C>,
    log: Vec<Logged>,
    undo: Vec<(C::Step, Instant)>,
    redo: Vec<C::Step>,
}

impl<C> Session for WithHistory<C>
where
    C: Undo + Merge + Clone + Default,
{
    fn content_type(&self) -> Uuid {
        C::CONTENT_TYPE
    }

    fn bytes(&self) -> Vec<u8> {
        self.live.content().encode()
    }

    fn is_clean(&self) -> bool {
        self.live.is_clean()
    }

    fn owes_seal(&self) -> bool {
        self.live.is_owner() && !self.live.is_clean()
    }

    fn edit<'a>(
        &'a mut self,
        operation: &'a [u8],
        origin: Option<u64>,
    ) -> LocalBoxFuture<'a, Result<(), ClientError>> {
        Box::pin(async move {
            let Ok(operation) = C::decode_operation(operation) else {
                return Ok(());
            };
            if let Some(step) = self.live.content().step(&operation) {
                self.record(step);
            }
            drain(&mut self.live, &mut self.log, None);
            let edited = self.live.edit(operation).await;
            drain(&mut self.live, &mut self.log, origin);
            edited
        })
    }

    fn take_log(&mut self) -> Vec<Logged> {
        drain(&mut self.live, &mut self.log, None);
        std::mem::take(&mut self.log)
    }

    fn poll(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { self.live.poll().await.map(|_| ()) })
    }

    fn seal(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { seal(&mut self.live).await })
    }

    fn history(&self) -> History {
        History {
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
        }
    }

    fn step_history(&mut self, redo: bool) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move {
            let (step, operations) = match redo {
                false => {
                    let Some((step, _)) = self.undo.pop() else {
                        return Ok(());
                    };
                    let operations = self.live.content().revert(&step);
                    (step, operations)
                }
                true => {
                    let Some(step) = self.redo.pop() else {
                        return Ok(());
                    };
                    let operations = self.live.content().reapply(&step);
                    (step, operations)
                }
            };
            match redo {
                false => self.redo.push(step),
                true => self.undo.push((step, Instant::now())),
            }
            for operation in operations {
                self.live.edit(operation).await?;
            }
            Ok(())
        })
    }
}

impl<C: Undo> WithHistory<C> {
    fn record(&mut self, step: C::Step) {
        self.redo.clear();
        let now = Instant::now();
        let step = match self.undo.last_mut() {
            Some((previous, at)) if now.duration_since(*at) <= EDIT_BURST => {
                match C::absorb(previous, step) {
                    Ok(()) => {
                        *at = now;
                        return;
                    }
                    Err(step) => step,
                }
            }
            _ => step,
        };
        self.undo.push((step, now));
        if self.undo.len() > HISTORY_STEPS {
            self.undo.remove(0);
        }
    }
}

pub(super) fn join<C>(
    peer: &Arc<Peer<Store>>,
    block: Uuid,
) -> LocalBoxFuture<'_, Result<Box<dyn Session>, ClientError>>
where
    C: LiveEdit + Merge + Clone + Default,
{
    Box::pin(async move {
        peer.ensure::<C>(block, BlockParent::Root).await?;
        let mut live = Live::<Store, C>::join(Arc::clone(peer), block).await?;
        live.reconcile().await?;
        Ok(Box::new(Plain {
            live,
            log: Vec::new(),
        }) as Box<dyn Session>)
    })
}

struct Plain<C: LiveEdit> {
    live: Live<Store, C>,
    log: Vec<Logged>,
}

impl<C> Session for Plain<C>
where
    C: LiveEdit + Merge + Clone + Default,
{
    fn content_type(&self) -> Uuid {
        C::CONTENT_TYPE
    }

    fn bytes(&self) -> Vec<u8> {
        self.live.content().encode()
    }

    fn is_clean(&self) -> bool {
        self.live.is_clean()
    }

    fn owes_seal(&self) -> bool {
        self.live.is_owner() && !self.live.is_clean()
    }

    fn edit<'a>(
        &'a mut self,
        operation: &'a [u8],
        origin: Option<u64>,
    ) -> LocalBoxFuture<'a, Result<(), ClientError>> {
        Box::pin(async move {
            let Ok(operation) = C::decode_operation(operation) else {
                return Ok(());
            };
            drain(&mut self.live, &mut self.log, None);
            let edited = self.live.edit(operation).await;
            drain(&mut self.live, &mut self.log, origin);
            edited
        })
    }

    fn take_log(&mut self) -> Vec<Logged> {
        drain(&mut self.live, &mut self.log, None);
        std::mem::take(&mut self.log)
    }

    fn poll(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { self.live.poll().await.map(|_| ()) })
    }

    fn seal(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { seal(&mut self.live).await })
    }
}

async fn seal<C>(live: &mut Live<Store, C>) -> Result<(), ClientError>
where
    C: LiveEdit + Merge + Clone + Default,
{
    if !live.is_owner() || live.is_clean() {
        return Ok(());
    }
    if matches!(live.seal().await?, Saved::Rejected { .. }) {
        live.reconcile().await?;
        live.seal().await?;
    }
    Ok(())
}

pub(crate) enum Logged {
    Operation { bytes: Vec<u8>, origin: Option<u64> },
    Replaced,
}

fn drain<C>(live: &mut Live<Store, C>, log: &mut Vec<Logged>, origin: Option<u64>)
where
    C: LiveEdit + Clone + Default,
{
    for entry in live.take_journal() {
        match entry {
            Journaled::Edited(operation) => log.push(Logged::Operation {
                bytes: C::encode_operation(&operation),
                origin,
            }),
            Journaled::Applied(operation) => log.push(Logged::Operation {
                bytes: C::encode_operation(&operation),
                origin: None,
            }),
            Journaled::Replaced => {
                log.clear();
                log.push(Logged::Replaced);
            }
        }
    }
}

pub(super) async fn serve<S: Fn() -> Result<Store, String>>(
    config: Config,
    make_store: S,
    mut commands: UnboundedReceiver<Command>,
    shared: Arc<Mutex<Shared>>,
    changed: Arc<Condvar>,
) {
    let mut open: HashMap<Uuid, Uuid> = HashMap::new();
    loop {
        if let Outcome::Stopped = connected(
            &config,
            &make_store,
            &mut commands,
            &shared,
            &changed,
            &mut open,
        )
        .await
        {
            changed.notify_all();
            return;
        }
        shared.lock().unwrap().connected = false;
        changed.notify_all();
        crate::host::wake();
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
    changed: &Arc<Condvar>,
    open: &mut HashMap<Uuid, Uuid>,
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
    changed.notify_all();
    crate::host::wake();
    let mut events = peer.connection().subscribe();
    let mut gone = peer.connection().closed();
    let mut sessions: HashMap<Uuid, Box<dyn Session>> = HashMap::new();
    for (block, content_type) in open.clone() {
        rejoin(&peer, &mut sessions, shared, block, content_type).await;
    }
    publish(&mut sessions, shared);
    changed.notify_all();
    crate::host::wake();
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
        unsealed_since = match sessions.values().any(|session| session.owes_seal()) {
            true => unsealed_since.or_else(|| Some(Instant::now())),
            false => None,
        };
        let moved = publish(&mut sessions, shared);
        changed.notify_all();
        if moved {
            crate::host::wake();
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
    sessions: &mut HashMap<Uuid, Box<dyn Session>>,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
    content_type: Uuid,
) {
    let Some(join) = super::join_for(content_type) else {
        return;
    };
    match join(peer, block).await {
        Ok(session) => {
            sessions.insert(block, session);
        }
        Err(error) => record(shared, error),
    }
}

async fn apply(
    peer: &Arc<Peer<Store>>,
    sessions: &mut HashMap<Uuid, Box<dyn Session>>,
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
            let mut held = shared.lock().unwrap();
            held.blocks.remove(&block);
            held.histories.remove(&block);
            true
        }
        Command::Operate(block, origin, operation) => {
            let Some(session) = sessions.get_mut(&block) else {
                return false;
            };
            if let Err(error) = session.edit(&operation, origin).await {
                record(shared, error);
            }
            false
        }
        Command::Duplicate {
            from,
            to,
            content_type,
        } => {
            let Some(copy) = super::copy_for(content_type) else {
                return false;
            };
            let shown = sessions.get(&from).map(|session| session.bytes());
            if let Err(error) = copy(peer, from, to, shown).await {
                record(shared, error);
            }
            false
        }
        Command::Seed {
            block,
            content_type,
            bytes,
        } => {
            if sessions.contains_key(&block) {
                return false;
            }
            let Some(seed) = super::seed_for(content_type) else {
                return false;
            };
            if let Err(error) = seed(peer, block, bytes).await {
                record(shared, error);
            }
            false
        }
        Command::History { block, redo } => {
            let Some(session) = sessions.get_mut(&block) else {
                return false;
            };
            if let Err(error) = session.step_history(redo).await {
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

async fn seal_all(sessions: &mut HashMap<Uuid, Box<dyn Session>>, shared: &Arc<Mutex<Shared>>) {
    for session in sessions.values_mut() {
        if let Err(error) = session.seal().await {
            record(shared, error);
        }
    }
}

fn publish(sessions: &mut HashMap<Uuid, Box<dyn Session>>, shared: &Arc<Mutex<Shared>>) -> bool {
    let mut held = shared.lock().unwrap();
    held.unsealed = sessions
        .values()
        .filter(|session| !session.is_clean())
        .count();
    let mut changed = false;
    for (block, session) in sessions.iter_mut() {
        let history = session.history();
        if held.histories.get(block) != Some(&history) {
            held.histories.insert(*block, history);
            changed = true;
        }
        let log = session.take_log();
        let Some(content) = held.blocks.get_mut(block) else {
            held.blocks.insert(
                *block,
                Content::new(session.content_type(), session.bytes()),
            );
            changed = true;
            continue;
        };
        if log.is_empty() {
            continue;
        }
        for entry in log {
            content.record(entry);
        }
        content.bytes = session.bytes();
        changed = true;
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
