use foldhash::HashMap;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};

use crate::computation::{Computation, no_computation};
use crate::scope::Owner;
use crate::signal::Tracked;

pub(crate) struct Runtime {
    pub(crate) observer: RefCell<Weak<Computation>>,
    pub(crate) owner: RefCell<Weak<Owner>>,
    pub(crate) pending: RefCell<Weak<Computation>>,
    pub(crate) probe: RefCell<Option<Probe>>,
    pub(crate) queue: RefCell<VecDeque<Weak<Computation>>>,
    pub(crate) depth: Cell<usize>,
    pub(crate) flushing: Cell<bool>,
    pub(crate) memo_depth: Cell<usize>,
    pub(crate) zone: Cell<u64>,
    pub(crate) parked: RefCell<HashMap<u64, Vec<Weak<Computation>>>>,
    pub(crate) sequence: Cell<u64>,
    pub(crate) flushes: Cell<u64>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            observer: RefCell::new(no_computation()),
            owner: RefCell::default(),
            pending: RefCell::new(no_computation()),
            probe: RefCell::default(),
            queue: RefCell::default(),
            depth: Cell::default(),
            flushing: Cell::default(),
            memo_depth: Cell::default(),
            zone: Cell::default(),
            parked: RefCell::default(),
            sequence: Cell::default(),
            flushes: Cell::default(),
        }
    }
}

thread_local! {
    pub(crate) static RUNTIME: Runtime = Runtime::default();
}

pub(crate) struct Reset<T: Copy + 'static> {
    cell: &'static std::thread::LocalKey<Runtime>,
    field: fn(&Runtime) -> &Cell<T>,
    previous: T,
}

impl<T: Copy> Reset<T> {
    pub(crate) fn set(field: fn(&Runtime) -> &Cell<T>, value: T) -> Self {
        Self {
            cell: &RUNTIME,
            field,
            previous: RUNTIME.with(|runtime| field(runtime).replace(value)),
        }
    }
}

impl<T: Copy> Drop for Reset<T> {
    fn drop(&mut self) {
        self.cell
            .with(|runtime| (self.field)(runtime).set(self.previous));
    }
}

pub(crate) type Read = (Rc<dyn Tracked>, u64);

pub(crate) struct Probe {
    pub(crate) read: Vec<Read>,
    pub(crate) parent: Weak<Owner>,
    pub(crate) owner: Option<Rc<Owner>>,
}

pub(crate) struct Probed<T> {
    pub(crate) value: T,
    pub(crate) read: Vec<Read>,
    pub(crate) owner: Option<Rc<Owner>>,
}

impl<T> Probed<T> {
    pub(crate) fn constant(&self) -> bool {
        self.read.is_empty() && self.owner.as_ref().is_none_or(|owner| owner.is_inert())
    }
}

pub(crate) struct Context {
    observer: Weak<Computation>,
    owner: Weak<Owner>,
    pending: Weak<Computation>,
    probe: Option<Probe>,
}

impl Context {
    pub(crate) fn enter(observer: Weak<Computation>, owner: Weak<Owner>) -> Self {
        Self::enter_pending(observer, owner, no_computation())
    }

    pub(crate) fn enter_pending(
        observer: Weak<Computation>,
        owner: Weak<Owner>,
        pending: Weak<Computation>,
    ) -> Self {
        RUNTIME.with(|runtime| Self {
            observer: runtime.observer.replace(observer),
            owner: runtime.owner.replace(owner),
            pending: runtime.pending.replace(pending),
            probe: runtime.probe.take(),
        })
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        RUNTIME.with(|runtime| {
            runtime.observer.replace(self.observer.clone());
            runtime.owner.replace(self.owner.clone());
            runtime.pending.replace(self.pending.clone());
            runtime.probe.replace(self.probe.take());
        });
    }
}

pub fn untrack<T>(f: impl FnOnce() -> T) -> T {
    let (owner, pending) = RUNTIME.with(|runtime| {
        (
            runtime.owner.borrow().clone(),
            runtime.pending.borrow().clone(),
        )
    });
    let _context = Context::enter_pending(no_computation(), owner, pending);
    f()
}

pub(crate) fn probe<T>(memo: bool, f: impl FnOnce() -> T) -> Probed<T> {
    let parent = Rc::downgrade(&crate::scope::current_owner());
    let memo_depth = RUNTIME.with(|runtime| runtime.memo_depth.get());
    let _memo_depth = Reset::set(
        |runtime| &runtime.memo_depth,
        memo_depth + usize::from(memo),
    );
    batch(|| {
        let _context = Context::enter(no_computation(), Weak::new());
        RUNTIME.with(|runtime| {
            runtime.probe.replace(Some(Probe {
                read: Vec::new(),
                parent,
                owner: None,
            }))
        });
        let value = f();
        let probe = RUNTIME
            .with(|runtime| runtime.probe.take())
            .expect("the probe is still running");
        Probed {
            value,
            read: probe.read,
            owner: probe.owner,
        }
    })
}

