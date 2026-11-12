use be_lib_stable_hash::HashMap;
use std::{
    cell::Cell,
    collections::BTreeMap,
    rc::{Rc, Weak},
};

use be_sdk_api::{
    EditorInstanceId,
    blocks::{BlockSubject, BlockValue, OpenRequest as WireOpen, ShowRequest},
    guest::blocks,
    ops::{
        DeserializeOwned, Window,
        guest::{Pending, spawn},
    },
};
use be_sdk_type_guest::editor::{FromEditor, ToEditor};
use uuid::Uuid;

use crate::{
    BlockQuery, EditorHost, GraphCommand,
    host::{ArtifactState, BlockHistory, OpenRequest, StoredHead},
};

struct Held {
    content_type: Uuid,
    replica: blocks::Replica,
    window: Rc<Cell<Window>>,
}

#[derive(Default)]
pub(crate) struct Link {
    instance: Option<EditorInstanceId>,
    own: Option<(Uuid, Uuid)>,
    replicas: HashMap<Uuid, Held>,
    watches: BTreeMap<Subject, Rc<blocks::BlockWatch>>,
    searches: BTreeMap<crate::Search, Rc<blocks::SearchWatch>>,
    shows: Option<Rc<blocks::ShowRequests>>,
}

/// What one `block-watch` follows (`block-subject`).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Subject {
    Blocks(BlockQuery),
    History(Uuid),
    Head(Uuid),
    Artifact(Uuid),
    Version,
}

impl Subject {
    fn encode(self) -> BlockSubject {
        match self {
            Self::Blocks(query) => BlockSubject::Blocks(query.encode()),
            Self::History(block) => BlockSubject::History(block.into_bytes()),
            Self::Head(block) => BlockSubject::Head(block.into_bytes()),
            Self::Artifact(block) => BlockSubject::Artifact(block.into_bytes()),
            Self::Version => BlockSubject::Version,
        }
    }

    /// Hands `value` to the editor's host, when it is a value of this subject.
    fn deliver(self, host: &EditorHost, value: BlockValue) {
        match (self, value) {
            (Self::Blocks(query), BlockValue::Blocks(Ok(listed))) => host.set_blocks(
                query,
                listed.into_iter().map(crate::BlockInfo::decode).collect(),
            ),
            (Self::Blocks(query), BlockValue::Blocks(Err(error))) => {
                host.fail_blocks(query, error.to_string());
            }
            (Self::History(block), BlockValue::History(history)) => host.set_history(
                block,
                BlockHistory {
                    head: history.head,
                    can_undo: history.can_undo,
                    can_redo: history.can_redo,
                    restarted_at: history.restarted_at,
                    lost_edits: history.lost_edits,
                },
            ),
            (Self::Head(block), BlockValue::Head(head)) => host.set_head(
                block,
                head.map(|head| {
                    head.map(|head| StoredHead {
                        commit: head.commit,
                        content_type: Uuid::from_bytes(head.content_type),
                        bytes: head.bytes,
                    })
                }),
            ),
            (Self::Artifact(block), BlockValue::Artifact(state)) => host.set_artifact(
                block,
                state.map(|state| ArtifactState {
                    block_id: block,
                    source_type: Uuid::from_bytes(state.source_type),
                    source: Uuid::from_bytes(state.source),
                    summary: state.summary,
                    error: state.error,
                    regenerating: state.regenerating,
                }),
            ),
            (Self::Version, BlockValue::Version(status)) => host.set_version_status(status),
            _ => {}
        }
    }
}

fn follow<W: 'static, T: DeserializeOwned + 'static>(
    waker: crate::Waker,
    watch: &Rc<W>,
    next: fn(&W) -> Pending<T>,
    mut deliver: impl FnMut(T) + 'static,
) {
    let watch: Weak<W> = Rc::downgrade(watch);
    spawn(async move {
        loop {
            let Some(held) = watch.upgrade() else {
                return;
            };
            let pending = next(&held);
            drop(held);
            match pending.await {
                Ok(value) => {
                    deliver(value);
                    waker.wake();
                }
                Err(_) => return,
            }
        }
    });
}

fn reconcile<K: Ord + Copy, W>(
    held: &mut BTreeMap<K, Rc<W>>,
    wanted: &[K],
    mut open: impl FnMut(K) -> Rc<W>,
) {
    held.retain(|key, _| wanted.contains(key));
    for key in wanted {
        if !held.contains_key(key) {
            let watch = open(*key);
            held.insert(*key, watch);
        }
    }
}

impl Link {
    pub(crate) fn connect(&mut self, block: Uuid, block_type: Uuid) {
        self.own = Some((block, block_type));
    }

    fn under<R>(&self, run: impl FnOnce() -> R) -> R {
        under(self.instance, run)
    }

    pub(crate) fn exchange(&mut self, instance: EditorInstanceId, host: &EditorHost) {
        self.instance = Some(instance);
        self.change_graph(host);
        self.watch(host);
        self.send_frames(host);
        for seeded in host.take_seeded_content() {
            let (block, content_type) =
                (seeded.block.into_bytes(), seeded.content_type.into_bytes());
            self.under(|| {
                blocks::write_content(block, content_type, seeded.bytes, seeded.replace).detach()
            });
        }
        for open in host.take_opens() {
            self.under(|| blocks::open_block(request_of(open)).detach());
        }
        for (block, command) in host.take_block_commands() {
            self.under(|| blocks::act(block.into_bytes(), command).detach());
        }
        for (block, command) in host.take_version_commands() {
            self.under(|| blocks::version(block.into_bytes(), command).detach());
        }
    }

