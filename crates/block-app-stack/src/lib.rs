use std::sync::{Arc, Condvar, OnceLock, PoisonError};

use wasi_threads::Mutex;

use std::path::PathBuf;

use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use uuid::Uuid;

pub mod accounts;
pub mod conflicts;
pub mod discovery;
#[cfg(not(target_arch = "wasm32"))]
pub mod embedded_server;
mod graph;
pub mod invite_link;
pub mod keys;
pub mod link;
mod nudge;
pub mod offline_choice;
pub mod plugin_graph;
mod replicas;
pub mod revisions;
#[cfg(feature = "testing")]
pub mod testing;
pub mod types;
mod version;
mod worker;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
use native as platform;
#[cfg(target_arch = "wasm32")]
use web as platform;

pub use graph::{Graph, Node, Owner, Query, Scope};
pub use worker::{History, PairingRequest, Shared};

use worker::Command;

fn refs(config: &Config) -> Result<be_client::RefsDb, String> {
    refs_in(&config.data_dir)
}

pub fn refs_in(root: &std::path::Path) -> Result<be_client::RefsDb, String> {
    static OPEN: Mutex<Vec<(std::path::PathBuf, be_client::RefsDb)>> = Mutex::new(Vec::new());
    let path = root.join("be-refs.sqlite3");
    let mut open = OPEN.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((_, refs)) = open.iter().find(|(held, _)| *held == path) {
        return Ok(refs.clone());
    }
    let refs = be_client::RefsDb::open(&path)
        .map_err(|error| format!("this device's record of what it holds cannot open: {error}"))?;
    open.push((path, refs.clone()));
    Ok(refs)
}

pub struct Config {
    pub server_url: String,
    pub token: String,
    pub account: Uuid,
    pub organization: Uuid,
    pub identity: [u8; 32],
    pub display_name: String,
    pub email: String,
    pub data_dir: PathBuf,
    pub clock: be_client::Clock,
}

impl Config {
    fn socket_url(&self) -> String {
        crate::accounts::socket_url(&self.server_url)
    }
}

#[derive(Clone)]
pub struct Content {
    pub content_type: Uuid,
    bytes: Vec<u8>,
    pub revision: u64,
}

impl Content {
    pub fn new(content_type: Uuid, bytes: Vec<u8>) -> Self {
        Self {
            content_type,
            bytes,
            revision: 1,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Default)]
pub struct Status {
    pub running: bool,
    pub connected: bool,
    pub connecting: bool,
    pub blocks: usize,
    pub wakes: u64,
    pub unsaved: usize,
    pub kept: usize,
    pub saves: u64,
    pub conflicts: usize,
    pub conflicted: Vec<Uuid>,
    pub error: Option<String>,
    pub waiting: usize,
    pub syncing: bool,
    pub unsent: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsentEdit {
    pub block: Uuid,
    pub name: Option<String>,
    pub since: i64,
    pub offerable: bool,
}

pub use types::{host_block_types, is_known, kind_of, provided};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stale {
    Unreadable(String),
    Reshaped,
}

pub fn locked(block: Uuid) -> bool {
    with_shared(|shared| {
        shared.locked.contains(&block) || shared.graph.get(block).is_some_and(|node| node.locked)
    })
    .unwrap_or_default()
}

pub fn staleness(block: Uuid) -> Option<Stale> {
    let guard = stack().lock().unwrap_or_else(PoisonError::into_inner);
    let stack = guard.as_ref()?;
    if stack.tried.contains(&block) {
        return None;
    }
    let shared = stack.shared.lock().unwrap_or_else(PoisonError::into_inner);
    let node = shared.graph.get(block)?;
    let kind = kind_of(node.content_type)?;
    if kind.frozen {
        return None;
    }
    if let Some(error) = shared.unreadable.get(&block) {
        return Some(Stale::Unreadable(error.clone()));
    }
    let stamped = node.metadata.get(&be_block::SCHEMA)?;
    (stamped != kind.schema).then_some(Stale::Reshaped)
}

pub fn reset(block: Uuid) {
    let Some(node) = node(block) else {
        return;
    };
    let Some(kind) = kind_of(node.content_type) else {
        return;
    };
    if let Some(stack) = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        stack.tried.remove(&block);
    }
    replace(block, node.content_type, kind.blank);
}

pub fn try_anyway(block: Uuid) {
    if !matches!(staleness(block), Some(Stale::Reshaped)) {
        return;
    }
    if let Some(stack) = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
    {
        stack.tried.insert(block);
    }
    block_app_base::host::changed();
}

struct Stack {
    account: Uuid,
    organization: Uuid,
    commands: Option<UnboundedSender<Command>>,
    nudges: Option<nudge::Nudges>,
    held: std::collections::HashSet<Uuid>,
    versioned: std::collections::HashSet<Uuid>,
    tried: std::collections::HashSet<Uuid>,
    shared: Arc<Mutex<Shared>>,
    #[cfg_attr(not(test), allow(dead_code))]
    changed: Arc<Condvar>,
    running: platform::Running,
}

impl Drop for Stack {
    fn drop(&mut self) {
        self.commands = None;
        self.nudges = None;
        self.running.stop();
    }
}

fn stack() -> &'static Mutex<Option<Stack>> {
    static STACK: OnceLock<Mutex<Option<Stack>>> = OnceLock::new();
    STACK.get_or_init(|| Mutex::new(None))
}

