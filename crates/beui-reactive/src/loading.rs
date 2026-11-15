use be_lib_stable_hash::HashMap;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::runtime::in_memo;
use crate::signal::{Tracked, track};
use crate::{
    Get, Memo, Stored, Trigger, create_memo, create_trigger, on_cleanup, provide_context, store,
    use_context,
};

/// Why a `Ready` value is not there yet. Data that may not have arrived is a `Ready<T>`, so
/// a memo derives from it with `?` and no `Option` or `loaded` flag sits beside it
/// (decisions/loading_state_is_ready_and_waits_at_a_boundary.md).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pending {
    Loading,
    Failed(String),
}

pub type Ready<T> = Result<T, Pending>;

type Probe = Rc<dyn Fn() -> Ready<()>>;

/// The reads a boundary waits on, one entry per node however often it is read, each kept as
/// long as a scope that read it lives.
#[derive(Default)]
struct Waits {
    probes: RefCell<Vec<(u64, Probe)>>,
    by_node: RefCell<HashMap<usize, (u64, usize)>>,
    next: Cell<u64>,
    revealed: Cell<bool>,
}

impl Waits {
    fn probes(&self) -> Vec<Probe> {
        self.probes
            .borrow()
            .iter()
            .map(|(_, probe)| Rc::clone(probe))
            .collect()
    }

    fn add(&self, probe: Probe) -> u64 {
        let id = self.next.get();
        self.next.set(id + 1);
        self.probes.borrow_mut().push((id, probe));
        id
    }

    fn remove(&self, id: u64) {
        self.probes.borrow_mut().retain(|(held, _)| *held != id);
    }
}

/// A probe reading `node`: whether its value is there, subscribing to it.
fn probe_of(node: Rc<dyn Tracked>) -> Probe {
    Rc::new(move || {
        track(Rc::clone(&node));
        let held = node.source();
        let producer = held.producer.borrow().upgrade();
        if let Some(producer) = producer {
            producer.refresh();
        }
        match held.freed.get() {
            true => Ok(()),
            false => held.readiness().map_or(Ok(()), Err),
        }
    })
}

/// Makes `waits` hear of `node` for as long as the current scope lives, notifying `changed`
/// when that adds or drops a node.
fn wait_on(waits: Stored<Waits>, changed: Trigger, node: Rc<dyn Tracked>) {
    let key = Rc::as_ptr(&node) as *const () as usize;
    let added = waits.with(|waits| {
        let mut by_node = waits.by_node.borrow_mut();
        match by_node.get_mut(&key) {
            Some((_, count)) => {
                *count += 1;
                false
            }
            None => {
                let id = waits.add(probe_of(node));
                by_node.insert(key, (id, 1));
                true
            }
        }
    });
    if added {
        changed.notify();
    }
    on_cleanup(move || {
        waits.try_with(|waits| {
            let mut by_node = waits.by_node.borrow_mut();
            if let Some((id, count)) = by_node.get_mut(&key) {
                *count -= 1;
                if *count == 0 {
                    let id = *id;
                    by_node.remove(&key);
                    waits.remove(id);
                    changed.notify();
                }
            }
        });
    });
}

/// What a `Loading` component provides: it collects the pending values read under it and
/// reveals its content once none is pending, after which nothing registers with it again.
/// An inner boundary keeps its content's waits from the outer one.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Boundary {
    changed: Trigger,
    waits: Stored<Waits>,
    state: Memo<Ready<()>>,
}

impl Boundary {
    pub fn provide() -> Self {
        let changed = create_trigger();
        let waits = store(Waits::default());
        let state = create_memo(move || {
            changed.track();
            settle(waits.with(Waits::probes).iter().map(|probe| probe()))
        });
        let boundary = Self {
            changed,
            waits,
            state,
        };
        provide_context(boundary);
        boundary
    }

    /// The nearest boundary that has not revealed its content.
    pub fn current() -> Option<Self> {
        Self::nearest().filter(Self::waiting)
    }

    fn nearest() -> Option<Self> {
        if in_memo() {
            return None;
        }
        use_context::<Self>()
    }

    fn waiting(&self) -> bool {
        self.waits
            .try_with(|waits| !waits.revealed.get())
            .unwrap_or(false)
    }

    pub fn reveal(&self) {
        let revealed = self.waits.try_with(|waits| {
            waits.revealed.set(true);
            waits.probes.borrow_mut().clear();
            waits.by_node.borrow_mut().clear();
        });
        if revealed.is_some() {
            self.changed.notify();
        }
    }

