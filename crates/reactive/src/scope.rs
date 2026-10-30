use stable_hash::HashMap;
use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::arena::{self, Key};
use crate::computation::{Computation, no_computation};
use crate::runtime::{
    Context, RUNTIME, batch, defer, flush_within, leave_component_body, next_sequence,
};

pub(crate) struct Gate {
    parent: Option<Rc<Gate>>,
    effects_only: bool,
    paused_at: Cell<Option<u64>>,
    parked: RefCell<Vec<Weak<Computation>>>,
}

impl Gate {
    pub(crate) fn closed_since(gate: &Rc<Gate>, queued_at: u64, binding: bool) -> Option<Rc<Gate>> {
        let mut gate = Some(gate);
        while let Some(current) = gate {
            if !(binding && current.effects_only)
                && current
                    .paused_at
                    .get()
                    .is_some_and(|paused| queued_at > paused)
            {
                return Some(current.clone());
            }
            gate = current.parent.as_ref();
        }
        None
    }

    pub(crate) fn park(&self, computation: Weak<Computation>) {
        self.parked.borrow_mut().push(computation);
    }
}

pub(crate) struct Owner {
    pub(crate) computation: Weak<Computation>,
    pub(crate) disposed: Cell<bool>,
    parent: Option<Rc<Owner>>,
    pub(crate) gate: RefCell<Option<Rc<Gate>>>,
    contexts: RefCell<HashMap<TypeId, Rc<dyn Any>>>,
    cleanups: RefCell<Vec<Cleanup>>,
}

impl Default for Owner {
    fn default() -> Self {
        Self {
            computation: no_computation(),
            disposed: Cell::default(),
            parent: None,
            gate: RefCell::default(),
            contexts: RefCell::default(),
            cleanups: RefCell::default(),
        }
    }
}

pub trait Release {
    fn release(&self, key: u64);
}

pub(crate) enum Cleanup {
    Run(Box<dyn FnOnce()>),
    Release(Rc<dyn Release>, u64),
    Free(Key),
    Dispose(Rc<Computation>),
    Close(Rc<Owner>),
}

impl Cleanup {
    fn run(self) {
        match self {
            Cleanup::Run(cleanup) => cleanup(),
            Cleanup::Release(target, key) => target.release(key),
            Cleanup::Free(key) => arena::remove(key),
            Cleanup::Dispose(computation) => computation.dispose(),
            Cleanup::Close(owner) => owner.dispose(),
        }
    }
}

impl Owner {
    pub(crate) fn for_computation(
        computation: Weak<Computation>,
        parent: Weak<Owner>,
        gate: Option<Rc<Gate>>,
    ) -> Self {
        Self {
            computation,
            parent: parent.upgrade(),
            gate: RefCell::new(gate),
            ..Self::default()
        }
    }

    pub(crate) fn within(self: &Rc<Self>, root: &Owner) -> bool {
        let mut current = Some(Rc::clone(self));
        while let Some(owner) = current {
            if std::ptr::eq(Rc::as_ptr(&owner), root) {
                return true;
            }
            current = owner.parent.clone();
        }
        false
    }

    pub(crate) fn add(&self, cleanup: Cleanup) {
        assert!(!self.disposed.get(), "reactive scope was disposed");
        self.cleanups.borrow_mut().push(cleanup);
    }

    pub(crate) fn is_inert(&self) -> bool {
        self.cleanups.borrow().is_empty() && self.contexts.borrow().is_empty()
    }

    pub(crate) fn dispose(&self) {
        if self.disposed.replace(true) {
            return;
        }
        let cleanups = self.cleanups.take();
        let _context = Context::enter(no_computation(), Weak::new());
        let _body = leave_component_body();
        let mut failure = None;
        for cleanup in cleanups.into_iter().rev() {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cleanup.run()));
            if let Err(error) = result {
                failure.get_or_insert(error);
            }
        }
        if let Some(error) = failure
            && !std::thread::panicking()
        {
            std::panic::resume_unwind(error);
        }
    }
}