pub fn start(config: Config) {
    stop();
    let (commands, receiver) = unbounded_channel();
    let (nudges, nudged) = nudge::channel();
    let shared = Arc::new(Mutex::new(Shared {
        wall: worker::Wall::new(config.clock.clone()),
        ..Shared::default()
    }));
    let changed = Arc::new(Condvar::new());
    let account = config.account;
    let organization = config.organization;
    let running = platform::spawn(
        config,
        worker::Inbox {
            commands: receiver,
            nudged,
        },
        Arc::clone(&shared),
        Arc::clone(&changed),
    );
    *stack().lock().unwrap_or_else(PoisonError::into_inner) = Some(Stack {
        account,
        organization,
        commands: Some(commands),
        nudges: Some(nudges),
        held: std::collections::HashSet::new(),
        versioned: std::collections::HashSet::new(),
        tried: std::collections::HashSet::new(),
        shared,
        changed,
        running,
    });
}

pub fn installed_for(account: Uuid, organization: Uuid) -> bool {
    stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .is_some_and(|stack| stack.account == account && stack.organization == organization)
}

pub fn stop() {
    let stack = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    if let Some(stack) = &stack {
        let connection = stack
            .shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .connection
            .take();
        if let Some(connection) = connection {
            connection.close();
        }
    }
    drop(stack);
}

pub fn stop_and_wait() {
    stop();
    platform::wait_for_stopped();
}

#[cfg(all(any(test, feature = "testing"), not(target_arch = "wasm32")))]
pub fn abandon() {
    let stack = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take();
    if let Some(mut stack) = stack {
        stack.running.abandon();
    }
}

pub fn retry_now() {
    if let Some(stack) = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        && let Some(nudges) = &stack.nudges
    {
        nudges.nudge();
    }
}

pub fn flush() -> usize {
    let unsaved = status().unsaved;
    let (done, sealed) = std::sync::mpsc::channel();
    send(Command::Flush(done));
    match platform::await_flush(sealed) {
        true => status().unsaved,
        false => unsaved,
    }
}

#[cfg(any(test, feature = "testing"))]
pub fn open(block: Uuid, content_type: Uuid) {
    if let Some(source) = revisions::source(block) {
        revisions::load(block, source, content_type);
        return;
    }
    show_staged(block);
    send(Command::Open {
        block,
        content_type,
        owned: false,
    });
}

#[cfg(any(test, feature = "testing"))]
pub fn close(block: Uuid) {
    if revisions::source(block).is_some() {
        return;
    }
    let held = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .is_some_and(|stack| stack.held.contains(&block));
    if !held {
        send(Command::Close(block));
    }
}

pub fn hold(block: Uuid, content_type: Uuid) {
    let fresh = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
        .is_some_and(|stack| stack.held.insert(block));
    if fresh {
        show_staged(block);
        send(Command::Open {
            block,
            content_type,
            owned: true,
        });
    }
}

