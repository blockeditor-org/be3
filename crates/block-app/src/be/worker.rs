use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use be_block::{BlockContent, BlockMetadata, LiveEdit, Merge, Undo};
use be_client::{ClientError, Credentials, Journaled, Live, Peer, PeerConfig, Saved};
use be_commit::CommitId;
use be_graph::{Access, BlockParent};
use be_protocol::{AccessEntry, BlockSummary, ServerMessage};
use be_store::ContentKey;
use futures_util::future::{Either, LocalBoxFuture, select};
use tokio::sync::mpsc::UnboundedReceiver;
use uuid::Uuid;

use super::graph::{Graph, Node};
use super::version::{Context, Versions};
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
        metadata: BlockMetadata,
    },
    Seed {
        block: Uuid,
        content_type: Uuid,
        bytes: Vec<u8>,
    },
    Replace {
        block: Uuid,
        content_type: Uuid,
        bytes: Vec<u8>,
    },
    Flush(std::sync::mpsc::Sender<()>),
    History {
        block: Uuid,
        redo: bool,
    },
    Presence {
        block: Uuid,
        kind: Uuid,
        value: Option<Vec<u8>>,
    },
    Create {
        block: Uuid,
        content_type: Uuid,
        parent: BlockParent,
        metadata: BlockMetadata,
        bytes: Option<Vec<u8>>,
    },
    SetParent {
        block: Uuid,
        parent: BlockParent,
    },
    SetMetadata {
        block: Uuid,
        metadata: BlockMetadata,
    },
    SetAccess {
        block: Uuid,
        account: Uuid,
        access: Access,
    },
    ListAccess {
        block: Uuid,
        reply: crate::host::WakingSender<Result<Vec<AccessEntry>, String>>,
    },
    Version {
        block: Uuid,
        command: block_plugin_api::VersionCommand,
    },
    WatchVersion(Uuid),
}

#[derive(Default)]
pub(crate) struct Shared {
    pub(crate) graph: Graph,
    pub(crate) blocks: HashMap<Uuid, Content>,
    pub(crate) histories: HashMap<Uuid, History>,
    pub(crate) presence: HashMap<Uuid, (u64, Vec<Presence>)>,
    pub(crate) presence_revision: u64,
    pub(crate) wakes: u64,
    pub(crate) unsealed: usize,
    pub(crate) connected: bool,
    pub(crate) error: Option<String>,
    pub(crate) version_watch: std::collections::HashSet<Uuid>,
    pub(crate) versions: HashMap<Uuid, super::version::VersionState>,
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
        peer.ensure::<C>(to, BlockParent::Detached).await?;
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
        peer.ensure::<C>(block, BlockParent::Detached).await?;
        if peer.remote_head(block).await?.is_some() {
            return Ok(());
        }
        peer.save(block, &content, None).await?;
        Ok(())
    })
}

pub(super) fn replace<C>(
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
        peer.ensure::<C>(block, BlockParent::Detached).await?;
        let mut expected = peer.remote_head(block).await?;
        for _ in 0..4 {
            match peer.save(block, &content, expected).await? {
                be_client::Saved::Rejected { head } => expected = head,
                _ => return Ok(()),
            }
        }
        Ok(())
    })
}

pub(super) trait Session {
    fn content_type(&self) -> Uuid;

    fn bytes(&self) -> Vec<u8>;

    fn head(&self) -> Option<CommitId>;

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

    fn replace(&mut self, bytes: Vec<u8>) -> LocalBoxFuture<'_, Result<(), ClientError>>;

    fn show(
        &mut self,
        kind: Uuid,
        value: Option<Vec<u8>>,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>>;

    fn take_presence(&mut self) -> Option<Vec<Presence>>;

    fn published_elsewhere(
        &mut self,
        head: CommitId,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>>;

    fn history(&self) -> History {
        History::default()
    }

    fn step_history(&mut self, redo: bool) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        let _ = redo;
        Box::pin(async { Ok(()) })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Presence {
    pub(crate) client: u64,
    pub(crate) kind: Uuid,
    pub(crate) value: Vec<u8>,
}

fn take_presence<C>(live: &mut Live<Store, C>) -> Option<Vec<Presence>>
where
    C: LiveEdit + Clone + Default,
{
    live.take_presence_changed().then(|| {
        live.presence()
            .iter()
            .map(|((client, kind), value)| Presence {
                client: *client,
                kind: *kind,
                value: value.clone(),
            })
            .collect()
    })
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
        peer.ensure::<C>(block, BlockParent::Detached).await?;
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

    fn head(&self) -> Option<CommitId> {
        self.live.head()
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
        Box::pin(async move {
            self.live.poll().await?;
            self.live.catch_up().await
        })
    }

    fn seal(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { seal(&mut self.live).await })
    }

    fn replace(&mut self, bytes: Vec<u8>) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move {
            let Ok(content) = C::decode(&bytes) else {
                return Ok(());
            };
            self.undo.clear();
            self.redo.clear();
            drain(&mut self.live, &mut self.log, None);
            let replaced = self.live.replace(content).await;
            drain(&mut self.live, &mut self.log, None);
            replaced.map(|_| ())
        })
    }