    pub fn state(&self) -> Ready<()> {
        self.state.get()
    }

    fn wait_on(self, node: Rc<dyn Tracked>) {
        wait_on(self.waits, self.changed, node);
    }
}

/// What an `Errored` component provides: it collects the failed values read under it, and
/// the async memos made under it, which `reset` runs again.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ErrorBoundary {
    changed: Trigger,
    waits: Stored<Waits>,
    refreshes: Stored<RefCell<Vec<Rc<dyn Fn()>>>>,
    state: Memo<Option<String>>,
    /// The `Loading` around it, so a read can tell which of the two is nearer.
    inside: Option<Boundary>,
}

impl ErrorBoundary {
    pub fn provide() -> Self {
        let changed = create_trigger();
        let waits = store(Waits::default());
        let state = create_memo(move || {
            changed.track();
            waits
                .with(Waits::probes)
                .iter()
                .find_map(|probe| match probe() {
                    Err(Pending::Failed(error)) => Some(error),
                    _ => None,
                })
        });
        let boundary = Self {
            changed,
            waits,
            refreshes: store(RefCell::new(Vec::new())),
            state,
            inside: Boundary::nearest(),
        };
        provide_context(boundary);
        boundary
    }

    /// The failure read under it, if any.
    pub fn failure(&self) -> Option<String> {
        self.state.get()
    }

    /// Runs the async memos made under it again, so what failed is asked for once more.
    pub fn reset(&self) {
        let refreshes = self.refreshes.with(|refreshes| refreshes.borrow().clone());
        for refresh in refreshes {
            refresh();
        }
    }

    pub(crate) fn hold_refresh(refresh: Rc<dyn Fn()>) {
        let Some(boundary) = use_context::<Self>() else {
            return;
        };
        let refreshes = boundary.refreshes;
        let held = Rc::clone(&refresh);
        refreshes.with(|refreshes| refreshes.borrow_mut().push(refresh));
        on_cleanup(move || {
            refreshes.try_with(|refreshes| {
                refreshes
                    .borrow_mut()
                    .retain(|refresh| !Rc::ptr_eq(refresh, &held))
            });
        });
    }

    fn wait_on(self, node: Rc<dyn Tracked>) {
        wait_on(self.waits, self.changed, node);
    }
}

/// Registers a read of `node`, which is pending, with the boundary that handles it: a value
/// still loading holds the nearest `Loading` that has not revealed; a failed one goes to the
/// nearest `Errored`, unless a `Loading` nearer than it has not revealed yet, which shows it
/// as its fallback. Reads inside a memo register nothing: the memo is pending in turn.
pub(crate) fn note_read(node: Rc<dyn Tracked>) {
    if in_memo() {
        return;
    }
    let Some(readiness) = node.source().readiness() else {
        return;
    };
    match readiness {
        Pending::Loading => {
            if let Some(boundary) = Boundary::current() {
                boundary.wait_on(node);
            }
        }
        Pending::Failed(_) => {
            let loading = Boundary::nearest();
            let errored = use_context::<ErrorBoundary>();
            let errored_nearer = errored.is_some_and(|errored| errored.inside == loading);
            match (errored, loading) {
                (_, Some(loading)) if !errored_nearer && loading.waiting() => {
                    loading.wait_on(node)
                }
                (Some(errored), _) => errored.wait_on(node),
                _ => {}
            }
        }
    }
}

fn settle(states: impl Iterator<Item = Ready<()>>) -> Ready<()> {
    let mut settled = Ok(());
    for state in states {
        match state {
            Ok(()) => {}
            Err(Pending::Failed(error)) => return Err(Pending::Failed(error)),
            Err(Pending::Loading) => settled = Err(Pending::Loading),
        }
    }
    settled
}

/// The last `T` the source was (`T::default()` before the first). Reading it while the
/// source is pending waits like reading the source, so a `Memo<Ready<T>>` handed to a
/// `Prop<T>` is held this way.
pub fn hold<T, S>(source: S) -> Memo<T>
where
    T: Clone + PartialEq + Default + 'static,
    S: Get<Ready<T>> + 'static,
{
    create_memo(move |last: Option<&T>| {
        source.with(|ready| match ready {
            Ok(value) => value.clone(),
            Err(_) => last.cloned().unwrap_or_default(),
        })
    })
}