pub fn change_child(
    block: Uuid,
    content_type: Uuid,
    change: be_block::ChildChange,
) -> Option<bool> {
    if !is_known(content_type) {
        return None;
    }
    send(Command::ChangeChild {
        block,
        content_type,
        change: api_child(change),
    });
    Some(true)
}

fn api_child(change: be_block::ChildChange) -> block_type_api::ChildChange {
    match change {
        be_block::ChildChange::Add(child) => block_type_api::ChildChange::Add(child),
        be_block::ChildChange::Delete(child) => block_type_api::ChildChange::Delete(child),
        be_block::ChildChange::Replace { old, new } => {
            block_type_api::ChildChange::Replace(block_type_api::ReplaceChild { old, new })
        }
    }
}

pub fn version(block: Uuid, command: block_plugin_api::VersionCommand) {
    send(Command::Version { block, command });
}

pub fn version_since(
    block: Uuid,
    content_type: Uuid,
    sent: Option<u64>,
) -> Option<(u64, block_plugin_api::VersionStatus)> {
    let fresh = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_mut()
        .is_some_and(|stack| stack.versioned.insert(block));
    if fresh {
        hold(block, content_type);
        send(Command::WatchVersion(block));
    }
    with_shared(|shared| {
        let state = shared.versions.get(&block)?;
        (sent != Some(state.revision)).then(|| (state.revision, state.status.clone()))
    })?
}

pub fn is_versioned(content_type: Uuid) -> bool {
    content_type == <be_block::Checkout as be_block::Root>::CONTENT_TYPE
        || content_type == <be_block::Repository as be_block::Root>::CONTENT_TYPE
}

pub fn operate(block: Uuid, operation: Vec<u8>) {
    send(Command::Operate(
        block,
        operation,
        block_app_base::host::clock(),
    ));
}

pub fn attach(block: Uuid, content_type: Uuid, editor: u64, editable: bool) {
    show_staged(block);
    send(Command::Attach {
        block,
        content_type,
        editor,
        editable,
    });
}

pub fn detach(block: Uuid, editor: u64) {
    send(Command::Detach { block, editor });
}

pub fn from_editor(block: Uuid, editor: u64, frame: &[u8]) {
    let Some(bytes) = replicas::unframe(frame) else {
        return;
    };
    send(Command::FromEditor {
        block,
        editor,
        bytes,
        at: block_app_base::host::clock(),
    });
}

pub use replicas::unframe;

pub fn take_editor_frames(editor: u64) -> Vec<(Uuid, Vec<u8>)> {
    with_shared_mut(|shared| shared.editor_frames.remove(&editor))
        .flatten()
        .unwrap_or_default()
}

pub fn keep_editor_frames(editor: u64, mut kept: Vec<(Uuid, Vec<u8>)>) {
    if kept.is_empty() {
        return;
    }
    with_shared_mut(|shared| {
        let held = shared.editor_frames.entry(editor).or_default();
        kept.append(held);
        *held = kept;
    });
}

pub fn snapshot_frame(block: Uuid) -> Option<Vec<u8>> {
    with_shared(|shared| {
        let content = match shared.revision_previews.get(&block) {
            Some(preview) => preview.content.as_ref()?,
            None => shared
                .blocks
                .get(&block)
                .filter(|_| shared.previewed.contains(&block))?,
        };
        Some(replicas::snapshot_frame(
            content.content_type,
            content.bytes().to_vec(),
        ))
    })?
}

#[cfg(test)]
pub fn show(block: Uuid, kind: Uuid, value: Option<Vec<u8>>) {
    send(Command::Presence { block, kind, value });
}

#[cfg(test)]
pub fn send_to_peers(block: Uuid, to: Option<u64>, channel: Uuid, payload: Vec<u8>) {
    send(Command::SendToPeers {
        block,
        to,
        channel,
        payload,
    });
}

pub fn history(block: Uuid) -> History {
    with_shared(|shared| shared.histories.get(&block).copied())
        .flatten()
        .unwrap_or_default()
}

pub fn undo(block: Uuid) {
    send(Command::History { block, redo: false });
}

pub fn redo(block: Uuid) {
    send(Command::History { block, redo: true });
}

