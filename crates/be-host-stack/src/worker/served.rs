//! The lane of commands that need only the server (`Command::served`): they run in order in tasks
//! beside the worker's loop, so a server that stopped answering holds up only them, not the
//! loop and the commands queued in it (guides/blocks/host.md, Commands that need the server).

use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    rc::Rc,
    sync::{Arc, PoisonError},
};

use be_wasm_threads::Mutex;
use uuid::Uuid;

use super::{Command, Peer, Shared, Store, Tasks};

#[derive(Default)]
pub(super) struct Served {
    /// Held by the command that runs, so they reach the server in the order they were sent.
    lane: Rc<tokio::sync::Mutex<()>>,
    /// The blocks the lane's commands need, counted, until each command ends.
    holding: Rc<RefCell<HashMap<Uuid, usize>>>,
    /// Commands for the loop that wait for the lane to be done with a block they need.
    deferred: Rc<RefCell<VecDeque<Command>>>,
}

impl Served {
    /// Hands `command` to the lane; it counts as unsynced (`Shared::serving`) until it ends.
    pub(super) fn start(
        &mut self,
        peer: &Arc<Peer<Store>>,
        shared: &Arc<Mutex<Shared>>,
        tasks: &Tasks,
        command: Command,
    ) {
        let needs = command.needs();
        let served_tasks = tasks.clone();
        self.run(shared, tasks, needs, {
            let (peer, shared) = (Arc::clone(peer), Arc::clone(shared));
            async move {
                super::serve_command(&peer, &shared, &served_tasks, command).await;
            }
        });
    }

    fn run(
        &mut self,
        shared: &Arc<Mutex<Shared>>,
        tasks: &Tasks,
        needs: Vec<Uuid>,
        work: impl Future<Output = ()> + 'static,
    ) {
        {
            let mut holding = self.holding.borrow_mut();
            for block in &needs {
                *holding.entry(*block).or_default() += 1;
            }
        }
        shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .serving += 1;
        let (lane, holding, shared) = (
            Rc::clone(&self.lane),
            Rc::clone(&self.holding),
            Arc::clone(shared),
        );
        tasks.spawn(async move {
            let _turn = lane.lock().await;
            work.await;
            {
                let mut holding = holding.borrow_mut();
                for block in needs {
                    if let Some(count) = holding.get_mut(&block) {
                        *count -= 1;
                        if *count == 0 {
                            holding.remove(&block);
                        }
                    }
                }
            }
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .serving -= 1;
            be_host_base::host::wake();
        });
    }

    /// The command back when nothing it needs is in the lane and nothing deferred before it
    /// needs it too; otherwise it waits, and `runnable` hands it back once the lane is done.
    pub(super) fn defer(&mut self, command: Command) -> Option<Command> {
        let needs = command.needs();
        let behind = self.deferred.borrow().iter().any(|earlier| {
            earlier
                .subject()
                .is_some_and(|subject| needs.contains(&subject))
        });
        if !behind && !self.holds(&command) {
            return Some(command);
        }
        self.deferred.borrow_mut().push_back(command);
        None
    }

    pub(super) fn runnable(&mut self) -> Vec<Command> {
        let mut runnable = Vec::new();
        let mut waiting = std::collections::HashSet::new();
        let deferred = std::mem::take(&mut *self.deferred.borrow_mut());
        for command in deferred {
            let behind = command.needs().iter().any(|block| waiting.contains(block));
            match behind || self.holds(&command) {
                true => {
                    waiting.extend(command.subject());
                    self.deferred.borrow_mut().push_back(command);
                }
                false => runnable.push(command),
            }
        }
        runnable
    }

    /// Whether a command in the lane needs `block`: what it changes there (a key it rotates, a
    /// parent it moves) is not settled until it ends.
    pub(super) fn holds_block(&self, block: Uuid) -> bool {
        self.holding.borrow().contains_key(&block)
    }

    fn holds(&self, command: &Command) -> bool {
        let holding = self.holding.borrow();
        command
            .needs()
            .iter()
            .any(|block| holding.contains_key(block))
    }
}
