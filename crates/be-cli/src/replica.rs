use std::marker::PhantomData;
use std::sync::{Arc, PoisonError};

use std::time::Instant;
use wasi_threads::Mutex;

use be_block::LiveEdit;
use be_client::Event;
use be_client::{ClientError, Peer};
use be_commit::CommitId;
use be_protocol::{ClientId, SessionState};
use be_store::ObjectStore;
use be_types_host::{Conflict, HOST_EDITOR, Replicas, Source};
use block_type_api::{Description, EditorId, Status};
use block_type_guest::editor::{EditorMirror, FromEditor, PeerMessage, PeerPresence, ToEditor};
use tokio::sync::{broadcast, watch};
use uuid::Uuid;

use crate::session::explain;

const CLOSING: std::time::Duration = std::time::Duration::from_secs(5);

pub struct Held<S> {
    frames: Vec<ToEditor>,
    errors: Vec<String>,
    released: bool,
    store: PhantomData<fn() -> S>,
}

impl<S> Default for Held<S> {
    fn default() -> Self {
        Self {
            frames: Vec::new(),
            errors: Vec::new(),
            released: false,
            store: PhantomData,
        }
    }
}

fn held<S>(shared: &Arc<Mutex<Held<S>>>) -> std::sync::MutexGuard<'_, Held<S>> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

impl<S: ObjectStore + 'static> be_types_host::Host for Held<S> {
    type Store = S;

    fn spawn(task: impl Future<Output = ()> + 'static) {
        tokio::task::spawn_local(task);
    }

    fn wake() {}

    fn record(shared: &Arc<Mutex<Self>>, error: ClientError) {
        held(shared).errors.push(explain(error));
    }

    fn report(shared: &Arc<Mutex<Self>>, _: Option<Uuid>, message: String) {
        held(shared).errors.push(message);
    }

    fn set_locked(shared: &Arc<Mutex<Self>>, block: Uuid) {
        held(shared).errors.push(format!(
            "this account holds no key that opens block {block}"
        ));
    }

    fn owned(_: &Arc<Mutex<Self>>, _: Uuid) -> bool {
        false
    }

    fn reshaped(_: &Arc<Mutex<Self>>, _: Uuid, _: &str) -> bool {
        false
    }

    fn reset_notice() {}

    fn known_head(_: &Arc<Mutex<Self>>, _: Uuid) -> Option<CommitId> {
        None
    }

    fn to_editor(shared: &Arc<Mutex<Self>>, _: Uuid, editor: EditorId, bytes: Vec<u8>) -> bool {
        if editor == HOST_EDITOR
            && let Some(frame) = ToEditor::decode(&bytes)
        {
            held(shared).frames.push(frame);
        }
        true
    }

    fn content(_: &Arc<Mutex<Self>>, _: Uuid, _: Uuid, _: Vec<u8>) {}

    fn released(shared: &Arc<Mutex<Self>>, _: Uuid) {
        held(shared).released = true;
    }

    fn status(_: &Arc<Mutex<Self>>, _: Uuid, _: &Status) {}

    fn described(_: &Arc<Mutex<Self>>, _: Uuid, _: Vec<Uuid>, _: Description) {}

    fn history(_: &Arc<Mutex<Self>>, _: Uuid, _: bool, _: bool) {}

    fn conflict(_: &Arc<Mutex<Self>>, _: Uuid, _: Conflict) {}

    fn counted(_: &Arc<Mutex<Self>>, _: usize, _: usize) {}
}

pub enum Woken {
    Event(Box<Option<Event>>),
    Module,
    Deadline,
    Closed,
}

pub struct Session<S: ObjectStore + 'static, C: LiveEdit + Clone + Default> {
    peer: Arc<Peer<S>>,
    block: Uuid,
    shared: Arc<Mutex<Held<S>>>,
    replicas: Replicas<Held<S>>,
    events: broadcast::Receiver<Event>,
    closed: watch::Receiver<bool>,
    mirror: EditorMirror<C>,
}