pub fn duplicate(from: Uuid) -> Option<(Uuid, Uuid)> {
    let source = node(from)?;
    if !is_known(source.content_type) {
        return None;
    }
    let to = Uuid::now_v7();
    let mut metadata = source.metadata.clone();
    metadata.remove(&be_block::ARTIFACT);
    let references = source.contents();
    let author = account().unwrap_or_default();
    with_shared_mut(|shared| {
        shared.graph.change(Node {
            id: to,
            content_type: source.content_type,
            author: Some(author),
            parent: Some(be_graph::BlockParent::Detached),
            access: be_graph::Access::Manage,
            references,
            metadata: metadata.clone(),
            head: None,
            locked: false,
            held: true,
            sets_access: false,
        });
    });
    send(Command::Duplicate {
        from,
        to,
        content_type: source.content_type,
        metadata,
    });
    Some((to, source.content_type))
}

pub fn replace(block: Uuid, content_type: Uuid, bytes: Vec<u8>) {
    send(Command::Replace {
        block,
        content_type,
        bytes,
    });
}

pub fn seed(block: Uuid, content_type: Uuid, bytes: Vec<u8>) {
    send(Command::Seed {
        block,
        content_type,
        bytes,
    });
}

fn show_staged(block: Uuid) {
    with_shared_mut(|shared| {
        if shared.blocks.contains_key(&block) {
            return;
        }
        let Some(content) = shared
            .staged
            .get(&block)
            .filter(|(content_type, _)| is_known(*content_type))
            .map(|(content_type, bytes)| Content::new(*content_type, bytes.clone()))
        else {
            return;
        };
        shared.blocks.insert(block, content);
        shared.previewed.insert(block);
        shared.touch(block);
    });
}

fn shared_content(shared: &Shared, block: Uuid) -> Option<&Content> {
    match shared.revision_previews.get(&block) {
        Some(preview)
            if !preview.released
                && shared
                    .graph
                    .get(preview.source)
                    .is_some_and(|node| node.access.can_view()) =>
        {
            preview.content.as_ref()
        }
        Some(_) => None,
        None => shared.blocks.get(&block),
    }
}

pub fn content(block: Uuid) -> Option<Content> {
    with_shared(|shared| shared_content(shared, block).cloned())?
}

pub fn content_revision(block: Uuid) -> Option<u64> {
    with_shared(|shared| shared_content(shared, block).map(|content| content.revision))?
}

pub fn describe_block(block: Uuid) -> Option<be_block::Entries> {
    with_shared(|shared| {
        shared
            .described
            .get(&block)
            .map(|description| description.derived.clone())
    })?
}

pub fn pairing_requests() -> Vec<PairingRequest> {
    with_shared(|shared| shared.pairing.clone()).unwrap_or_default()
}

pub fn approve_pairing(from: u64, code: String) {
    send(Command::ApprovePairing { from, code });
}

pub fn dismiss_pairing(from: u64) {
    send(Command::DismissPairing(from));
}

pub fn contacts() -> Vec<be_client::Contact> {
    with_shared(|shared| shared.contacts.clone()).unwrap_or_default()
}

pub fn contacts_revision() -> u64 {
    with_shared(|shared| shared.contacts_revision).unwrap_or_default()
}

pub fn make_contact_link(
    offer: Option<(Uuid, be_graph::Access)>,
    reply: impl FnOnce(Result<String, String>) + Send + 'static,
) {
    if stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_none()
    {
        reply(Err("Open an organization first.".to_owned()));
        return;
    }
    send(Command::Contact(worker::ContactCommand::Link {
        offer,
        reply: Box::new(reply),
    }));
}

pub fn open_contact_link(
    code: be_keys::InviteCode,
    reply: impl FnOnce(Result<be_keys::ContactCard, String>) + Send + 'static,
) {
    send(Command::Contact(worker::ContactCommand::Open {
        code,
        reply: Box::new(reply),
    }));
}

pub fn accept_contact_link(
    code: be_keys::InviteCode,
    reply: impl FnOnce(Result<be_keys::ContactCard, String>) + Send + 'static,
) {
    send(Command::Contact(worker::ContactCommand::Accept {
        code,
        reply: Box::new(reply),
    }));
}

