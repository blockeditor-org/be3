//! The lane of server work beside the worker's loop (guides/blocks/host.md, Commands that need
//! the server): commands that need only the server (`Command::served`), and version commands
//! after the loop's part of them (`version`), run one at a time in the order sent, each in a
//! task, so a server that stopped answering holds up only the lane, not the loop and the edits
//! queued in it.
//!
//! Order. A command the loop carries out waits (`defer`) while:
//! - the lane holds a block it needs (`Command::needs`), until the lane is done with it;
//! - it is not order-safe (`Command::order_safe`: it may reach blocks it does not name, as a
//!   version command reaches a whole tree), until the lane is empty;
//! - a command deferred before it needs one of its blocks, or is not order-safe: so nothing
//!   overtakes what was sent first.

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet, VecDeque},
    rc::Rc,
    sync::{Arc, PoisonError},
};

use be_wasm_threads::Mutex;
use uuid::Uuid;

use super::{Command, Peer, Shared, Store, Tasks};
use crate::version::{Context, LoopCalls, Reach, Versions};

#[derive(Default)]
pub(super) struct Served {
    /// Held by the command that runs, so they reach the server in the order they were sent.
    lane: Rc<tokio::sync::Mutex<()>>,
    /// Commands in the lane, running or waiting their turn.
    in_lane: Rc<Cell<usize>>,
    /// The blocks the lane's commands need, counted, until each command ends.
    holding: Rc<RefCell<HashMap<Uuid, usize>>>,
    /// Commands for the loop that wait (see the module's doc).
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

    /// Hands a version command to the lane, after the loop sealed the open replicas
    /// (`Ran::version`): it waits for the outbox to be sent, since it reads a whole tree from
    /// the server, then runs, reaching the replicas through the loop (`calls`).
    pub(super) fn version(
        &mut self,
        peer: &Arc<Peer<Store>>,
        shared: &Arc<Mutex<Shared>>,
        tasks: &Tasks,
        calls: &LoopCalls,
        block: Uuid,
        command: be_sdk_api::VersionCommand,
    ) {
        let (peer, shared, served_tasks, calls) = (
            Arc::clone(peer),
            Arc::clone(shared),
            tasks.clone(),
            calls.clone(),
        );
        // The busy flag set by `Ran::version` lives in this stack's `Shared`: a task dropped
        // with the stack (stopped or restarted) cannot leave it set on the next, which starts
        // from a fresh `Shared`.
        self.run(&shared.clone(), tasks, vec![block], async move {
            served_tasks.sent(&peer, &shared, None).await;
            let outcome = Versions::default()
                .run(
                    &mut Context {
                        peer: &peer,
                        shared: &shared,
                        replicas: Reach::Loop(&calls),
                    },
                    block,
                    command,
                )
                .await;
            super::set_busy(&shared, block, false, outcome.err());
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
        self.in_lane.set(self.in_lane.get() + 1);
        shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .serving += 1;
        let (lane, holding, in_lane, shared) = (
            Rc::clone(&self.lane),
            Rc::clone(&self.holding),
            Rc::clone(&self.in_lane),
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
            in_lane.set(in_lane.get() - 1);
            shared
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .serving -= 1;
            be_host_base::host::wake();
        });
    }

    /// The command back when it may run now (see the module's doc); otherwise it waits, and
    /// `runnable` hands it back once it may.
    pub(super) fn defer(&mut self, command: Command) -> Option<Command> {
        let behind = {
            let deferred = self.deferred.borrow();
            let needs = command.needs();
            deferred.iter().any(|earlier| {
                !earlier.order_safe()
                    || !command.order_safe()
                    || earlier.needs().iter().any(|block| needs.contains(block))
            })
        };
        if !behind && !self.waits(&command) {
            return Some(command);
        }
        self.deferred.borrow_mut().push_back(command);
        None
    }

    pub(super) fn runnable(&mut self) -> Vec<Command> {
        let mut runnable = Vec::new();
        let mut waiting = HashSet::new();
        let mut barred = false;
        let deferred = std::mem::take(&mut *self.deferred.borrow_mut());
        for command in deferred {
            let behind = barred
                || (!command.order_safe() && !waiting.is_empty())
                || command.needs().iter().any(|block| waiting.contains(block));
            match behind || self.waits(&command) {
                true => {
                    waiting.extend(command.needs());
                    barred |= !command.order_safe();
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

    fn waits(&self, command: &Command) -> bool {
        if !command.order_safe() && self.in_lane.get() > 0 {
            return true;
        }
        let holding = self.holding.borrow();
        command
            .needs()
            .iter()
            .any(|block| holding.contains_key(block))
    }
}

#[cfg(test)]
mod tests;