    fn show(
        &mut self,
        kind: Uuid,
        value: Option<Vec<u8>>,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { self.live.set_presence(kind, value).await })
    }

    fn take_presence(&mut self) -> Option<Vec<Presence>> {
        take_presence(&mut self.live)
    }

    fn published_elsewhere(
        &mut self,
        head: CommitId,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { self.live.published_elsewhere(head).await })
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
        peer.ensure::<C>(block, BlockParent::Detached).await?;
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

    fn head(&self) -> Option<CommitId> {
        self.live.head()
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
        Box::pin(async move {
            self.live.poll().await?;
            self.live.catch_up().await
        })
    }

    fn seal(&mut self) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { seal(&mut self.live).await })
    }

    fn replace(&mut self, bytes: Vec<u8>) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move {
            let Ok(content) = C::decode(&bytes) else {
                return Ok(());
            };
            drain(&mut self.live, &mut self.log, None);
            let replaced = self.live.replace(content).await;
            drain(&mut self.live, &mut self.log, None);
            replaced.map(|_| ())
        })
    }

    fn show(
        &mut self,
        kind: Uuid,
        value: Option<Vec<u8>>,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { self.live.set_presence(kind, value).await })
    }

    fn take_presence(&mut self) -> Option<Vec<Presence>> {
        take_presence(&mut self.live)
    }

    fn published_elsewhere(
        &mut self,
        head: CommitId,
    ) -> LocalBoxFuture<'_, Result<(), ClientError>> {
        Box::pin(async move { self.live.published_elsewhere(head).await })
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
        Ok(peer) => {
            let resolving = Arc::clone(shared);
            peer.set_resolver(Arc::new(move |block, references| {
                let held = resolving.lock().unwrap();
                let Some(scope) = held.graph.scope_of(block) else {
                    return references;
                };
                references
                    .into_iter()
                    .map(|reference| {
                        held.graph.to_real(
                            scope,
                            reference,
                            block_plugin_api::BlockIdRole::Existing,
                        )
                    })
                    .collect()
            }));
            Arc::new(peer)
        }
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
    load_graph(&peer, shared).await;
    let mut sessions: HashMap<Uuid, Box<dyn Session>> = HashMap::new();
    for (block, content_type) in open.clone() {
        rejoin(&peer, &mut sessions, shared, block, content_type).await;
    }
    let mut versions = Versions::default();
    publish(&mut sessions, shared);
    versions
        .refresh(&mut Context {
            peer: &peer,
            shared,
            sessions: &mut sessions,
        })
        .await;
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
            Woken::Command(command) => {
                apply(&peer, &mut sessions, shared, open, &mut versions, command).await
            }
            Woken::Event(event) => {
                match event {
                    Some(ServerMessage::BlockChanged { block }) => {
                        let node = node_of(&peer, &block);
                        shared.lock().unwrap().graph.put(node);
                    }
                    Some(ServerMessage::BlockRemoved { block }) => {
                        shared.lock().unwrap().graph.remove(block);
                    }
                    Some(ServerMessage::HeadChanged { block, head, .. }) => {
                        shared.lock().unwrap().graph.set_head(block, Some(head));
                    }
                    Some(_) => {}
                    None => load_graph(&peer, shared).await,
                }
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
        let mut moved = publish(&mut sessions, shared);
        let before = version_revisions(shared);
        versions
            .refresh(&mut Context {
                peer: &peer,
                shared,
                sessions: &mut sessions,
            })
            .await;
        moved |= version_revisions(shared) != before;
        changed.notify_all();
        if moved {
            crate::host::wake();
        }
    }
}