pub fn remove_contact(account: Uuid) {
    send(Command::Contact(worker::ContactCommand::Remove(account)));
}

pub fn escrow_warning() -> Option<&'static str> {
    with_shared(|shared| shared.escrow_warning).flatten()
}

pub fn status() -> Status {
    with_shared(|shared| Status {
        running: true,
        connected: shared.connected,
        connecting: !shared.connected && !shared.reached,
        blocks: shared.blocks.len(),
        wakes: shared.wakes,
        unsaved: shared.unsealed + shared.pending + shared.importing,
        kept: shared.waiting,
        saves: shared.saves,
        conflicts: shared.conflicts.values().map(Vec::len).sum(),
        conflicted: shared.conflicts.keys().copied().collect(),
        error: shared.error.clone(),
        waiting: shared.unsealed + shared.waiting,
        syncing: shared.draining || (shared.connected && shared.unsealed + shared.waiting > 0),
        unsent: shared.unsent.len(),
    })
    .unwrap_or_default()
}

pub fn unsent() -> Vec<UnsentEdit> {
    with_shared(|shared| {
        shared
            .unsent
            .iter()
            .map(|unsent| UnsentEdit {
                block: unsent.block,
                name: shared
                    .graph
                    .get(unsent.block)
                    .and_then(|node| node.metadata.name()),
                since: unsent.since,
                offerable: unsent.offerable,
            })
            .collect()
    })
    .unwrap_or_default()
}

pub fn restore_unsent(block: Uuid) {
    send(Command::Unsent {
        block,
        restore: true,
    });
}

pub fn discard_unsent(block: Uuid) {
    send(Command::Unsent {
        block,
        restore: false,
    });
}

fn send(command: Command) {
    let held = stack().lock().unwrap_or_else(PoisonError::into_inner);
    let Some(stack) = held.as_ref() else {
        return;
    };
    let Some(commands) = &stack.commands else {
        return;
    };
    stack
        .shared
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .pending += 1;
    if commands.send(command).is_err() {
        stack
            .shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pending -= 1;
    }
}

pub fn busy() -> Vec<&'static str> {
    with_shared(Shared::busy).unwrap_or_default()
}

pub fn seal_owed() {
    if with_shared(|shared| shared.owing > 0 && shared.pending == 0).unwrap_or_default() {
        let (done, _) = std::sync::mpsc::channel();
        send(Command::Flush(done));
    }
}

pub fn backgrounded() {
    let (done, _) = std::sync::mpsc::channel();
    send(Command::Flush(done));
}

#[cfg(all(any(test, feature = "testing"), not(target_arch = "wasm32")))]
pub fn wait_until<T>(read: impl Fn(&Shared) -> Option<T>) -> T {
    let (shared, changed) = stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|stack| (Arc::clone(&stack.shared), Arc::clone(&stack.changed)))
        .expect("the stack is running");
    let mut held = shared.lock().unwrap_or_else(PoisonError::into_inner);
    loop {
        if let Some(value) = read(&held) {
            return value;
        }
        held = changed.wait(held).unwrap_or_else(PoisonError::into_inner);
    }
}

#[cfg(all(any(test, feature = "testing"), not(target_arch = "wasm32")))]
pub fn fail_listings(query: Query, error: &str) {
    let query = sourced(query);
    with_shared_mut(|shared| {
        for listing in shared.graph.listings_of(query) {
            shared.graph.failed(listing, error.to_owned());
        }
    });
}

#[cfg(all(any(test, feature = "testing"), not(target_arch = "wasm32")))]
pub fn refetch(query: Query) {
    let query = sourced(query);
    with_shared_mut(|shared| {
        for listing in shared.graph.listings_of(query) {
            shared.graph.relist(listing);
        }
    });
    send(Command::Sync);
}

fn with_shared_mut<T>(change: impl FnOnce(&mut Shared) -> T) -> Option<T> {
    let held = stack().lock().unwrap_or_else(PoisonError::into_inner);
    let stack = held.as_ref()?;
    let mut shared = stack.shared.lock().unwrap_or_else(PoisonError::into_inner);
    Some(change(&mut shared))
}