impl<S: ObjectStore + 'static, C: LiveEdit + Clone + Default> Session<S, C> {
    pub async fn open(
        peer: Arc<Peer<S>>,
        block: Uuid,
        sources: Vec<Source>,
    ) -> Result<Self, String> {
        let content_type = C::CONTENT_TYPE;
        let replicas =
            Replicas::new(be_types_host::started(sources, be_client::Clock::real()).await);
        let events = peer.connection().subscribe();
        let closed = peer.connection().closed();
        let mut session = Self {
            peer,
            block,
            shared: Arc::new(Mutex::new(Held::default())),
            replicas,
            events,
            closed,
            mirror: EditorMirror::default(),
        };
        if !be_types_host::is_known(content_type) {
            return Err(format!("no type module has the block type {content_type}"));
        }
        session
            .replicas
            .open((&session.peer, &session.shared), block, content_type, true)
            .await;
        if !session.replicas.is_open(block) {
            let errors = std::mem::take(&mut held(&session.shared).errors);
            return Err(match errors.is_empty() {
                true => format!("block {block} could not be opened"),
                false => errors.join("; "),
            });
        }
        session.settle().await;
        Ok(session)
    }

    pub fn peer(&self) -> &Arc<Peer<S>> {
        &self.peer
    }

    pub fn show(&mut self, kind: Uuid, value: Option<Vec<u8>>) {
        self.say(FromEditor::ShowPresence { kind, value });
    }

    pub fn send(&mut self, to: Option<ClientId>, channel: Uuid, payload: Vec<u8>) {
        self.say(FromEditor::SendToPeers {
            to,
            channel,
            payload,
        });
    }

    pub fn edit(&mut self, operation: C::Op) {
        if let Some(message) = self.mirror.operate(operation) {
            self.say(message);
        }
    }

    pub fn content(&self) -> Option<&C> {
        self.mirror.content()
    }

    pub fn status(&self) -> Option<Status> {
        self.replicas.status(self.block)
    }

    pub fn is_owner(&self) -> bool {
        self.status().is_some_and(|status| status.owner)
    }

    pub fn client(&self) -> Option<ClientId> {
        self.replicas.session(self.block).map(|(client, _)| client)
    }

    pub fn state(&self) -> Option<&SessionState> {
        self.replicas.session(self.block).map(|(_, state)| state)
    }

    pub fn claim(&mut self) {
        self.replicas.claim(self.block);
    }

    pub async fn seal(&mut self) {
        self.replicas
            .seal((&self.peer, &self.shared), self.block, false)
            .await;
        self.settle().await;
    }

    fn say(&mut self, message: FromEditor) {
        self.replicas
            .send_as_host(self.block, &message, Instant::now());
    }

    pub fn presence(&self) -> &[PeerPresence] {
        self.mirror.presence()
    }

    pub fn take_messages(&mut self) -> Vec<PeerMessage> {
        self.mirror.take_peer_messages()
    }

    pub async fn next(&mut self) -> Woken {
        if *self.closed.borrow() {
            return Woken::Closed;
        }
        let wake_at = self.replicas.modules.wake_at();
        let clock = self.replicas.modules.clock().clone();
        let sleeping = async {
            match wake_at {
                Some(at) => clock.sleep_until(at).await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            event = self.events.recv() => match event {
                Ok(event) => Woken::Event(Box::new(Some(event))),
                Err(broadcast::error::RecvError::Lagged(_)) => Woken::Event(Box::new(None)),
                Err(broadcast::error::RecvError::Closed) => Woken::Closed,
            },
            _ = self.closed.changed() => Woken::Closed,
            () = self.replicas.woken() => Woken::Module,
            () = sleeping => Woken::Deadline,
        }
    }

    pub async fn handle(&mut self, woken: Woken) -> Result<(), String> {
        let ctx = (&self.peer, &self.shared);
        match woken {
            Woken::Closed => return Err("the connection to the server closed".to_owned()),
            Woken::Event(event) => match *event {
                Some(event) => self.replicas.heard(ctx, &event),
                None => self.replicas.resynchronize(ctx),
            },
            Woken::Deadline | Woken::Module => self.replicas.modules.tick_due(),
        }
        self.settle().await;
        Ok(())
    }

    pub async fn settle(&mut self) {
        loop {
            self.replicas.pump((&self.peer, &self.shared)).await;
            while self.replicas.relaying() || (self.replicas.joining() && !*self.closed.borrow()) {
                self.replicas.woken().await;
                self.replicas.pump((&self.peer, &self.shared)).await;
            }
            let (frames, errors) = {
                let mut held = held(&self.shared);
                (
                    std::mem::take(&mut held.frames),
                    std::mem::take(&mut held.errors),
                )
            };
            for error in errors {
                eprintln!("be: {error}");
            }
            let answers: Vec<FromEditor> = frames
                .into_iter()
                .filter_map(|frame| self.mirror.receive(frame))
                .collect();
            if answers.is_empty() {
                return;
            }
            for answer in answers {
                self.say(answer);
            }
        }
    }

    pub async fn close(mut self) {
        self.replicas.close(self.block);
        let closing = async {
            while !held(&self.shared).released || self.replicas.leaving() {
                let woken = self.next().await;
                if self.handle(woken).await.is_err() {
                    return;
                }
            }
        };
        let _ = tokio::time::timeout(CLOSING, closing).await;
    }
}