pub(crate) fn tracking() -> bool {
    RUNTIME.with(|runtime| {
        runtime.probe.borrow().is_some()
            || runtime
                .observer
                .borrow()
                .upgrade()
                .is_some_and(|observer| !observer.is_disposed())
    })
}

pub fn batch<T>(f: impl FnOnce() -> T) -> T {
    let depth = RUNTIME.with(|runtime| runtime.depth.get());
    let guard = Reset::set(|runtime| &runtime.depth, depth + 1);
    let result = f();
    drop(guard);
    flush();
    result
}

pub(crate) fn next_sequence() -> u64 {
    RUNTIME.with(|runtime| {
        let next = runtime.sequence.get() + 1;
        runtime.sequence.set(next);
        next
    })
}

pub(crate) fn enqueue(computation: &Rc<Computation>) {
    if !computation.queued.replace(true) {
        computation.queued_at.set(next_sequence());
        RUNTIME.with(|runtime| {
            runtime
                .queue
                .borrow_mut()
                .push_back(Rc::downgrade(computation))
        });
    }
}

pub(crate) fn flush() {
    if RUNTIME.with(|runtime| runtime.depth.get() != 0 || runtime.flushing.get()) {
        return;
    }
    let _guard = Reset::set(|runtime| &runtime.flushing, true);
    let flush = next_flush();
    loop {
        let next = RUNTIME.with(|runtime| runtime.queue.borrow_mut().pop_front());
        let Some(next) = next else { break };
        run_queued(next, flush);
    }
}

pub(crate) fn flush_within(root: &Owner) {
    let depth = RUNTIME.with(|runtime| runtime.depth.get());
    let _depth = Reset::set(|runtime| &runtime.depth, depth + 1);
    let flush = next_flush();
    loop {
        let next = RUNTIME.with(|runtime| {
            let mut queue = runtime.queue.borrow_mut();
            let at = queue.iter().position(|queued| {
                queued
                    .upgrade()
                    .is_some_and(|computation| computation.owned_within(root))
            })?;
            queue.remove(at)
        });
        let Some(next) = next else { break };
        run_queued(next, flush);
    }
}

fn next_flush() -> u64 {
    RUNTIME.with(|runtime| {
        let flush = runtime.flushes.get() + 1;
        runtime.flushes.set(flush);
        flush
    })
}

fn run_queued(next: Weak<Computation>, flush: u64) {
    let Some(computation) = next.upgrade() else {
        return;
    };
    if let Some(gate) = computation.closed_gate() {
        gate.park(next);
        return;
    }
    let current = RUNTIME.with(|runtime| runtime.zone.get());
    if computation.zone != 0 && current != 0 && computation.zone != current {
        RUNTIME.with(|runtime| {
            runtime
                .parked
                .borrow_mut()
                .entry(computation.zone)
                .or_default()
                .push(next);
        });
        return;
    }
    computation.queued.set(false);
    if computation.count_run(flush) > 10_000 {
        computation.dispose();
        panic!("reactive effects did not settle after 10000 executions");
    }
    computation.refresh();
}

pub struct ZoneGuard {
    previous: u64,
}

impl Drop for ZoneGuard {
    fn drop(&mut self) {
        RUNTIME.with(|runtime| runtime.zone.set(self.previous));
    }
}

pub fn enter_zone(zone: u64) -> ZoneGuard {
    let previous = RUNTIME.with(|runtime| runtime.zone.replace(zone));
    let parked = RUNTIME.with(|runtime| runtime.parked.borrow_mut().remove(&zone));
    if let Some(parked) = parked {
        RUNTIME.with(|runtime| runtime.queue.borrow_mut().extend(parked));
        flush();
    }
    ZoneGuard { previous }
}

pub fn zone_pending(zone: u64) -> bool {
    RUNTIME.with(|runtime| {
        runtime.parked.borrow().get(&zone).is_some_and(|parked| {
            parked
                .iter()
                .any(|computation| computation.strong_count() > 0)
        })
    })
}

pub fn forget_zone(zone: u64) {
    RUNTIME.with(|runtime| runtime.parked.borrow_mut().remove(&zone));
}

pub(crate) fn current_zone() -> u64 {
    RUNTIME.with(|runtime| runtime.zone.get())
}