pub struct Scope {
    owner: Rc<Owner>,
    gate: Option<Rc<Gate>>,
}

fn open_owner(parent: Option<Rc<Owner>>) -> Rc<Owner> {
    Rc::new(Owner {
        computation: parent
            .as_ref()
            .map(|parent| parent.computation.clone())
            .unwrap_or_else(no_computation),
        gate: RefCell::new(
            parent
                .as_ref()
                .and_then(|parent| parent.gate.borrow().clone()),
        ),
        parent,
        ..Owner::default()
    })
}

impl Scope {
    pub fn new() -> Self {
        let scope = Self::detached();
        let parent = live_owner();
        if let Some(parent) = parent {
            let child = scope.owner.clone();
            parent.add(Cleanup::Close(child));
        }
        scope
    }

    pub fn detached() -> Self {
        Self {
            owner: open_owner(live_owner()),
            gate: None,
        }
    }

    pub fn root() -> Self {
        Self {
            owner: open_owner(None),
            gate: None,
        }
    }

    pub fn pausable(self) -> Self {
        self.gated(false)
    }

    pub fn pausable_effects(self) -> Self {
        self.gated(true)
    }

    fn gated(mut self, effects_only: bool) -> Self {
        assert!(
            self.owner.is_inert(),
            "a scope must be made pausable before anything is created in it"
        );
        let gate = Rc::new(Gate {
            parent: self.owner.gate.borrow().clone(),
            effects_only,
            paused_at: Cell::new(None),
            parked: RefCell::new(Vec::new()),
        });
        self.owner.gate.replace(Some(gate.clone()));
        self.gate = Some(gate);
        self
    }

    pub fn pause(&self) {
        let gate = self
            .gate
            .as_ref()
            .expect("only a pausable scope can be paused");
        if gate.paused_at.get().is_none() {
            gate.paused_at.set(Some(next_sequence()));
        }
    }

    pub fn resume(&self) {
        let gate = self
            .gate
            .as_ref()
            .expect("only a pausable scope can be resumed");
        if gate.paused_at.take().is_none() {
            return;
        }
        let parked = gate.parked.take();
        RUNTIME.with(|runtime| {
            let mut queue = runtime.queue.borrow_mut();
            for computation in parked {
                if let Some(live) = computation.upgrade() {
                    live.rerun_whatever_it_reads();
                    live.queued_at.set(next_sequence());
                    queue.push_back(computation);
                }
            }
        });
        crate::runtime::wake_if_pending();
    }

    pub fn run<T>(&self, f: impl FnOnce() -> T) -> T {
        assert!(!self.is_disposed(), "reactive scope was disposed");
        let _context = Context::enter(no_computation(), Rc::downgrade(&self.owner));
        batch(f)
    }

    pub fn dispose(&self) {
        defer(|| self.owner.dispose());
    }

    pub fn is_disposed(&self) -> bool {
        self.owner.disposed.get()
    }

    pub fn context(&self) -> ScopeContext {
        ScopeContext(Rc::downgrade(&self.owner))
    }

    pub fn flush_owned(&self) {
        flush_within(&self.owner);
    }
}

pub struct RootScope {
    _context: Context,
    _scope: Scope,
}

impl RootScope {
    pub fn new() -> Self {
        let scope = Scope::detached();
        Self {
            _context: Context::enter(no_computation(), Rc::downgrade(&scope.owner)),
            _scope: scope,
        }
    }

    pub fn into_scope(self) -> Scope {
        let Self { _context, _scope } = self;
        drop(_context);
        _scope
    }
}

impl Default for RootScope {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct ScopeContext(Weak<Owner>);

impl ScopeContext {
    pub fn run<T>(&self, f: impl FnOnce() -> T) -> T {
        if let Some(owner) = self.0.upgrade() {
            assert!(!owner.disposed.get(), "reactive scope was disposed");
        }
        let _context = Context::enter(no_computation(), self.0.clone());
        batch(f)
    }