enum Woken {
    Command(Command),
    Event(Option<ServerMessage>),
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
        Either::Left((Either::Right((Ok(event), _)), _)) => Woken::Event(Some(event)),
        Either::Left((Either::Right((Err(error), _)), _)) => match error {
            tokio::sync::broadcast::error::RecvError::Lagged(_) => Woken::Event(None),
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

fn remember_head(peer: &Peer<Store>, shared: &Arc<Mutex<Shared>>, block: Uuid) {
    shared
        .lock()
        .unwrap()
        .graph
        .set_head(block, peer.local_head(block));
}

fn version_revisions(shared: &Arc<Mutex<Shared>>) -> u64 {
    shared
        .lock()
        .unwrap()
        .versions
        .values()
        .map(|state| state.revision)
        .sum()
}

fn set_busy(shared: &Arc<Mutex<Shared>>, block: Uuid, busy: bool, error: Option<String>) {
    let mut held = shared.lock().unwrap();
    let state = held.versions.entry(block).or_default();
    state.status.busy = busy;
    state.status.error = error;
    state.revision += 1;
}

async fn apply(
    peer: &Arc<Peer<Store>>,
    sessions: &mut HashMap<Uuid, Box<dyn Session>>,
    shared: &Arc<Mutex<Shared>>,
    open: &mut HashMap<Uuid, Uuid>,
    versions: &mut Versions,
    command: Command,
) -> bool {
    match command {
        Command::WatchVersion(block) => {
            Versions::watch(shared, block);
            false
        }
        Command::Version { block, command } => {
            set_busy(shared, block, true, None);
            crate::host::wake();
            seal_all(sessions, shared).await;
            publish(sessions, shared);
            let outcome = versions
                .run(
                    &mut Context {
                        peer,
                        shared,
                        sessions,
                    },
                    block,
                    command,
                )
                .await;
            set_busy(shared, block, false, outcome.err());
            true
        }
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
            held.presence.remove(&block);
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
            metadata,
        } => {
            let Some(copy) = super::copy_for(content_type) else {
                shared.lock().unwrap().graph.settle(to);
                return false;
            };
            let created = peer
                .create_block(to, content_type, BlockParent::Detached, &metadata)
                .await;
            if let Err(error) = settled(peer, shared, to, created).await {
                record(shared, error);
                return false;
            }
            let shown = sessions.get(&from).map(|session| session.bytes());
            if let Err(error) = copy(peer, from, to, shown).await {
                record(shared, error);
            }
            remember_head(peer, shared, to);
            false
        }
        Command::Replace {
            block,
            content_type,
            bytes,
        } => {
            let outcome = match sessions.get_mut(&block) {
                Some(session) => session.replace(bytes).await,
                None => match super::replace_for(content_type) {
                    Some(replace) => replace(peer, block, bytes).await,
                    None => Ok(()),
                },
            };
            if let Err(error) = outcome {
                record(shared, error);
            }
            remember_head(peer, shared, block);
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
            remember_head(peer, shared, block);
            false
        }
        Command::Presence { block, kind, value } => {
            let Some(session) = sessions.get_mut(&block) else {
                return false;
            };
            if let Err(error) = session.show(kind, value).await {
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
        Command::Create {
            block,
            content_type,
            parent,
            metadata,
            bytes,
        } => {
            let created = peer
                .create_block(block, content_type, parent, &metadata)
                .await;
            if let Err(error) = settled(peer, shared, block, created).await {
                record(shared, error);
                return false;
            }
            if let (Some(bytes), Some(seed)) = (bytes, super::seed_for(content_type))
                && let Err(error) = seed(peer, block, bytes).await
            {
                record(shared, error);
            }
            remember_head(peer, shared, block);
            true
        }
        Command::SetParent { block, parent } => {
            let moved = peer.set_parent(block, parent).await;
            let settled = shared.lock().unwrap().graph.settle(block);
            if let Err(error) = moved {
                record(shared, error);
            }
            if settled {
                refresh(peer, shared, block).await;
            }
            true
        }
        Command::SetMetadata { block, metadata } => {
            let changed = peer.set_metadata(block, &metadata).await;
            if let Err(error) = settled(peer, shared, block, changed).await {
                record(shared, error);
            }
            true
        }
        Command::SetAccess {
            block,
            account,
            access,
        } => {
            if let Err(error) = peer.grant(block, account, access).await {
                record(shared, error);
            }
            true
        }
        Command::ListAccess { block, reply } => {
            let listed = peer
                .list_access(block)
                .await
                .map_err(|error| error.to_string());
            let _ = reply.send(listed);
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

async fn load_graph(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>) {
    match peer.list_blocks().await {
        Ok(blocks) => {
            let nodes = blocks.iter().map(|block| node_of(peer, block)).collect();
            shared.lock().unwrap().graph.load(nodes);
        }
        Err(error) => record(shared, error),
    }
}

async fn settled(
    peer: &Arc<Peer<Store>>,
    shared: &Arc<Mutex<Shared>>,
    block: Uuid,
    reply: Result<BlockSummary, ClientError>,
) -> Result<(), ClientError> {
    if !shared.lock().unwrap().graph.settle(block) {
        return reply.map(|_| ());
    }
    match reply {
        Ok(summary) => {
            let node = node_of(peer, &summary);
            shared.lock().unwrap().graph.put(node);
            Ok(())
        }
        Err(error) => {
            refresh(peer, shared, block).await;
            Err(error)
        }
    }
}

async fn refresh(peer: &Arc<Peer<Store>>, shared: &Arc<Mutex<Shared>>, block: Uuid) {
    match peer.summary(block).await {
        Ok(summary) => {
            let node = node_of(peer, &summary);
            shared.lock().unwrap().graph.put(node);
        }
        Err(ClientError::Refused(..)) => shared.lock().unwrap().graph.remove(block),
        Err(error) => record(shared, error),
    }
}

pub(super) fn node_of(peer: &Peer<Store>, summary: &BlockSummary) -> Node {
    Node {
        id: summary.id,
        content_type: summary.content_type,
        author: summary.author,
        parent: summary.parent,
        access: summary.access,
        references: summary.references.clone(),
        metadata: peer.metadata(summary),
        head: summary.head,
        version: summary.version,
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
        if let Some(presence) = session.take_presence() {
            held.presence_revision += 1;
            let revision = held.presence_revision;
            held.presence.insert(*block, (revision, presence));
            changed = true;
        }
        held.graph.set_head(*block, session.head());
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
            Credentials::Token(config.token.clone()),
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