    fn change_graph(&self, host: &EditorHost) {
        for command in host.take_graph_commands() {
            self.under(|| match command {
                GraphCommand::Create {
                    id,
                    block_type,
                    parent,
                    defined,
                    content,
                    owner_only,
                } => blocks::create_block(
                    id.into_bytes(),
                    block_type.into_bytes(),
                    parent.encode(),
                    crate::graph::wire_entries(&defined),
                    content,
                    owner_only,
                )
                .detach(),
                GraphCommand::SetParent { id, parent } => {
                    blocks::set_parent(id.into_bytes(), parent.encode()).detach();
                }
                GraphCommand::Define { id, key, value } => {
                    blocks::define(id.into_bytes(), key.into_bytes(), value).detach();
                }
            });
        }
    }

    fn watch(&mut self, host: &EditorHost) {
        if let Some(watched) = host.take_content_watch() {
            let own = self.own.map(|(block, _)| block);
            self.replicas.retain(|block, held| {
                Some(*block) == own
                    || watched.iter().any(|(kept, content_type)| {
                        kept == block && *content_type == held.content_type
                    })
            });
            for (block, content_type) in watched {
                self.hold(host, block, content_type);
            }
        }
        if let Some(wanted) = host.take_block_subjects() {
            let parent = self.instance;
            reconcile(&mut self.watches, &wanted, |subject| {
                let watch = Rc::new(under(parent, || blocks::BlockWatch::new(subject.encode())));
                let host = host.clone();
                follow(host.waker(), &watch, blocks::BlockWatch::next, move |value| {
                    subject.deliver(&host, value);
                });
                watch
            });
        }
        if let Some(searches) = host.take_search_watch() {
            let parent = self.instance;
            self.searches.retain(|search, _| searches.contains(search));
            for search in searches {
                if self.searches.contains_key(&search) {
                    continue;
                }
                let watch = Rc::new(under(parent, || {
                    blocks::SearchWatch::new(
                        search.text.clone(),
                        search.within.map(Uuid::into_bytes),
                    )
                }));
                let (host, asked) = (host.clone(), search.clone());
                follow(
                    host.waker(),
                    &watch,
                    blocks::SearchWatch::next,
                    move |results| {
                        host.set_search(asked.clone(), crate::SearchResults::decode(results));
                    },
                );
                self.searches.insert(search, watch);
            }
        }
        if self.shows.is_none() && host.wants_shows() {
            let watch = Rc::new(self.under(blocks::ShowRequests::new));
            let host = host.clone();
            follow(
                host.waker(),
                &watch,
                blocks::ShowRequests::next,
                move |requests| {
                    for request in requests {
                        match request {
                            ShowRequest::Block(open) => host.show_block(crate::ShowRequest {
                                block_id: Uuid::from_bytes(open.block_id),
                                block_type: Uuid::from_bytes(open.block_type),
                                via: open.via.map(Uuid::from_bytes),
                                editor: open.editor.map(Uuid::from_bytes),
                            }),
                            ShowRequest::Panel(panel) => host.show_panel(panel),
                            ShowRequest::History(history) => {
                                host.history_requested(crate::HistoryRequest {
                                    block_id: Uuid::from_bytes(history.block_id),
                                    commit: history.commit,
                                });
                            }
                        }
                    }
                },
            );
            self.shows = Some(watch);
        }
    }

    fn hold(&mut self, host: &EditorHost, block: Uuid, content_type: Uuid) {
        if self
            .replicas
            .get(&block)
            .is_some_and(|held| held.content_type == content_type)
        {
            return;
        }
        let parent = self.instance;
        let replica = under(parent, || {
            blocks::Replica::new(block.into_bytes(), content_type.into_bytes())
        });
        let mut frames = replica.frames();
        let host = host.clone();
        let shown = (self.own.map(|(own, _)| own) != Some(block)).then_some(block);
        spawn(async move {
            while let Some(frame) = frames.recv().await {
                if let Some(message) = ToEditor::from_frame(&frame) {
                    host.receive_from_replica(shown, message);
                    host.waker().wake();
                }
            }
        });
        self.replicas.insert(
            block,
            Held {
                content_type,
                replica,
                window: Rc::default(),
            },
        );
    }

    fn send(
        &mut self,
        host: &EditorHost,
        block: Option<Uuid>,
        message: FromEditor,
    ) -> Option<FromEditor> {
        let target = match (block, self.own) {
            (Some(block), _) => block,
            (None, Some((own, _))) => own,
            (None, None) => return None,
        };
        if let Some((own, block_type)) = self.own
            && own == target
        {
            self.hold(host, own, block_type);
        }
        let held = self.replicas.get(&target)?;
        if !held.window.get().open() {
            return Some(message);
        }
        let frame = message.frame();
        let bytes = frame.len();
        let mut window = held.window.get();
        window.take(bytes);
        held.window.set(window);
        let accepted = held.replica.send(frame);
        let room = Rc::downgrade(&held.window);
        let waker = host.waker();
        spawn(async move {
            let _ = accepted.await;
            let Some(room) = room.upgrade() else {
                return;
            };
            let mut window = room.get();
            let was_open = window.open();
            window.grant(1, bytes as u64);
            room.set(window);
            if !was_open && window.open() {
                waker.wake();
            }
        });
        None
    }

    fn send_frames(&mut self, host: &EditorHost) {
        host.take_outgoing(|block, message| self.send(host, block, message));
    }
}

fn under<R>(instance: Option<EditorInstanceId>, run: impl FnOnce() -> R) -> R {
    crate::asked::asking(instance, run)
}

fn request_of(open: OpenRequest) -> WireOpen {
    WireOpen {
        block_id: open.block_id.into_bytes(),
        block_type: open.block_type.into_bytes(),
        via: open.via.map(Uuid::into_bytes),
        editor: open.editor.map(Uuid::into_bytes),
    }
}

#[cfg(test)]
mod tests;