pub fn account() -> Option<Uuid> {
    stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|stack| stack.account)
}

pub fn identity() -> Option<(Uuid, Uuid)> {
    stack()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        .map(|stack| (stack.account, stack.organization))
}

pub fn stale_revision() -> u64 {
    with_shared(|shared| shared.stale_revision).unwrap_or_default()
        + be_types_host::kinds_revision()
}

pub fn graph_revision() -> u64 {
    with_shared(|shared| shared.graph.revision).unwrap_or_default()
}

pub fn take_touched() -> stable_hash::HashSet<Uuid> {
    with_shared_mut(|shared| std::mem::take(&mut shared.touched)).unwrap_or_default()
}

pub fn ready(query: Query) -> bool {
    let query = sourced(query);
    with_shared(|shared| shared.graph.ready(query)).unwrap_or_default()
}

pub fn failure(query: Query) -> Option<String> {
    let query = sourced(query);
    with_shared(|shared| shared.graph.failure(query)).flatten()
}

pub fn want(owner: Owner, queries: Vec<Query>) {
    let queries: Vec<Query> = queries.into_iter().map(sourced).collect();
    let fetch = with_shared_mut(|shared| {
        if shared.graph.wanted(owner) == queries.as_slice() {
            return false;
        }
        shared.graph.want(owner, queries);
        shared.graph.fetching()
    })
    .unwrap_or_default();
    if fetch {
        send(Command::Sync);
    }
}

fn sourced(query: Query) -> Query {
    match query {
        Query::Children(id) => Query::Children(revisions::source(id).unwrap_or(id)),
        Query::References(id) => Query::References(revisions::source(id).unwrap_or(id)),
        Query::Backrefs(id) => Query::Backrefs(revisions::source(id).unwrap_or(id)),
        Query::Parents(id) => Query::Parents(revisions::source(id).unwrap_or(id)),
        Query::Subtree(id) => Query::Subtree(revisions::source(id).unwrap_or(id)),
        Query::Block(id) => Query::Block(revisions::source(id).unwrap_or(id)),
        Query::OpenSettings(id) => Query::OpenSettings(revisions::source(id).unwrap_or(id)),
        other => other,
    }
}

pub fn with_graph<T>(read: impl FnOnce(&Graph) -> T) -> Option<T> {
    with_shared(|shared| read(&shared.graph))
}

pub fn query(query: Query) -> Vec<Node> {
    if let Query::Block(id) = query
        && revisions::source(id).is_some()
    {
        return node(id).into_iter().collect();
    }
    let query = sourced(query);
    with_shared(|shared| shared.graph.query(query)).unwrap_or_default()
}

pub fn node(block: Uuid) -> Option<Node> {
    with_shared(|shared| {
        if let Some(preview) = shared.revision_previews.get(&block) {
            let mut node = shared.graph.get(preview.source)?.clone();
            node.id = block;
            node.access = if preview.released {
                be_graph::Access::None
            } else {
                node.access.min(be_graph::Access::View)
            };
            return Some(node);
        }
        shared.graph.get(block).cloned()
    })?
}

pub fn may_load(block: Uuid) -> bool {
    node(block).is_none_or(|node| node.access.can_view())
        && with_shared(|shared| !shared.unreadable.contains_key(&block)).unwrap_or_default()
}

pub fn access(block: Uuid) -> be_graph::Access {
    node(block).map_or(be_graph::Access::None, |node| node.access)
}

