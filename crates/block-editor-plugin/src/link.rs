use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap},
    rc::{Rc, Weak},
};

use block_plugin_api::{
    EditorInstanceId,
    blocks::{OpenRequest as WireOpen, ShowRequest},
    guest::blocks,
    ops::{
        DeserializeOwned, OpError, Window,
        guest::{Pending, spawn},
    },
};
use block_type_guest::editor::{FromEditor, ToEditor};
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
    blocks: BTreeMap<BlockQuery, Rc<blocks::BlocksWatch>>,
    histories: BTreeMap<Uuid, Rc<blocks::HistoryWatch>>,
    heads: BTreeMap<Uuid, Rc<blocks::HeadWatch>>,
    artifacts: BTreeMap<Uuid, Rc<blocks::ArtifactWatch>>,
    version: Option<Rc<blocks::VersionWatch>>,
    shows: Option<Rc<blocks::ShowRequests>>,
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
                Err(OpError::Canceled | OpError::Unsupported | OpError::Denied) => return,
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
            self.under(|| match seeded.replace {
                true => blocks::replace_content(block, content_type, seeded.bytes),
                false => blocks::seed_content(block, content_type, seeded.bytes),
            });
        }
        for open in host.take_opens() {
            self.under(|| blocks::open_block(request_of(open)));
        }
        for (block, command) in host.take_block_commands() {
            self.under(|| blocks::act(block.into_bytes(), command));
        }
        for (block, command) in host.take_version_commands() {
            self.under(|| blocks::version(block.into_bytes(), command));
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
                } => blocks::create_block(
                    id.into_bytes(),
                    block_type.into_bytes(),
                    parent.encode(),
                    crate::graph::wire_entries(&defined),
                    content,
                ),
                GraphCommand::SetParent { id, parent } => {
                    blocks::set_parent(id.into_bytes(), parent.encode());
                }
                GraphCommand::Define { id, key, value } => {
                    blocks::define(id.into_bytes(), key.into_bytes(), value);
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
        if let Some(queries) = host.take_block_watch() {
            let parent = self.instance;
            reconcile(&mut self.blocks, &queries, |query| {
                let watch = Rc::new(under(parent, || blocks::BlocksWatch::new(query.encode())));
                let host = host.clone();
                follow(
                    host.waker(),
                    &watch,
                    blocks::BlocksWatch::next,
                    move |listed| match listed {
                        Ok(listed) => host.set_blocks(
                            query,
                            listed.into_iter().map(crate::BlockInfo::decode).collect(),
                        ),
                        Err(error) => host.fail_blocks(query, error.to_string()),
                    },
                );
                watch
            });
        }
        if let Some(watched) = host.take_history_watch() {
            let parent = self.instance;
            reconcile(&mut self.histories, &watched, |block| {
                let watch = Rc::new(under(parent, || {
                    blocks::HistoryWatch::new(block.into_bytes())
                }));
                let host = host.clone();
                follow(
                    host.waker(),
                    &watch,
                    blocks::HistoryWatch::next,
                    move |history| {
                        host.set_history(
                            block,
                            BlockHistory {
                                head: history.head,
                                can_undo: history.can_undo,
                                can_redo: history.can_redo,
                            },
                        );
                    },
                );
                watch
            });
        }
        if let Some(watched) = host.take_head_watch() {
            let parent = self.instance;
            reconcile(&mut self.heads, &watched, |block| {
                let watch = Rc::new(under(parent, || blocks::HeadWatch::new(block.into_bytes())));
                let host = host.clone();
                follow(host.waker(), &watch, blocks::HeadWatch::next, move |head| {
                    host.set_head(
                        block,
                        head.map(|head| {
                            head.map(|head| StoredHead {
                                commit: head.commit,
                                content_type: Uuid::from_bytes(head.content_type),
                                bytes: head.bytes,
                            })
                        }),
                    );
                });
                watch
            });
        }
        if let Some(watched) = host.take_artifact_watch() {
            let parent = self.instance;
            reconcile(&mut self.artifacts, &watched, |block| {
                let watch = Rc::new(under(parent, || {
                    blocks::ArtifactWatch::new(block.into_bytes())
                }));
                let host = host.clone();
                follow(
                    host.waker(),
                    &watch,
                    blocks::ArtifactWatch::next,
                    move |state| {
                        host.set_artifact(
                            block,
                            state.map(|state| ArtifactState {
                                block_id: block,
                                source_type: Uuid::from_bytes(state.source_type),
                                source: state.source.map(Uuid::from_bytes),
                                summary: state.summary,
                                error: state.error,
                                regenerating: state.regenerating,
                            }),
                        );
                    },
                );
                watch
            });
        }
        if self.version.is_none() && host.wants_version() {
            let watch = Rc::new(self.under(blocks::VersionWatch::new));
            let host = host.clone();
            follow(
                host.waker(),
                &watch,
                blocks::VersionWatch::next,
                move |status| {
                    host.set_version_status(status);
                },
            );
            self.version = Some(watch);
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
