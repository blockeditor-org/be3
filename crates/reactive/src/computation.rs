use std::cell::{Cell, RefCell};
use std::mem::ManuallyDrop;
use std::rc::{Rc, Weak};

use crate::runtime::{
    Context, RUNTIME, Read, Reset, current_zone, defer, enqueue, leave_component_body, probe,
};
use crate::scope::{Cleanup, Gate, Owner, current_owner};
use crate::signal::Tracked;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    Clean,
    Check,
    Dirty,
    Disposed,
}

struct Slot<F: ?Sized> {
    live: bool,
    callback: ManuallyDrop<F>,
}

impl<F: ?Sized> Slot<F> {
    fn release(&mut self) {
        if std::mem::replace(&mut self.live, false) {
            // unsafe(memory)
            unsafe { ManuallyDrop::drop(&mut self.callback) }
        }
    }
}

impl<F: ?Sized> Drop for Slot<F> {
    fn drop(&mut self) {
        self.release();
    }
}

pub(crate) struct Computation<F: ?Sized = dyn FnMut() -> bool> {
    parent: Weak<Computation>,
    pub(crate) owner: Weak<Owner>,
    state: Cell<State>,
    refreshing: Cell<bool>,
    execution_owner: RefCell<Option<Rc<Owner>>>,
    pub(crate) dependencies: RefCell<Vec<Dependency>>,
    pub(crate) tracking: Cell<u64>,
    pub(crate) source: Option<Rc<dyn Tracked>>,
    pub(crate) queued: Cell<bool>,
    pub(crate) queued_at: Cell<u64>,
    runs: Cell<(u64, u32)>,
    binding: Cell<bool>,
    pub(crate) zone: u64,
    gate: Option<Rc<Gate>>,
    callback: RefCell<Slot<F>>,
}

pub(crate) type Dependency = (Rc<dyn Tracked>, u64, usize);

thread_local! {
    static TRACKINGS: Cell<u64> = const { Cell::new(0) };
}

fn next_tracking() -> u64 {
    TRACKINGS.with(|trackings| {
        let next = trackings.get() + 1;
        trackings.set(next);
        next
    })
}

pub(crate) fn no_computation() -> Weak<Computation> {
    Weak::<Computation<fn() -> bool>>::new()
}

impl Computation {
    pub(crate) fn new(
        source: Option<Rc<dyn Tracked>>,
        callback: impl FnMut() -> bool + 'static,
    ) -> Rc<Self> {
        let owner = current_owner();
        let computation: Rc<Self> = Rc::new(Computation {
            parent: owner.computation.clone(),
            owner: Rc::downgrade(&owner),
            state: Cell::new(State::Dirty),
            refreshing: Cell::new(false),
            execution_owner: RefCell::new(None),
            dependencies: RefCell::new(Vec::new()),
            tracking: Cell::new(next_tracking()),
            source,
            queued: Cell::new(false),
            queued_at: Cell::new(0),
            runs: Cell::new((0, 0)),
            binding: Cell::new(false),
            zone: current_zone(),
            gate: owner.gate.borrow().clone(),
            callback: RefCell::new(Slot {
                live: true,
                callback: ManuallyDrop::new(callback),
            }),
        });
        owner.add(Cleanup::Dispose(computation.clone()));
        computation
    }

    pub(crate) fn count_run(&self, flush: u64) -> u32 {
        let (counted, runs) = self.runs.get();
        let runs = if counted == flush { runs + 1 } else { 1 };
        self.runs.set((flush, runs));
        runs
    }

    pub(crate) fn watch(self: &Rc<Self>, read: Vec<Read>, owner: Option<Rc<Owner>>) {
        self.execution_owner.replace(owner);
        let stale = read
            .iter()
            .any(|(source, version)| source.source().version.get() != *version);
        let tracking = next_tracking();
        self.tracking.set(tracking);
        let dependencies = read
            .into_iter()
            .map(|(source, version)| {
                source.source().tracked_by.set(tracking);
                let slot = source
                    .source()
                    .subscribers
                    .borrow_mut()
                    .add(Rc::downgrade(self));
                (source, version, slot)
            })
            .collect();
        *self.dependencies.borrow_mut() = dependencies;
        self.state.set(State::Clean);
        if stale {
            self.invalidate(State::Dirty);
        }
    }

    pub(crate) fn invalidate(self: &Rc<Self>, state: State) {
        let previous = self.state.get();
        if previous == State::Disposed {
            return;
        }
        if self.source.is_none() {
            enqueue(self);
        }
        if previous == State::Dirty || previous == state {
            return;
        }
        self.state.set(state);
        if let Some(source) = &self.source {
            let subscribers = source.source().subscribers.borrow().live();
            for subscriber in subscribers {
                subscriber.invalidate(State::Check);
            }
        }
    }

