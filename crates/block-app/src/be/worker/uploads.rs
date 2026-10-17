use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
    sync::{Arc, Mutex, PoisonError},
};

use be_client::transfer::Saved;
use be_graph::BlockParent;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use uuid::Uuid;

use super::{Command, Peer, Shared, Store, platform};

const IN_FLIGHT: usize = 64;

struct Upload {
    block: Saved,
    parent: BlockParent,
    after: Option<Uuid>,
    above: Vec<Uuid>,
}

struct Queue {
    uploads: HashMap<Uuid, Upload>,
    ready: VecDeque<Uuid>,
    blocked: HashMap<Uuid, Vec<Uuid>>,
    running: HashSet<Uuid>,
    urgent: HashSet<Uuid>,
    below: HashMap<Uuid, usize>,
    generation: u64,
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
                uploads: HashMap::new(),
                ready: VecDeque::new(),
                blocked: HashMap::new(),
                running: HashSet::new(),
                urgent: HashSet::new(),
                below: HashMap::new(),
                generation: 0,
                peer: None,
                shared: Arc::clone(shared),
                finished,
            })),
            finishing,
            deferred: VecDeque::new(),
        }
    }

    pub(super) fn connect(&mut self, peer: &Arc<Peer<Store>>) {
        {
            let mut queue = self.queue.borrow_mut();
            queue.generation += 1;
            queue.peer = Some(Arc::clone(peer));
            let running: Vec<Uuid> = queue.running.drain().collect();
            for id in running {
                queue.ready.push_front(id);
            }
        }
        start(&self.queue);
    }

    pub(super) fn add(&mut self, blocks: Vec<(Saved, BlockParent)>) {
        {
            let mut queue = self.queue.borrow_mut();
            let ids: Vec<Uuid> = blocks.iter().map(|(block, _)| block.id).collect();
            for (block, parent) in blocks {
                let after = block
                    .parent
                    .map(|position| ids[position])
                    .filter(|parent| queue.uploads.contains_key(parent));
                let id = block.id;
                let above = match after {
                    Some(after) => {
                        queue.blocked.entry(after).or_default().push(id);
                        let mut above = queue.uploads[&after].above.clone();
                        above.push(after);
                        above
                    }
                    None => {
                        queue.ready.push_back(id);
                        parent.block().into_iter().collect()
                    }
                };
                for block in &above {
                    *queue.below.entry(*block).or_default() += 1;
                }
                queue.uploads.insert(
                    id,
                    Upload {
                        block,
                        parent,
                        after,
                        above,
                    },
                );
            }
            queue.report();
        }
        start(&self.queue);
    }

    pub(super) fn is_empty(&self) -> bool {
        self.queue.borrow().uploads.is_empty() && self.deferred.is_empty()
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
        let mut queue = self.queue.borrow_mut();
        for block in needs {
            queue.hurry(block);
        }
        drop(queue);
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
            .any(|block| queue.uploads.contains_key(block))
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
    fn hurry(&mut self, block: Uuid) {
        let mut next = Some(block);
        while let Some(block) = next {
            let Some(upload) = self.uploads.get(&block) else {
                return;
            };
            if !self.urgent.insert(block) {
                return;
            }
            if let Some(position) = self.ready.iter().position(|ready| *ready == block) {
                self.ready.remove(position);
                self.ready.push_front(block);
            }
            next = upload.after;
        }
    }

    fn finish(&mut self, id: Uuid) {
        if let Some(upload) = self.uploads.remove(&id) {
            for block in upload.above {
                if let Some(count) = self.below.get_mut(&block) {
                    *count -= 1;
                    if *count == 0 {
                        self.below.remove(&block);
                    }
                }
            }
        }
        self.running.remove(&id);
        self.urgent.remove(&id);
        for child in self.blocked.remove(&id).unwrap_or_default() {
            match self.urgent.contains(&child) {
                true => self.ready.push_front(child),
                false => self.ready.push_back(child),
            }
        }
        let mut held = self.shared.lock().unwrap_or_else(PoisonError::into_inner);
        held.staged.remove(&id);
        held.uploading = self.uploads.len();
    }

    fn report(&self) {
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .uploading = self.uploads.len();
    }
}

fn start(queue: &Rc<RefCell<Queue>>) {
    let mut held = queue.borrow_mut();
    let Some(peer) = held.peer.clone() else {
        return;
    };
    while held.running.len() < IN_FLIGHT
        && let Some(id) = held.ready.pop_front()
    {
        let Some(upload) = held.uploads.get(&id) else {
            continue;
        };
        let (block, parent) = (upload.block.clone(), upload.parent);
        let (peer, shared) = (Arc::clone(&peer), Arc::clone(&held.shared));
        let (queue, generation) = (Rc::clone(queue), held.generation);
        held.running.insert(id);
        platform::spawn_local(async move {
            super::import(&peer, &shared, &block, parent).await;
            {
                let mut held = queue.borrow_mut();
                if held.generation != generation {
                    return;
                }
                held.finish(id);
                let _ = held.finished.send(());
            }
            start(&queue);
            crate::host::wake();
        });
    }
}

#[cfg(test)]
mod tests;