thread_local! {
    static PENDING_NAMES: std::cell::RefCell<std::collections::HashMap<Uuid, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn name_when_created(block: Uuid, name: String) {
    if node(block).is_some() {
        set_name(block, Some(name));
        return;
    }
    PENDING_NAMES.with(|pending| pending.borrow_mut().insert(block, name));
}

pub fn create(
    block: Uuid,
    content_type: Uuid,
    parent: be_graph::BlockParent,
    mut metadata: be_block::BlockMetadata,
    content: Option<Vec<u8>>,
) {
    if let Some(name) = PENDING_NAMES.with(|pending| pending.borrow_mut().remove(&block)) {
        metadata.insert(&be_block::NAME, &name);
    }
    if metadata.derived.is_empty()
        && content.is_none()
        && let Some(kind) = kind_of(content_type)
    {
        metadata.derived = kind.described.derived;
    }
    let author = account().unwrap_or_default();
    with_shared_mut(|shared| {
        if let be_graph::BlockParent::Block(above) = parent
            && shared
                .graph
                .get(above)
                .is_some_and(|above| !above.references.contains(&block))
        {
            shared.graph.placing(block, true);
        }
        shared.graph.change(Node {
            id: block,
            content_type,
            author: Some(author),
            parent: Some(parent),
            access: be_graph::Access::Manage,
            references: Vec::new(),
            metadata: metadata.clone(),
            head: None,
            locked: false,
            held: true,
            sets_access: false,
        });
    });
    send(Command::Create {
        block,
        content_type,
        parent,
        metadata,
        bytes: content,
    });
}

pub fn placing(block: Uuid, placing: bool) {
    with_shared_mut(|shared| shared.graph.placing(block, placing));
}

pub fn set_parent(block: Uuid, parent: be_graph::BlockParent) {
    let node = node(block);
    if node
        .as_ref()
        .is_some_and(|node| node.parent == Some(parent))
    {
        return;
    }
    if let Some(node) = node
        && !node.metadata.contains(&be_block::VIEW_EDITOR)
        && node.access.can_edit()
    {
        let from = match (node.parent, parent) {
            (Some(be_graph::BlockParent::Root), be_graph::BlockParent::Detached) => {
                Some(be_block::DeletedFrom::Root)
            }
            (Some(be_graph::BlockParent::Block(old)), be_graph::BlockParent::Detached) => {
                Some(be_block::DeletedFrom::Block(old))
            }
            _ => None,
        };
        let restored = parent != be_graph::BlockParent::Detached;
        if from.is_some() || (restored && node.metadata.contains(&be_block::DELETED_FROM)) {
            define(
                block,
                vec![be_block::DELETED_FROM.definition(from.as_ref())],
            );
        }
    }
    with_shared_mut(|shared| {
        let placing = match parent {
            be_graph::BlockParent::Block(above) => shared
                .graph
                .get(above)
                .is_some_and(|above| !above.references.contains(&block)),
            be_graph::BlockParent::Root | be_graph::BlockParent::Detached => false,
        };
        shared.graph.placing(block, placing);
        shared
            .graph
            .update(block, |node| node.parent = Some(parent))
    });
    send(Command::SetParent { block, parent });
}

pub fn define(block: Uuid, changes: Vec<be_block::Definition>) {
    let Some(metadata) = node(block).map(|node| node.metadata) else {
        return;
    };
    let mut next = metadata.clone();
    next.define(&changes);
    if next == metadata {
        return;
    }
    let derived = describe_block(block);
    with_shared_mut(|shared| {
        shared.graph.update(block, |node| node.metadata = next);
        if let Some(derived) = derived {
            shared.graph.derive(block, derived);
        }
    });
    send(Command::Define { block, changes });
}

pub fn set_name(block: Uuid, name: Option<String>) {
    define(block, vec![be_block::NAME.definition(name.as_ref())]);
}

pub fn describe_implicitly(block: Uuid, derived: be_block::Entries) {
    with_shared_mut(|shared| shared.graph.derive(block, derived));
}

pub fn set_access(block: Uuid, account: Uuid, access: be_graph::Access) {
    send(Command::SetAccess {
        block,
        account,
        access,
    });
}

pub fn leave(block: Uuid) {
    send(Command::Leave(block));
}

type MembersReply = Box<dyn FnOnce(Result<Vec<be_protocol::Member>, String>) + Send>;

pub fn list_members(reply: MembersReply) {
    send(Command::Members(worker::MembersCommand::List(reply)));
}

pub fn set_role(account: Uuid, role: be_protocol::OrganizationRole, reply: MembersReply) {
    send(Command::Members(worker::MembersCommand::SetRole {
        account,
        role,
        reply,
    }));
}

pub fn remove_member(account: Uuid, reply: MembersReply) {
    send(Command::Members(worker::MembersCommand::Remove {
        account,
        reply,
    }));
}

pub async fn invite(email: String, role: be_protocol::OrganizationRole) -> Result<String, String> {
    let (reply, made) = tokio::sync::oneshot::channel();
    send(Command::Invite {
        email,
        role,
        reply: Box::new(move |made| {
            let _ = reply.send(made);
        }),
    });
    made.await.unwrap_or_else(|_| Err(NOT_OPEN.to_owned()))
}

pub async fn create_agent(name: String) -> Result<crate::accounts::AgentCredentials, String> {
    let (reply, made) = tokio::sync::oneshot::channel();
    send(Command::CreateAgent {
        name,
        reply: Box::new(move |made| {
            let _ = reply.send(made);
        }),
    });
    made.await.unwrap_or_else(|_| Err(NOT_OPEN.to_owned()))
}

const NOT_OPEN: &str = "Open the organization first.";

pub fn list_access(
    block: Uuid,
    reply: impl FnOnce(Result<be_protocol::AccessList, String>) + Send + 'static,
) {
    send(Command::ListAccess {
        block,
        reply: Box::new(reply),
    });
}

pub fn read_head(
    block: Uuid,
    commit: be_commit::CommitId,
    reply: impl FnOnce(Result<Vec<u8>, String>) + Send + 'static,
) {
    send(Command::ReadHead {
        block,
        commit,
        reply: Box::new(reply),
    });
}

type Exported = Result<(String, Vec<u8>), String>;

pub fn export(block: Uuid, reply: impl FnOnce(Exported) + Send + 'static) {
    send(Command::Export {
        block,
        reply: Box::new(reply),
    });
}

pub fn plain_file(
    block: Uuid,
    reply: impl FnOnce(Result<be_types_host::PlainFile, String>) + Send + 'static,
) {
    send(Command::PlainFile {
        block,
        reply: Box::new(reply),
    });
}

pub fn from_file(block: Uuid, content_type: Uuid, name: String, bytes: Vec<u8>) {
    send(Command::FromFile {
        block,
        content_type,
        name,
        bytes,
    });
}

pub fn import(bytes: &[u8], parent: be_graph::BlockParent) -> Result<(Uuid, Uuid), String> {
    let blocks = be_client::transfer::renumber(be_client::transfer::decode(bytes)?);
    let author = account().ok_or("Sign in to import blocks.")?;
    let top = (blocks[0].id, blocks[0].block_type);
    let mut imported = Vec::with_capacity(blocks.len());
    for block in blocks.iter().cloned() {
        let placed = match block.parent {
            Some(position) => be_graph::BlockParent::Block(blocks[position].id),
            None => parent,
        };
        with_shared_mut(|shared| {
            shared.graph.change(Node {
                id: block.id,
                content_type: block.block_type,
                author: Some(author),
                parent: Some(placed),
                access: be_graph::Access::Manage,
                references: block.references.clone(),
                metadata: block.metadata.clone(),
                head: None,
                locked: false,
                held: true,
                sets_access: false,
            });
        });
        if let Some(bytes) = &block.content {
            with_shared_mut(|shared| {
                shared
                    .staged
                    .insert(block.id, (block.block_type, bytes.clone()))
            });
        }
        imported.push((block, placed));
    }
    send(Command::Import(imported));
    Ok(top)
}

pub fn import_file(
    name: String,
    bytes: Vec<u8>,
    parent: be_graph::BlockParent,
    reply: impl FnOnce(Result<(Uuid, Uuid), String>) + Send + 'static,
) {
    if account().is_none() {
        reply(Err("Sign in to import files.".to_owned()));
        return;
    }
    let metadata = be_block::BlockMetadata::named(&name);
    send(Command::ContentOfFile {
        name,
        bytes,
        reply: Box::new(move |content| {
            reply(content.map(|(content_type, content)| {
                let block = Uuid::new_v4();
                create(block, content_type, parent, metadata, Some(content));
                (block, content_type)
            }));
        }),
    });
}

fn with_shared<T>(read: impl FnOnce(&Shared) -> T) -> Option<T> {
    let held = stack().lock().unwrap_or_else(PoisonError::into_inner);
    let stack = held.as_ref()?;
    let shared = stack.shared.lock().unwrap_or_else(PoisonError::into_inner);
    Some(read(&shared))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
