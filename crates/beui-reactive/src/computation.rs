use std::cell::{Cell, RefCell};
use std::mem::ManuallyDrop;
use std::rc::{Rc, Weak};

use crate::arena::{self, Key};
use crate::memo::Compute;
use crate::runtime::{
    Context, RUNTIME, Read, Reset, current_zone, defer, enqueue, leave_component_body, probe,
    untrack,
};
use crate::scope::{Cleanup, Gate, Owner, current_owner};
use crate::signal::{Get, Set, Signal, Tracked, create_signal, readiness_of};

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
    kept: Cell<bool>,
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

struct Parts {
    owner: Rc<Owner>,
    tracking: u64,
    zone: u64,
    gate: Option<Rc<Gate>>,
}

fn parts() -> Parts {
    let owner = current_owner();
    let gate = owner.gate.borrow().clone();
    Parts {
        owner,
        tracking: next_tracking(),
        zone: current_zone(),
        gate,
    }
}

impl Computation {
    pub(crate) fn new(
        source: Option<Rc<dyn Tracked>>,
        callback: impl FnMut() -> bool + 'static,
    ) -> Rc<Self> {
        let parts = parts();
        let computation: Rc<Self> = Rc::new(Computation {
            parent: parts.owner.computation.clone(),
            owner: Rc::downgrade(&parts.owner),
            state: Cell::new(State::Dirty),
            refreshing: Cell::new(false),
            execution_owner: RefCell::new(None),
            dependencies: RefCell::new(Vec::new()),
            tracking: Cell::new(parts.tracking),
            source,
            queued: Cell::new(false),
            queued_at: Cell::new(0),
            runs: Cell::new((0, 0)),
            binding: Cell::new(false),
            kept: Cell::new(false),
            zone: parts.zone,
            gate: parts.gate,
            callback: RefCell::new(Slot {
                live: true,
                callback: ManuallyDrop::new(callback),
            }),
        });
        computation.adopt(&parts.owner);
        computation
    }

    fn adopt(self: &Rc<Self>, owner: &Owner) {
        owner.add(Cleanup::Dispose(self.clone()));
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
            self.settle_check();
            if self.state.get() == State::Clean {
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
                if let Some(source) = &self.source {
                    let readiness = readiness_of(
                        self.dependencies
                            .borrow()
                            .iter()
                            .map(|(dependency, _, _)| dependency.source()),
                    );
                    let source = source.source();
                    if source.set_readiness(readiness) || changed {
                        source.version.set(source.version.get().wrapping_add(1));
                    }
                }
            }
            Err(error) => std::panic::resume_unwind(error),
        }
        if self.is_spent() {
            self.dispose();
        }
    }

    /// Settles a `Check` without running: brings the memos it read up to date, and is `Dirty`
    /// if one of them changed, else `Clean`.
    fn settle_check(&self) {
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
                return;
            }
        }
        self.state.set(State::Clean);
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
        !self.kept.get()
            && self.source.is_none()
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

    pub(crate) fn is_dirty(&self) -> bool {
        self.state.get() == State::Dirty
    }

    pub(crate) fn kept_binding(callback: impl FnMut() -> bool + 'static) -> Rc<Self> {
        let computation = Self::new(None, callback);
        computation.binding.set(true);
        computation.kept.set(true);
        computation
    }
}

/// A handle to an effect or binding: a `Copy` key, like every reactive handle. The scope
/// that made the effect owns it; the handle only stops it early.
pub struct Effect {
    key: Option<Key>,
}

impl Clone for Effect {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for Effect {}

impl PartialEq for Effect {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for Effect {}

impl std::fmt::Debug for Effect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Effect").field(&self.key).finish()
    }
}

impl Effect {
    fn of(computation: &Rc<Computation>) -> Self {
        Self {
            key: Some(arena::insert_owned_computation(Rc::downgrade(computation))),
        }
    }

    /// A handle to nothing: a binding that read nothing made no computation.
    fn none() -> Self {
        Self { key: None }
    }

    fn computation(&self) -> Option<Rc<Computation>> {
        arena::get_computation(self.key?)
    }

    /// Stops it now, as disposing its scope would. Does nothing once it has stopped.
    pub fn dispose(&self) {
        if let Some(computation) = self.computation() {
            defer(|| computation.dispose());
        }
    }

    pub fn is_disposed(&self) -> bool {
        self.computation()
            .is_none_or(|computation| computation.is_disposed())
    }

    /// Whether something it read changed since its last run, so it will run again. One that
    /// may have (a memo it read was woken) brings those memos up to date to tell, without
    /// running.
    pub(crate) fn is_due(&self) -> bool {
        let Some(computation) = self.computation() else {
            return false;
        };
        if computation.state.get() == State::Check {
            computation.settle_check();
        }
        computation.state.get() == State::Dirty
    }
}