    pub(crate) fn refresh(self: &Rc<Self>) {
        if self.source.is_none()
            && let Some(parent) = self.parent.upgrade()
            && !parent.refreshing.get()
        {
            parent.refresh();
        }
        assert!(!self.refreshing.get(), "reactive dependency cycle detected");
        if matches!(self.state.get(), State::Clean | State::Disposed) {
            return;
        }
        defer(|| {
            self.refreshing.set(true);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.refresh_inner();
            }));
            self.refreshing.set(false);
            if let Err(error) = result {
                if self.state.get() != State::Disposed {
                    self.state.set(State::Dirty);
                    if self.source.is_none() {
                        enqueue(self);
                    }
                }
                std::panic::resume_unwind(error);
            }
        });
    }

    fn refresh_inner(self: &Rc<Self>) {
        if self.state.get() == State::Check {
            let dependencies = self.dependencies.borrow().clone();
            for (source, version, _) in dependencies {
                let source = source.source();
                if source.freed.get() {
                    continue;
                }
                let producer = source.producer.borrow().upgrade();
                if let Some(producer) = producer {
                    producer.refresh();
                }
                if source.version.get() != version {
                    self.state.set(State::Dirty);
                    break;
                }
            }
            if self.state.get() == State::Check {
                self.state.set(State::Clean);
                return;
            }
        }
        self.detach();
        let owner = self.reusable_owner();
        if self.state.get() == State::Disposed {
            return;
        }
        let _context = match &owner {
            Some(owner) => Context::enter(Rc::downgrade(self), Rc::downgrade(owner)),
            None => Context::enter_pending(Rc::downgrade(self), Weak::new(), Rc::downgrade(self)),
        };
        let zone = match self.zone {
            0 => current_zone(),
            zone => zone,
        };
        let _zone = Reset::set(|runtime| &runtime.zone, zone);
        let _body = leave_component_body();
        let memo_depth = RUNTIME.with(|runtime| runtime.memo_depth.get());
        let _memo_guard = Reset::set(
            |runtime| &runtime.memo_depth,
            match self.source {
                Some(_) => memo_depth + 1,
                None => 0,
            },
        );
        self.state.set(State::Clean);
        let result = {
            let mut slot = self
                .callback
                .try_borrow_mut()
                .expect("reactive callback is already running");
            if !slot.live {
                return;
            }
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (slot.callback)()));
            if self.state.get() == State::Disposed {
                slot.release();
            }
            result
        };
        match result {
            Ok(changed) => {
                if changed && let Some(source) = &self.source {
                    let source = source.source();
                    source.version.set(source.version.get().wrapping_add(1));
                }
            }
            Err(error) => std::panic::resume_unwind(error),
        }
        if self.is_spent() {
            self.dispose();
        }
    }

    pub(crate) fn reads_freed(&self) -> bool {
        self.state.get() != State::Clean
            && self
                .dependencies
                .borrow()
                .iter()
                .any(|(source, _, _)| source.source().freed.get())
    }

    pub(crate) fn owned_within(&self, root: &Owner) -> bool {
        self.owner.upgrade().is_some_and(|owner| owner.within(root))
    }

    pub(crate) fn rerun_whatever_it_reads(&self) {
        if self.state.get() == State::Check {
            self.state.set(State::Dirty);
        }
    }

    pub(crate) fn closed_gate(&self) -> Option<Rc<Gate>> {
        if self.source.is_some() {
            return None;
        }
        Gate::closed_since(
            self.gate.as_ref()?,
            self.queued_at.get(),
            self.binding.get(),
        )
    }

    fn is_spent(&self) -> bool {
        self.source.is_none()
            && self.state.get() == State::Clean
            && self.dependencies.borrow().is_empty()
            && self
                .execution_owner
                .borrow()
                .as_ref()
                .is_none_or(|owner| owner.is_inert())
    }

    fn reusable_owner(&self) -> Option<Rc<Owner>> {
        let held = self.execution_owner.borrow().clone()?;
        if held.is_inert() && !held.disposed.get() {
            return Some(held);
        }
        self.execution_owner.replace(None);
        held.dispose();
        None
    }

    pub(crate) fn ensure_owner(self: &Rc<Self>) -> Rc<Owner> {
        if let Some(owner) = self.execution_owner.borrow().clone() {
            return owner;
        }
        let owner = Rc::new(Owner::for_computation(
            Rc::downgrade(self),
            self.owner.clone(),
            self.gate.clone(),
        ));
        self.execution_owner.replace(Some(owner.clone()));
        owner
    }

    fn detach(&self) {
        self.tracking.set(next_tracking());
        for (source, _, slot) in self.dependencies.take() {
            source.source().subscribers.borrow_mut().remove(slot, self);
        }
    }

    pub(crate) fn dispose(&self) {
        if self.state.replace(State::Disposed) == State::Disposed {
            return;
        }
        self.detach();
        if let Ok(mut slot) = self.callback.try_borrow_mut() {
            slot.release();
        }
        let owner = self.execution_owner.borrow().clone();
        if let Some(owner) = owner {
            owner.dispose();
        }
    }

    pub(crate) fn is_disposed(&self) -> bool {
        self.state.get() == State::Disposed
    }
}

#[derive(Clone)]
pub struct Effect {
    computation: Rc<Computation>,
}

pub fn create_effect(mut effect: impl FnMut() + 'static) -> Effect {
    let computation = Computation::new(None, move || {
        effect();
        false
    });
    enqueue(&computation);
    Effect { computation }
}

pub fn create_binding(mut binding: impl FnMut() + 'static) {
    let probed = probe(false, &mut binding);
    if probed.constant() {
        if let Some(owner) = probed.owner {
            owner.dispose();
        }
        return;
    }
    let computation = Computation::new(None, move || {
        binding();
        false
    });
    computation.binding.set(true);
    computation.watch(probed.read, probed.owner);
}

impl Effect {
    pub fn dispose(&self) {
        defer(|| self.computation.dispose());
    }

    pub fn is_disposed(&self) -> bool {
        self.computation.is_disposed()
    }
}
