use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};

use crate::computation::{Computation, no_computation};
use crate::scope::Owner;
use crate::signal::Tracked;

type Wake = Rc<dyn Fn()>;

pub(crate) struct Runtime {
    pub(crate) observer: RefCell<Weak<Computation>>,
    pub(crate) owner: RefCell<Weak<Owner>>,
    pub(crate) pending: RefCell<Weak<Computation>>,
    pub(crate) probe: RefCell<Option<Probe>>,
    pub(crate) queue: RefCell<VecDeque<Weak<Computation>>>,
    pub(crate) late: RefCell<VecDeque<Weak<Computation>>>,
    pub(crate) depth: Cell<usize>,
    pub(crate) flushing: Cell<bool>,
    pub(crate) memo_depth: Cell<usize>,
    pub(crate) zone: Cell<u64>,
    pub(crate) parked: RefCell<Vec<(u64, Vec<Weak<Computation>>)>>,
    pub(crate) wakes: RefCell<Vec<(u64, Wake)>>,
    pub(crate) waker: RefCell<Option<Wake>>,
    pub(crate) woken: Cell<bool>,
    pub(crate) sequence: Cell<u64>,
    pub(crate) flushes: Cell<u64>,
    pub(crate) removals: Cell<u64>,
    #[cfg(debug_assertions)]
    pub(crate) component_body: Cell<bool>,
}

impl Runtime {
    const fn new() -> Self {
        Self {
            observer: RefCell::new(Weak::<Computation<fn() -> bool>>::new()),
            owner: RefCell::new(Weak::new()),
            pending: RefCell::new(Weak::<Computation<fn() -> bool>>::new()),
            probe: RefCell::new(None),
            queue: RefCell::new(VecDeque::new()),
            late: RefCell::new(VecDeque::new()),
            depth: Cell::new(0),
            flushing: Cell::new(false),
            memo_depth: Cell::new(0),
            zone: Cell::new(0),
            parked: RefCell::new(Vec::new()),
            wakes: RefCell::new(Vec::new()),
            waker: RefCell::new(None),
            woken: Cell::new(false),
            sequence: Cell::new(0),
            flushes: Cell::new(0),
            removals: Cell::new(0),
            #[cfg(debug_assertions)]
            component_body: Cell::new(false),
        }
    }

    fn take_parked(&self, zone: u64) -> Option<Vec<Weak<Computation>>> {
        let mut parked = self.parked.borrow_mut();
        let at = parked.iter().position(|(held, _)| *held == zone)?;
        Some(parked.swap_remove(at).1)
    }
}

thread_local! {
    pub(crate) static RUNTIME: Runtime = const { Runtime::new() };
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
    let _body = leave_component_body();
    f()
}

pub fn component_body<T>(f: impl FnOnce() -> T) -> T {
    #[cfg(debug_assertions)]
    let _body = Reset::set(|runtime| &runtime.component_body, true);
    f()
}

pub(crate) struct LeaveBody {
    #[cfg(debug_assertions)]
    _reset: Reset<bool>,
}

pub(crate) fn leave_component_body() -> LeaveBody {
    LeaveBody {
        #[cfg(debug_assertions)]
        _reset: Reset::set(|runtime| &runtime.component_body, false),
    }
}

#[cfg(debug_assertions)]
fn short_type_name(name: &str) -> String {
    let mut short = String::new();
    let mut word = String::new();
    for character in name.chars() {
        if character.is_alphanumeric() || character == '_' {
            word.push(character);
        } else if character == ':' {
            word.clear();
        } else {
            short.push_str(&word);
            word.clear();
            short.push(character);
        }
    }
    short + &word
}

