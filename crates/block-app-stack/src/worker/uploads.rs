use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
    sync::{Arc, PoisonError},
};

use wasi_threads::Mutex;

use be_client::transfer::Saved;
use be_graph::BlockParent;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use uuid::Uuid;

use super::{Command, Peer, Shared, Store, platform};

struct Queue {
    above: HashMap<Uuid, Vec<Uuid>>,
    below: HashMap<Uuid, usize>,
    unkept: HashSet<Uuid>,
    peer: Option<Arc<Peer<Store>>>,
    shared: Arc<Mutex<Shared>>,
    finished: UnboundedSender<()>,
}

pub(super) struct Uploads {
    queue: Rc<RefCell<Queue>>,
    finishing: UnboundedReceiver<()>,
    deferred: VecDeque<Command>,
}

impl Uploads {
    pub(super) fn new(shared: &Arc<Mutex<Shared>>) -> Self {
        let (finished, finishing) = unbounded_channel();
        Self {
            queue: Rc::new(RefCell::new(Queue {
                above: HashMap::new(),
                below: HashMap::new(),
                unkept: HashSet::new(),
                peer: None,
                shared: Arc::clone(shared),
                finished,
            })),
            finishing,
            deferred: VecDeque::new(),
        }
    }

    pub(super) fn connect(&mut self, peer: &Arc<Peer<Store>>) {
        self.queue.borrow_mut().peer = Some(Arc::clone(peer));
    }

    pub(super) fn add(&mut self, blocks: Vec<(Saved, BlockParent)>) {
        let peer = {
            let mut queue = self.queue.borrow_mut();
            let ids: Vec<Uuid> = blocks.iter().map(|(block, _)| block.id).collect();
            for (block, parent) in &blocks {
                let above = match block.parent {
                    Some(position) => {
                        let mut above = queue.above[&ids[position]].clone();
                        above.push(ids[position]);
                        above
                    }
                    None => parent.block().into_iter().collect(),
                };
                for held in &above {
                    *queue.below.entry(*held).or_default() += 1;
                }
                queue.above.insert(block.id, above);
                queue.unkept.insert(block.id);
            }
            queue.report();
            queue.peer.clone()
        };
        let Some(peer) = peer else {
            return;
        };
        let queue = Rc::clone(&self.queue);
        platform::spawn_local(async move {
            let shared = Arc::clone(&queue.borrow().shared);
            let (sender, mut receiver) = unbounded_channel();
            let uploading = {
                let queue = Rc::clone(&queue);
                let peer = Arc::clone(&peer);
                async move {
                    let failed = sender.clone();
                    let queued = peer
                        .queue_import(&blocks, |block, imported| {
                            let _ = failed.send((block, imported));
                        })
                        .await;
                    queue.borrow_mut().kept(&blocks);
                    peer.upload_import(queued, move |block, imported| {
                        let _ = sender.send((block, imported));
                    })
                    .await;
                }
            };
            let settling = async {
                while let Some((block, imported)) = receiver.recv().await {
                    super::imported(&peer, &shared, block, imported).await;
                    queue.borrow_mut().finish(block);
                    block_app_base::host::wake();
                }
            };
            futures_util::future::join(uploading, settling).await;
        });
    }

    pub(super) fn is_empty(&self) -> bool {
        self.queue.borrow().above.is_empty() && self.deferred.is_empty()
    }

    pub(super) fn defer(&mut self, command: Command) -> Option<Command> {
        let needs = command.needs();
        let behind = self.deferred.iter().any(|earlier| {
            earlier
                .subject()
                .is_some_and(|subject| needs.contains(&subject))
        });
        if !behind && !self.waits(&command) {
            return Some(command);
        }
        self.deferred.push_back(command);
        None
    }

    pub(super) fn runnable(&mut self) -> Vec<Command> {
        let mut runnable = Vec::new();
        let mut waiting: HashSet<Uuid> = HashSet::new();
        for command in std::mem::take(&mut self.deferred) {
            let behind = command.needs().iter().any(|block| waiting.contains(block));
            match behind || self.waits(&command) {
                true => {
                    waiting.extend(command.subject());
                    self.deferred.push_back(command);
                }
                false => runnable.push(command),
            }
        }
        runnable
    }

    fn waits(&self, command: &Command) -> bool {
        let queue = self.queue.borrow();
        command
            .needs()
            .iter()
            .any(|block| queue.above.contains_key(block))
            || command
                .rekeys()
                .is_some_and(|block| queue.below.get(&block).is_some_and(|count| *count > 0))
    }

    pub(super) async fn next(&mut self) {
        if self.finishing.recv().await.is_none() {
            std::future::pending::<()>().await;
        }
    }
}

impl Queue {
    fn kept(&mut self, blocks: &[(Saved, BlockParent)]) {
        for (block, _) in blocks {
            self.unkept.remove(&block.id);
        }
        self.report();
    }

    fn finish(&mut self, block: Uuid) {
        let Some(above) = self.above.remove(&block) else {
            return;
        };
        for held in above {
            if let Some(count) = self.below.get_mut(&held) {
                *count -= 1;
                if *count == 0 {
                    self.below.remove(&held);
                }
            }
        }
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .staged
            .remove(&block);
        self.report();
        let _ = self.finished.send(());
    }

    fn report(&self) {
        let mut shared = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        shared.uploading = self.above.len();
        shared.importing = self.unkept.len();
    }
}

#[cfg(test)]
mod tests;