/// Queues a first run, then reruns after any tracked input changes, with writes coalesced
/// into one run. It runs only while its zone is entered (see `enter_zone`) and its scope is
/// not paused. Effects it creates run after it finishes.
///
/// One that finishes its first run having read nothing, opened no scope and registered no
/// cleanup is disposed then and there. More than 10,000 runs in one flush panics and
/// disposes it, to stop a feedback loop. Use it for work that needs the tree built around
/// it (moving focus) or that acts outside the graph; bind an existing node's property with
/// `create_binding`.
pub fn create_effect(mut effect: impl FnMut() + 'static) -> Effect {
    let computation = Computation::new(None, move || {
        effect();
        false
    });
    enqueue(&computation);
    Effect::of(&computation)
}

/// An effect whose first run happens now, as a memo's does, instead of being queued. A
/// first run that reads nothing costs only the closure: no computation is made, and the
/// handle it returns is already disposed.
///
/// Each binding still costs a probe, so props applied together share one binding. Unlike
/// an effect, a binding runs while its scope's effects are paused by
/// `Scope::pausable_effects`, so a hidden `Loading` content still builds its tree.
pub fn create_binding(mut binding: impl FnMut() + 'static) -> Effect {
    let probed = probe(false, &mut binding);
    if probed.constant() {
        if let Some(owner) = probed.owner {
            owner.dispose();
        }
        return Effect::none();
    }
    let computation = Computation::new(None, move || {
        binding();
        false
    });
    computation.binding.set(true);
    computation.watch(probed.read, probed.owner);
    Effect::of(&computation)
}

/// An effect over explicit dependencies, like SolidJS's `on`: `deps` is read tracked, and
/// `run` gets its value and the one before it (`None` on the first run) with nothing
/// tracked, so what `run` reads does not wake it. Queued like `create_effect`.
///
/// It runs when a dependency was written, not when `deps` returns something new: deps
/// that change and change back before it runs (while its scope is paused, say) run it
/// with `value == previous`. Compare them when that matters.
///
/// `deps` returns the values `run` needs, or `()` after `track`ing what should wake it:
/// `create_effect_on(move || (pointer.get(), caret.get()), move |_, _| publish())`.
pub fn create_effect_on<D: 'static>(
    mut deps: impl FnMut() -> D + 'static,
    mut run: impl FnMut(&D, Option<&D>) + 'static,
) -> Effect {
    let mut previous = None;
    create_effect(move || {
        let value = deps();
        untrack(|| run(&value, previous.as_ref()));
        previous = Some(value);
    })
}

/// `create_effect_on` without its first run (SolidJS's `defer`): `deps` is read now, at
/// creation, and `run` runs only after what it read changes, with the value before; a
/// change made before the first flush still runs it. For acting on a
/// change rather than on the state: begin a move each time the count of asked moves goes
/// up.
pub fn create_effect_on_change<D: 'static>(
    mut deps: impl FnMut() -> D + 'static,
    mut run: impl FnMut(&D, &D) + 'static,
) -> Effect {
    let probed = probe(false, &mut deps);
    if probed.constant() {
        if let Some(owner) = probed.owner {
            owner.dispose();
        }
        return Effect::none();
    }
    let mut previous = probed.value;
    let computation = Computation::new(None, move || {
        let value = deps();
        untrack(|| run(&value, &previous));
        previous = value;
        false
    });
    computation.watch(probed.read, probed.owner);
    Effect::of(&computation)
}

/// A signal that holds what was last written to it, but resets to what `source` returns
/// whenever that changes, like Angular's `linkedSignal`: a draft seeded from a saved value
/// and reset when the saved value changes. `source` is compared with what it returned
/// before, so a change to what it reads that leaves its result the same keeps a draft.
///
/// `source` runs now. The reset is not synchronous: it is a binding, so after `source`'s
/// inputs change the signal still holds the old value until effects next run (the end of
/// the entry, or `flush()`).
///
/// Like `create_memo`'s, the closure may take an annotated `previous: Option<&T>`: what the
/// signal holds when the reset comes (`None` the first time), so a reset can keep part of
/// what was written, such as text typed that still means the new value.
pub fn create_linked_signal<T: Clone + PartialEq + 'static, M>(
    mut source: impl Compute<T, M> + 'static,
) -> Signal<T> {
    let probed = probe(false, || source.compute(None));
    let constant = probed.constant();
    let mut returned = probed.value.clone();
    let signal = create_signal(probed.value);
    if constant {
        if let Some(owner) = probed.owner {
            owner.dispose();
        }
        return signal;
    }
    let computation = Computation::new(None, move || {
        let value = signal.with_untracked(|held| source.compute(Some(held)));
        if value != returned {
            returned = value.clone();
            signal.set(value);
        }
        false
    });
    computation.binding.set(true);
    computation.watch(probed.read, probed.owner);
    signal
}