#[track_caller]
pub fn check_tracked_read<T: ?Sized>() {
    #[cfg(debug_assertions)]
    if RUNTIME.with(|runtime| runtime.component_body.get()) {
        let name = short_type_name(std::any::type_name::<T>());
        panic!(
            "a component body read `{name}` with `get` or `with`, which subscribes nothing \
             because a component body runs once. Move the read into a closure, memo or effect \
             so it updates; or pass the signal or memo itself as the prop; or, if a one-time \
             snapshot is intended, use `get_untracked()` or `with_untracked()`"
        );
    }
}

pub(crate) fn probe<T>(memo: bool, f: impl FnOnce() -> T) -> Probed<T> {
    let parent = Rc::downgrade(&crate::scope::current_owner());
    let memo_depth = RUNTIME.with(|runtime| runtime.memo_depth.get());
    let _memo_depth = Reset::set(
        |runtime| &runtime.memo_depth,
        match memo {
            true => memo_depth + 1,
            false => 0,
        },
    );
    defer(|| {
        let _context = Context::enter(no_computation(), Weak::new());
        let _body = leave_component_body();
        RUNTIME.with(|runtime| {
            runtime.probe.replace(Some(Probe {
                read: Vec::new(),
                parent,
                owner: None,
            }))
        });
        let value = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
            Ok(value) => value,
            Err(panic) => {
                let owner = RUNTIME.with(|runtime| runtime.probe.take()?.owner);
                if let Some(owner) = owner {
                    owner.dispose();
                }
                std::panic::resume_unwind(panic);
            }
        };
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

pub(crate) fn in_memo() -> bool {
    RUNTIME.with(|runtime| runtime.memo_depth.get() > 0)
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

pub(crate) fn batch<T>(f: impl FnOnce() -> T) -> T {
    let result = nested(f);
    settle();
    result
}

pub(crate) fn defer<T>(f: impl FnOnce() -> T) -> T {
    let result = nested(f);
    wake_if_pending();
    result
}

fn nested<T>(f: impl FnOnce() -> T) -> T {
    let depth = RUNTIME.with(|runtime| runtime.depth.get());
    let _depth = Reset::set(|runtime| &runtime.depth, depth + 1);
    f()
}

pub fn wake_with(wake: impl Fn() + 'static) {
    RUNTIME.with(|runtime| runtime.waker.replace(Some(Rc::new(wake))));
}

pub(crate) fn wake_if_pending() {
    let wake = RUNTIME.with(|runtime| {
        if runtime.depth.get() != 0
            || runtime.flushing.get()
            || runtime.woken.get()
            || runtime.queue.borrow().is_empty()
        {
            return None;
        }
        let wake = runtime.waker.borrow().clone()?;
        runtime.woken.set(true);
        Some(wake)
    });
    if let Some(wake) = wake {
        wake();
    }
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
        wake_if_pending();
    }
}

pub(crate) fn settle() {
    if RUNTIME.with(|runtime| runtime.depth.get() == 0) {
        flush();
    }
}

pub fn flush() {
    if RUNTIME.with(|runtime| runtime.flushing.get()) {
        return;
    }
    let _guard = Reset::set(|runtime| &runtime.flushing, true);
    RUNTIME.with(|runtime| runtime.woken.set(false));
    let flush = next_flush();
    loop {
        let next = RUNTIME.with(|runtime| {
            runtime.removals.set(runtime.removals.get() + 1);
            let next = runtime.queue.borrow_mut().pop_front();
            match next {
                Some(next) => Some((next, true)),
                None => runtime
                    .late
                    .borrow_mut()
                    .pop_front()
                    .map(|late| (late, false)),
            }
        });
        let Some((next, may_defer)) = next else { break };
        run_queued(next, flush, may_defer);
    }
}

pub(crate) fn flush_within(root: &Owner) {
    nested(|| run_within(root));
    let requeued = RUNTIME.with(|runtime| {
        if runtime.flushing.get() || runtime.late.borrow().is_empty() {
            return false;
        }
        let late = std::mem::take(&mut *runtime.late.borrow_mut());
        runtime.queue.borrow_mut().extend(late);
        true
    });
    if requeued {
        wake_if_pending();
    }
}

fn run_within(root: &Owner) {
    let flush = next_flush();
    let mut checked = 0;
    let mut seen = RUNTIME.with(|runtime| runtime.removals.get());
    let owned = |queued: &Weak<Computation>| {
        queued
            .upgrade()
            .is_some_and(|computation| computation.owned_within(root))
    };
    loop {
        let next = RUNTIME.with(|runtime| {
            if runtime.removals.get() != seen {
                checked = 0;
            }
            let mut queue = runtime.queue.borrow_mut();
            let found = queue.iter().skip(checked).position(owned);
            runtime.removals.set(runtime.removals.get() + 1);
            seen = runtime.removals.get();
            if let Some(at) = found {
                checked += at;
                return queue.remove(checked).map(|next| (next, true));
            }
            checked = queue.len();
            let mut late = runtime.late.borrow_mut();
            let at = late.iter().position(owned)?;
            late.remove(at).map(|late| (late, false))
        });
        let Some((next, may_defer)) = next else { break };
        run_queued(next, flush, may_defer);
    }
}

fn next_flush() -> u64 {
    RUNTIME.with(|runtime| {
        let flush = runtime.flushes.get() + 1;
        runtime.flushes.set(flush);
        flush
    })
}

fn run_queued(next: Weak<Computation>, flush: u64, may_defer: bool) {
    let Some(computation) = next.upgrade() else {
        return;
    };
    if let Some(gate) = computation.closed_gate() {
        gate.park(next);
        return;
    }
    let current = RUNTIME.with(|runtime| runtime.zone.get());
    if computation.zone != current {
        let wake = RUNTIME.with(|runtime| {
            let mut parked = runtime.parked.borrow_mut();
            if let Some((_, waiting)) = parked
                .iter_mut()
                .find(|(zone, _)| *zone == computation.zone)
            {
                waiting.push(next);
                return None;
            }
            parked.push((computation.zone, vec![next]));
            let wakes = runtime.wakes.borrow();
            let wake = wakes.iter().find(|(zone, _)| *zone == computation.zone);
            wake.map(|(_, wake)| Rc::clone(wake))
        });
        if let Some(wake) = wake {
            wake();
        }
        return;
    }
    if may_defer && computation.reads_freed() {
        RUNTIME.with(|runtime| runtime.late.borrow_mut().push_back(next));
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
        if !std::thread::panicking() {
            run_parked(self.previous);
        }
    }
}

pub fn enter_zone(zone: u64) -> ZoneGuard {
    let previous = RUNTIME.with(|runtime| runtime.zone.replace(zone));
    run_parked(zone);
    ZoneGuard { previous }
}

fn run_parked(zone: u64) {
    let parked = RUNTIME.with(|runtime| runtime.take_parked(zone));
    if let Some(parked) = parked {
        RUNTIME.with(|runtime| runtime.queue.borrow_mut().extend(parked));
        settle();
    }
}

pub fn zone_pending(zone: u64) -> bool {
    RUNTIME.with(|runtime| {
        runtime.parked.borrow().iter().any(|(held, parked)| {
            *held == zone
                && parked
                    .iter()
                    .any(|computation| computation.strong_count() > 0)
        })
    })
}

pub fn wake_zone_with(zone: u64, wake: impl Fn() + 'static) {
    RUNTIME.with(|runtime| {
        let mut wakes = runtime.wakes.borrow_mut();
        wakes.retain(|(held, _)| *held != zone);
        wakes.push((zone, Rc::new(wake)));
    });
}

pub fn forget_zone(zone: u64) {
    RUNTIME.with(|runtime| {
        runtime.take_parked(zone);
        runtime.wakes.borrow_mut().retain(|(held, _)| *held != zone);
    });
}

pub(crate) fn current_zone() -> u64 {
    RUNTIME.with(|runtime| runtime.zone.get())
}