    pub fn is_alive(&self) -> bool {
        self.0.upgrade().is_some_and(|owner| !owner.disposed.get())
    }

    pub fn flush_owned(&self) {
        if let Some(owner) = self.0.upgrade() {
            flush_within(&owner);
        }
    }

    pub fn child(&self) -> Option<Scope> {
        self.is_alive().then(|| self.run(Scope::detached))
    }
}

pub fn child_scope<T>(f: impl FnOnce() -> T) -> (T, ScopeContext) {
    let parent = current_owner();
    let owner = open_owner(Some(parent.clone()));
    parent.add(Cleanup::Close(owner.clone()));
    let context = ScopeContext(Rc::downgrade(&owner));
    (context.run(f), context)
}

pub fn current_scope() -> Option<ScopeContext> {
    let context = ScopeContext(Rc::downgrade(&live_owner()?));
    context.is_alive().then_some(context)
}

pub fn owner_scope() -> Option<ScopeContext> {
    RUNTIME.with(|runtime| {
        let observer = runtime.observer.borrow().upgrade();
        match observer {
            Some(computation) => Some(ScopeContext(computation.owner.clone())),
            None => {
                let owner = runtime.owner.borrow().clone();
                owner.upgrade().map(|_| ScopeContext(owner))
            }
        }
    })
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.owner.dispose();
        } else {
            self.dispose();
        }
    }
}

pub fn on_cleanup(cleanup: impl FnOnce() + 'static) {
    current_owner().add(Cleanup::Run(Box::new(cleanup)));
}

pub fn on_release(target: Rc<dyn Release>, key: u64) {
    current_owner().add(Cleanup::Release(target, key));
}

pub fn provide_context<T: 'static>(value: T) {
    current_owner()
        .contexts
        .borrow_mut()
        .insert(TypeId::of::<T>(), Rc::new(value));
}

pub fn use_context<T: Clone + 'static>() -> Option<T> {
    let mut owner = RUNTIME.with(|runtime| {
        runtime
            .owner
            .borrow()
            .upgrade()
            .or_else(|| {
                let pending = runtime.pending.borrow().upgrade()?;
                pending.owner.upgrade()
            })
            .or_else(|| runtime.probe.borrow().as_ref()?.parent.upgrade())
    });
    while let Some(current) = owner {
        let found = current
            .contexts
            .borrow()
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<T>().cloned());
        if found.is_some() {
            return found;
        }
        owner = current.parent.clone();
    }
    None
}

pub(crate) fn live_owner() -> Option<Rc<Owner>> {
    let owner = RUNTIME.with(|runtime| runtime.owner.borrow().upgrade());
    if owner.is_some() {
        return owner;
    }
    if let Some(owner) = probe_owner() {
        RUNTIME.with(|runtime| runtime.owner.replace(Rc::downgrade(&owner)));
        return Some(owner);
    }
    let pending = RUNTIME.with(|runtime| runtime.pending.borrow().upgrade())?;
    let owner = pending.ensure_owner();
    RUNTIME.with(|runtime| runtime.owner.replace(Rc::downgrade(&owner)));
    Some(owner)
}

fn probe_owner() -> Option<Rc<Owner>> {
    RUNTIME.with(|runtime| {
        let mut probe = runtime.probe.borrow_mut();
        let probe = probe.as_mut()?;
        if probe.owner.is_none() {
            let parent = probe.parent.upgrade()?;
            let gate = parent.gate.borrow().clone();
            probe.owner = Some(Rc::new(Owner::for_computation(
                no_computation(),
                Rc::downgrade(&parent),
                gate,
            )));
        }
        probe.owner.clone()
    })
}

pub(crate) fn current_owner() -> Rc<Owner> {
    let owner = live_owner().expect("create signals, memos, effects, and cleanups inside a scope");
    assert!(!owner.disposed.get(), "reactive scope was disposed");
    owner
}
